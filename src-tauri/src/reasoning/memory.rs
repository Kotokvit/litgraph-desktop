//! memory.rs — KnowledgeBase: долговременная память reasoning engine.
//!
//! Этот модуль — **семантический слой** между графом проекта (`LitNode`/
//! `LitEdge`) и журналом фактов/событий (`FactLog`). Главная задача —
//! **subgraph retrieval**: вместо отправки ВСЕГО проекта в LLM (как сегодня
//! делает `ai/prompts.rs::build_assistant_prompt`, передавая «статистику»
//! и весь текст) мы извлекаем небольшой подграф вокруг релевантной сущности
//! и отдаём в контекст только его.
//!
//! # Зачем это нужно
//!
//! Текущий подход в `ai/prompts.rs` stuffing'ает в промпт все узлы и рёбра
//! проекта — это взрывается по токенам на больших романах (сотни персонажей,
//! тысячи глав). KnowledgeBase решает эту проблему двумя способами:
//!
//! 1. **`subgraph(center, max_hops)`** — BFS от выбранной сущности, возвращает
//!    только узлы/рёбра/факты/события в радиусе `max_hops`. Для вопроса
//!    «что было между Иваном и Анной?» достаточно 2 прыжков вместо всего графа.
//! 2. **`retrieve_for_question(question, max_nodes)`** — токенизирует вопрос
//!    пользователя, находит совпадения в именах узлов, объединяет подграфы
//!    каждого совпадения и обрезает по `max_nodes` (по степени узла).
//!
//! # Архитектурный принцип
//!
//! KnowledgeBase — **read-only** после построения. `from_project` копирует
//! узлы/рёбра и забирает владение `FactLog`. Все методы — чистые getter'ы
//! и алгоритмы извлечения, никакой мутации внешнего состояния. Это согласовано
//! с принципом SPEC §0.1: «State is truth» — мутации идут через `WorldState`,
//! а `KnowledgeBase` только поставляет контекст.
//!
//! # Связь с другими модулями
//!
//! - [`FactLog`] — owned копия журнала фактов/событий (см. SPEC §2.6).
//! - [`LitNode`]/[`LitEdge`] — копии графа проекта (см. `crate::models`).
//! - BFS-алгоритм вдохновлён `causality.rs::explain_chain`, но работает
//!   на node-ID, а не на EventId.
//!
//! # Example
//!
//! ```ignore
//! use litgraph_desktop_lib::reasoning::memory::KnowledgeBase;
//! use litgraph_desktop_lib::reasoning::facts::FactLog;
//! use litgraph_desktop_lib::models::Project;
//!
//! let kb = KnowledgeBase::from_project(&project, fact_log);
//! let sg = kb.retrieve_for_question("Где Иван встретил Анну?", 20);
//! println!("{}", sg.summary());
//! // → "Подграф вокруг «Иван» (2 прыжка): 5 узлов, 8 рёбер, 12 фактов, 6 событий"
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::models::{LitEdge, LitNode, Project};
use crate::reasoning::facts::{Event, Fact, FactLog};
// FactValue и TemporalAnchor нужны только в тестах (для сборки фикстур).
// В не-test сборке они не ссылаются напрямую — silenced allow(unused_imports).
#[allow(unused_imports)]
use crate::reasoning::facts::FactValue;
#[allow(unused_imports)]
use crate::reasoning::timeline::TemporalAnchor;

// ============================================================================
// Subgraph
// ============================================================================

/// Результат извлечения фрагмента базы знаний для контекста LLM.
///
/// Содержит:
/// - `center` — ID сущности, вокруг которой построен подграф.
/// - `nodes` / `edges` — структурный фрагмент графа.
/// - `facts` / `events` — семантический фрагмент (FactLog slice).
/// - `max_hops` — радиус BFS, использованный при извлечении.
///
/// Все коллекции отсортированы по ID для детерминированности (важно для
/// тестов и для воспроизводимости промптов LLM).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subgraph {
    /// ID центрального узла (или строки запроса, если совпадений не было).
    pub center: String,
    /// Узлы графа в радиусе `max_hops` от `center`.
    pub nodes: Vec<LitNode>,
    /// Рёбра, у которых оба конца (`source` и `target`) лежат в `nodes`.
    pub edges: Vec<LitEdge>,
    /// Факты, у которых `entity` есть в `nodes`.
    pub facts: Vec<Fact>,
    /// События, у которых `actor` ИЛИ `target` есть в `nodes`.
    pub events: Vec<Event>,
    /// Радиус BFS, использованный при извлечении подграфа.
    pub max_hops: usize,
}

impl Subgraph {
    /// `true` если подграф пуст (нет ни узлов, ни рёбер, ни фактов, ни событий).
    ///
    /// Это значит, что извлечение ничего не нашло — например, запрос не
    /// совпал ни с одним именем узла.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.edges.is_empty()
            && self.facts.is_empty()
            && self.events.is_empty()
    }

    /// Человекочитаемое summary на русском (для отладки и UI).
    ///
    /// Формат: `Подграф вокруг «{center}» ({hops} прыжок|прыжка|прыжков):
    /// {n} узел|узла|узлов, {m} ребро|ребра|рёбер, {k} факт|факта|фактов,
    /// {j} событие|события|событий`.
    ///
    /// Пример: `Подграф вокруг «Иван» (3 прыжка): 5 узлов, 8 рёбер, 12 фактов,
    /// 6 событий`.
    pub fn summary(&self) -> String {
        format!(
            "Подграф вокруг «{}» ({} {}): {} {}, {} {}, {} {}, {} {}",
            self.center,
            self.max_hops,
            pluralize_ru(self.max_hops, "прыжок", "прыжка", "прыжков"),
            self.nodes.len(),
            pluralize_ru(self.nodes.len(), "узел", "узла", "узлов"),
            self.edges.len(),
            pluralize_ru(self.edges.len(), "ребро", "ребра", "рёбер"),
            self.facts.len(),
            pluralize_ru(self.facts.len(), "факт", "факта", "фактов"),
            self.events.len(),
            pluralize_ru(self.events.len(), "событие", "события", "событий"),
        )
    }
}

// ============================================================================
// KnowledgeBase
// ============================================================================

/// Долговременное хранилище reasoning engine: граф проекта + FactLog.
///
/// Построение: [`KnowledgeBase::from_project`] копирует `nodes`/`edges` из
/// [`Project`] и забирает владение переданным [`FactLog`]. Adjacency list
/// строится автоматически (рёбра трактуются как неориентированные).
///
/// Все методы — иммутабельные getter'ы и алгоритмы извлечения. Никаких
/// мутаций: чтобы добавить факт/событие, мутабельную сторону ведёт сам
/// `FactLog` (до передачи в `KnowledgeBase`).
pub struct KnowledgeBase {
    /// Узлы графа по ID (`LitNode.id` → node).
    nodes: HashMap<String, LitNode>,
    /// Все рёбра проекта (в порядке вставки).
    edges: Vec<LitEdge>,
    /// Журнал фактов и событий (owned).
    facts: FactLog,
    /// Adjacency list: node_id → список соседей (неориентированный).
    /// Соседи каждого узла хранятся в порядке первого появления в `edges`,
    /// без дубликатов.
    adjacency: HashMap<String, Vec<String>>,
}

impl KnowledgeBase {
    /// Создать пустую базу знаний.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            facts: FactLog::new(),
            adjacency: HashMap::new(),
        }
    }

    /// Построить KB из проекта и готового FactLog.
    ///
    /// - Копирует все `nodes` (по значению, через `clone`).
    /// - Копирует все `edges` (через `clone`).
    /// - Забирает владение `facts` (без клонирования).
    /// - Строит adjacency list (рёбра неориентированные, дубликаты
    ///   соседей удаляются).
    pub fn from_project(project: &Project, facts: FactLog) -> Self {
        let mut nodes = HashMap::with_capacity(project.nodes.len());
        for n in &project.nodes {
            nodes.insert(n.id.clone(), n.clone());
        }
        let edges = project.edges.clone();
        let adjacency = build_adjacency(&edges);
        Self {
            nodes,
            edges,
            facts,
            adjacency,
        }
    }

    /// Найти узел по ID.
    pub fn get_node(&self, id: &str) -> Option<&LitNode> {
        self.nodes.get(id)
    }

    /// Соседи узла по любому ребру (рёбра неориентированные).
    ///
    /// Возвращает ссылки на узлы, отсортированные по ID для детерминированности.
    /// Дубликаты (если несколько рёбер ведут к одному соседу) устраняются.
    pub fn neighbors(&self, id: &str) -> Vec<&LitNode> {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut result: Vec<&LitNode> = Vec::new();
        if let Some(neighbor_ids) = self.adjacency.get(id) {
            for nid in neighbor_ids {
                if seen.insert(nid.as_str()) {
                    if let Some(n) = self.nodes.get(nid) {
                        result.push(n);
                    }
                }
            }
        }
        // Sort for deterministic output (HashMap adjacency order may vary).
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }

    /// Соседи узла, соединённые ребром с конкретным `data.kind`.
    ///
    /// Рёбра трактуются как неориентированные: если `id` — это `source` ИЛИ
    /// `target` ребра с `data.kind == Some(edge_kind)`, противоположный
    /// конец считается соседом.
    ///
    /// Рёбра без `data` или без `kind` пропускаются.
    pub fn neighbors_filtered(&self, id: &str, edge_kind: &str) -> Vec<&LitNode> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut result: Vec<&LitNode> = Vec::new();
        for e in &self.edges {
            // Определяем «другой конец» ребра относительно `id`.
            let other_id = if e.source == id {
                Some(&e.target)
            } else if e.target == id {
                Some(&e.source)
            } else {
                None
            };
            let Some(other) = other_id else { continue };
            // Проверяем kind.
            let kind_matches = e
                .data
                .as_ref()
                .and_then(|d| d.kind.as_ref())
                .map(|k| k == edge_kind)
                .unwrap_or(false);
            if !kind_matches {
                continue;
            }
            if seen.insert(other.clone()) {
                if let Some(n) = self.nodes.get(other) {
                    result.push(n);
                }
            }
        }
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }

    /// Все активные факты для сущности. Делегирует в `FactLog::get_facts_for`.
    pub fn facts_for(&self, entity: &str) -> Vec<&Fact> {
        self.facts.get_facts_for(entity)
    }

    /// Все события, в которых `entity` — actor ИЛИ target.
    ///
    /// События без target проверяют только actor. Возвращает ссылки в порядке
    /// вставки событий в `FactLog`.
    pub fn events_involving(&self, entity: &str) -> Vec<&Event> {
        self.facts
            .all_events()
            .iter()
            .filter(|e| e.actor == entity || e.target.as_deref() == Some(entity))
            .collect()
    }

    /// Все события в указанной главе (по `time.chapter_num`).
    /// Делегирует в `FactLog::get_events_in_chapter`.
    pub fn events_in_chapter(&self, chapter: u32) -> Vec<&Event> {
        self.facts.get_events_in_chapter(chapter)
    }

    /// BFS от `entity` до `max_hops` включительно.
    ///
    /// Возвращает уникальные ID сущностей, достижимых из `entity` за
    /// `max_hops` шагов (не включая сам `entity`). Порядок —
    /// лексикографический по ID (для детерминированности).
    ///
    /// `max_hops = 0` → пустой результат (никаких соседей).
    /// `max_hops = 1` → прямые соседи.
    pub fn related_entities(&self, entity: &str, max_hops: usize) -> Vec<String> {
        let frontier = self.bfs_frontier(entity, max_hops);
        let mut result: Vec<String> = frontier.into_iter().filter(|id| id != entity).collect();
        result.sort();
        result
    }

    /// Извлечь подграф вокруг `center` радиусом `max_hops` (BFS).
    ///
    /// Алгоритм (см. SPEC §3 memory.rs brief):
    /// 1. BFS от `center`, собираем все ID в радиусе `max_hops`.
    /// 2. Узлы: все из frontier, которые есть в `self.nodes`.
    /// 3. Рёбра: оба конца (source и target) в frontier.
    /// 4. Факты: `entity` в frontier.
    /// 5. События: `actor` в frontier ИЛИ `target` в frontier.
    ///
    /// Если `center` не существует ни в `nodes`, ни в `adjacency`, frontier
    /// = `{center}` и подграф может содержать только факты/события для этого
    /// ID (если они есть в FactLog).
    pub fn subgraph(&self, center: &str, max_hops: usize) -> Subgraph {
        let frontier = self.bfs_frontier(center, max_hops);

        let mut nodes: Vec<LitNode> = frontier
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .cloned()
            .collect();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));

        let mut edges: Vec<LitEdge> = self
            .edges
            .iter()
            .filter(|e| frontier.contains(&e.source) && frontier.contains(&e.target))
            .cloned()
            .collect();
        edges.sort_by(|a, b| a.id.cmp(&b.id));

        let mut facts: Vec<Fact> = self
            .facts
            .all_facts()
            .iter()
            .filter(|f| frontier.contains(&f.entity))
            .cloned()
            .collect();
        facts.sort_by_key(|f| f.id);

        let mut events: Vec<Event> = self
            .facts
            .all_events()
            .iter()
            .filter(|e| {
                frontier.contains(&e.actor)
                    || e.target.as_ref().map(|t| frontier.contains(t)).unwrap_or(false)
            })
            .cloned()
            .collect();
        events.sort_by_key(|e| e.id);

        Subgraph {
            center: center.to_string(),
            nodes,
            edges,
            facts,
            events,
            max_hops,
        }
    }

    /// Поиск узлов по названию (case-insensitive substring на `data.title`).
    ///
    /// Возвращает ссылки, отсортированные по ID для детерминированности.
    pub fn search_by_name(&self, query: &str) -> Vec<&LitNode> {
        if query.is_empty() {
            return Vec::new();
        }
        let q = query.to_lowercase();
        let mut result: Vec<&LitNode> = self
            .nodes
            .values()
            .filter(|n| n.data.title.to_lowercase().contains(&q))
            .collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));
        result
    }

    /// Количество узлов в базе.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Количество рёбер в базе.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Количество фактов (включая ретракнутые — для audit).
    pub fn fact_count(&self) -> usize {
        self.facts.all_facts().len()
    }

    /// Количество событий.
    pub fn event_count(&self) -> usize {
        self.facts.all_events().len()
    }

    /// Наивное извлечение контекста по строке запроса.
    ///
    /// Алгоритм:
    /// 1. Найти узлы, чей `title` содержит `query` (case-insensitive).
    /// 2. Если совпадений нет — вернуть пустой Subgraph.
    /// 3. Иначе: взять первое совпадение как center, запустить
    ///    `subgraph(center, max_hops=2)`.
    /// 4. Если в подграфе больше `max_nodes` узлов — обрезать по степени
    ///    узла (desc), сохраняя только топ-N и связанные с ними рёбра/
    ///    факты/события.
    pub fn retrieve_relevant(&self, query: &str, max_nodes: usize) -> Subgraph {
        let matches = self.search_by_name(query);
        if matches.is_empty() {
            return Subgraph {
                center: query.to_string(),
                nodes: Vec::new(),
                edges: Vec::new(),
                facts: Vec::new(),
                events: Vec::new(),
                max_hops: 0,
            };
        }
        // matches отсортированы по ID — берём первое (стабильно).
        let center = matches[0].id.clone();
        let sg = self.subgraph(&center, 2);
        trim_subgraph(sg, max_nodes)
    }

    /// Контекстно-зависимое извлечение для вопроса пользователя.
    ///
    /// Алгоритм:
    /// 1. Токенизация вопроса по whitespace.
    /// 2. Для каждого токена — поиск узлов по title (case-insensitive
    ///    substring).
    /// 3. Дедупликация совпадений по ID.
    /// 4. Если совпадений нет — пустой Subgraph.
    /// 5. Если одно совпадение — `subgraph(match, max_hops=2)`.
    /// 6. Если несколько — объединение подграфов (union по ID для nodes/
    ///    edges/facts/events).
    /// 7. Обрезать до `max_nodes` по степени узла (desc).
    pub fn retrieve_for_question(&self, question: &str, max_nodes: usize) -> Subgraph {
        let tokens: Vec<&str> = question.split_whitespace().collect();

        let mut match_ids: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for token in tokens {
            for n in self.search_by_name(token) {
                if seen.insert(n.id.clone()) {
                    match_ids.push(n.id.clone());
                }
            }
        }

        if match_ids.is_empty() {
            return Subgraph {
                center: question.to_string(),
                nodes: Vec::new(),
                edges: Vec::new(),
                facts: Vec::new(),
                events: Vec::new(),
                max_hops: 0,
            };
        }

        if match_ids.len() == 1 {
            let sg = self.subgraph(&match_ids[0], 2);
            return trim_subgraph(sg, max_nodes);
        }

        // Multiple matches: combine subgraphs (union by ID).
        let mut subgraphs: Vec<Subgraph> = Vec::with_capacity(match_ids.len());
        for id in &match_ids {
            subgraphs.push(self.subgraph(id, 2));
        }
        let merged = merge_subgraphs(subgraphs, match_ids[0].clone());
        trim_subgraph(merged, max_nodes)
    }

    // ── Внутренние хелперы ──────────────────────────────────────────────

    /// BFS от `start`, собираем все достижимые ID в радиусе `max_hops`.
    ///
    /// `start` всегда включается в frontier (даже если его нет в `nodes`
    /// или `adjacency`), чтобы можно было извлечь факты/события для ID,
    /// у которого нет графа-узла.
    fn bfs_frontier(&self, start: &str, max_hops: usize) -> HashSet<String> {
        let mut frontier: HashSet<String> = HashSet::new();
        frontier.insert(start.to_string());

        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(start.to_string());

        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        queue.push_back((start.to_string(), 0));

        while let Some((node, hops)) = queue.pop_front() {
            if hops >= max_hops {
                continue;
            }
            if let Some(neighbor_ids) = self.adjacency.get(&node) {
                for n in neighbor_ids {
                    if visited.insert(n.clone()) {
                        frontier.insert(n.clone());
                        queue.push_back((n.clone(), hops + 1));
                    }
                }
            }
        }

        frontier
    }
}

impl Default for KnowledgeBase {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for KnowledgeBase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeBase")
            .field("node_count", &self.nodes.len())
            .field("edge_count", &self.edges.len())
            .field("fact_count", &self.facts.all_facts().len())
            .field("event_count", &self.facts.all_events().len())
            .field("adjacency_size", &self.adjacency.len())
            .finish()
    }
}

// ============================================================================
// Приватные хелперы (фри-функции)
// ============================================================================

/// Построить неориентированный adjacency list из списка рёбер.
///
/// Для каждого ребра добавляем `source → target` И `target → source`.
/// Дубликаты соседей (от многократно повторяющихся рёбер) удаляются.
fn build_adjacency(edges: &[LitEdge]) -> HashMap<String, Vec<String>> {
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for e in edges {
        adj.entry(e.source.clone()).or_default().push(e.target.clone());
        adj.entry(e.target.clone()).or_default().push(e.source.clone());
    }
    // Dedup each neighbor list (preserve first-seen order).
    for v in adj.values_mut() {
        let mut seen: HashSet<String> = HashSet::new();
        v.retain(|id| seen.insert(id.clone()));
    }
    adj
}

/// Обрезать подграф до `max_nodes` узлов, отсортированных по степени (desc).
///
/// Алгоритм:
/// 1. Если `nodes.len() <= max_nodes` — вернуть как есть.
/// 2. Посчитать степень каждого узла по рёбрам подграфа (сколько раз
///    встречается как source или target).
/// 3. Отсортировать узлы: по степени desc, при равенстве — по ID asc
///    (для стабильности).
/// 4. Оставить первые `max_nodes` узлов.
/// 5. Отфильтровать рёбра: оба конца в оставшихся.
/// 6. Отфильтровать факты: `entity` в оставшихся.
/// 7. Отфильтровать события: `actor` ИЛИ `target` в оставшихся.
fn trim_subgraph(mut sg: Subgraph, max_nodes: usize) -> Subgraph {
    if sg.nodes.len() <= max_nodes {
        return sg;
    }

    // Считаем степень по рёбрам подграфа.
    let mut degree: HashMap<&str, usize> = HashMap::new();
    for e in &sg.edges {
        *degree.entry(e.source.as_str()).or_insert(0) += 1;
        *degree.entry(e.target.as_str()).or_insert(0) += 1;
    }

    // Сортируем: степень desc, ID asc.
    sg.nodes.sort_by(|a, b| {
        let da = degree.get(a.id.as_str()).copied().unwrap_or(0);
        let db = degree.get(b.id.as_str()).copied().unwrap_or(0);
        db.cmp(&da).then_with(|| a.id.cmp(&b.id))
    });

    sg.nodes.truncate(max_nodes);

    let kept: HashSet<String> = sg.nodes.iter().map(|n| n.id.clone()).collect();
    sg.edges
        .retain(|e| kept.contains(&e.source) && kept.contains(&e.target));
    sg.facts.retain(|f| kept.contains(&f.entity));
    sg.events.retain(|e| {
        kept.contains(&e.actor)
            || e.target.as_ref().map(|t| kept.contains(t)).unwrap_or(false)
    });

    sg
}

/// Объединить несколько подграфов в один (union по ID).
///
/// `center` результирующего подграфа — переданный аргумент (обычно ID
/// первого совпадения). `max_hops` — максимум по всем входным подграфам.
fn merge_subgraphs(subgraphs: Vec<Subgraph>, center: String) -> Subgraph {
    let mut nodes_map: HashMap<String, LitNode> = HashMap::new();
    let mut edges_map: HashMap<String, LitEdge> = HashMap::new();
    let mut facts_map: HashMap<u64, Fact> = HashMap::new();
    let mut events_map: HashMap<u64, Event> = HashMap::new();
    let mut max_hops = 0usize;

    for sg in subgraphs {
        if sg.max_hops > max_hops {
            max_hops = sg.max_hops;
        }
        for n in sg.nodes {
            nodes_map.entry(n.id.clone()).or_insert(n);
        }
        for e in sg.edges {
            edges_map.entry(e.id.clone()).or_insert(e);
        }
        for f in sg.facts {
            facts_map.entry(f.id).or_insert(f);
        }
        for ev in sg.events {
            events_map.entry(ev.id).or_insert(ev);
        }
    }

    let mut nodes: Vec<LitNode> = nodes_map.into_values().collect();
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut edges: Vec<LitEdge> = edges_map.into_values().collect();
    edges.sort_by(|a, b| a.id.cmp(&b.id));

    let mut facts: Vec<Fact> = facts_map.into_values().collect();
    facts.sort_by_key(|f| f.id);

    let mut events: Vec<Event> = events_map.into_values().collect();
    events.sort_by_key(|e| e.id);

    Subgraph {
        center,
        nodes,
        edges,
        facts,
        events,
        max_hops,
    }
}

/// Русская плюрализация: возвращает одну из трёх форм в зависимости от числа.
///
/// Правило: последние две цифры определяют форму.
/// - 1 (но не 11) → `one` («прыжок», «узел», «ребро», «факт», «событие»).
/// - 2–4 (но не 12–14) → `few` («прыжка», «узла», «ребра», «факта», «события»).
/// - 0, 5–19, 20+ → `many` («прыжков», «узлов», «рёбер», «фактов», «событий»).
fn pluralize_ru<'a>(n: usize, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let n10 = n % 10;
    let n100 = n % 100;
    if n10 == 1 && n100 != 11 {
        one
    } else if (2..=4).contains(&n10) && !(12..=14).contains(&n100) {
        few
    } else {
        many
    }
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::edge::EdgeData;
    use crate::models::node::{LitNode, LitNodeData, Position};
    use crate::models::edge::LitEdge;
    use crate::models::project::Project;
    use crate::reasoning::facts::{
        Action, Event, Fact, FactId, FactLog, FactValue, Provenance,
    };
    use crate::reasoning::timeline::TemporalAnchor;

    // ── Хелперы для тестовых данных ────────────────────────────────────

    /// Хелпер: минимальный LitNode с заданным ID, типом и title.
    fn make_node(id: &str, node_type: &str, title: &str) -> LitNode {
        LitNode {
            id: id.to_string(),
            node_type: node_type.to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: title.to_string(),
                body: String::new(),
                node_type: node_type.to_string(),
                tags: Vec::new(),
                meta: None,
                full_text: None,
                versions: None,
            },
        }
    }

    /// Хелпер: минимальное LitEdge с заданным kind.
    fn make_edge(id: &str, source: &str, target: &str, kind: &str) -> LitEdge {
        LitEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            source_handle: None,
            target_handle: None,
            edge_type: None,
            animated: None,
            data: Some(EdgeData {
                kind: Some(kind.to_string()),
                note: None,
            }),
        }
    }

    /// Хелпер: TemporalAnchor для главы (без суффикса/сцены/offset).
    fn anchor(chapter: u32) -> TemporalAnchor {
        TemporalAnchor {
            chapter_num: chapter,
            chapter_suffix: None,
            scene_index: None,
            char_offset: 0,
        }
    }

    /// Хелпер: минимальный Fact.
    fn make_fact(id: FactId, entity: &str, attr: &str, value: FactValue, time: TemporalAnchor) -> Fact {
        Fact {
            id,
            entity: entity.to_string(),
            attribute: attr.to_string(),
            value,
            derived_from: Vec::new(),
            valid_from: time,
            valid_until: None,
            provenance: Provenance::SvoParser,
        }
    }

    /// Хелпер: минимальный Event.
    fn make_event(
        id: u64,
        actor: &str,
        action: Action,
        target: Option<&str>,
        time: TemporalAnchor,
    ) -> Event {
        Event {
            id,
            actor: actor.to_string(),
            action,
            target: target.map(|s| s.to_string()),
            instrument: None,
            time,
            source_text: String::new(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        }
    }

    /// Сборка тестового проекта: 4 узла (Иван, Анна, Замок, Глава 1) и
    /// 4 ребра (location×2, character×1, reference×1) + заполненный FactLog
    /// с 4 фактами и 3 событиями.
    fn build_fixture() -> (Project, FactLog) {
        let nodes = vec![
            make_node("ivan", "character", "Иван"),
            make_node("anna", "character", "Анна"),
            make_node("castle", "location", "Замок"),
            make_node("ch1", "chapter", "Глава 1"),
        ];
        let edges = vec![
            make_edge("e1", "ivan", "castle", "location"),
            make_edge("e2", "anna", "castle", "location"),
            make_edge("e3", "ivan", "anna", "character"),
            make_edge("e4", "ch1", "ivan", "reference"),
        ];
        let project = Project {
            title: "Test Project".to_string(),
            author: "Test".to_string(),
            description: String::new(),
            nodes,
            edges,
            created_at: 0,
            updated_at: 0,
        };

        let mut facts = FactLog::new();
        // 4 факта: alive×2 + location×2.
        facts.assert_fact(make_fact(0, "ivan", "alive", FactValue::Bool(true), anchor(1)));
        facts.assert_fact(make_fact(0, "anna", "alive", FactValue::Bool(true), anchor(1)));
        facts.assert_fact(make_fact(0, "ivan", "location", FactValue::Str("Замок".into()), anchor(1)));
        facts.assert_fact(make_fact(0, "anna", "location", FactValue::Str("Замок".into()), anchor(1)));

        // 3 события: Ivan Speak→Anna, Anna Arrive, Ivan Kill→Anna.
        facts.record_event(make_event(
            0,
            "ivan",
            Action::Speak { topic: None },
            Some("anna"),
            anchor(1),
        ));
        facts.record_event(make_event(
            0,
            "anna",
            Action::Arrive { destination: "Замок".into() },
            None,
            anchor(1),
        ));
        facts.record_event(make_event(
            0,
            "ivan",
            Action::Kill,
            Some("anna"),
            anchor(2),
        ));

        (project, facts)
    }

    // ── Тесты (8 обязательных) ────────────────────────────────────────

    #[test]
    fn test_knowledge_base_from_project_initializes_adjacency() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        // Все 4 узла на месте.
        assert_eq!(kb.node_count(), 4, "должно быть 4 узла");
        assert_eq!(kb.edge_count(), 4, "должно быть 4 ребра");
        assert_eq!(kb.fact_count(), 4, "должно быть 4 факта");
        assert_eq!(kb.event_count(), 3, "должно быть 3 события");

        // Adjacency построен (проверяем через neighbors, т.к. поле приватное).
        // ivan соединён с: castle (e1), anna (e3), ch1 (e4) — 3 соседа.
        let ivan_neighbors = kb.neighbors("ivan");
        assert_eq!(
            ivan_neighbors.len(),
            3,
            "Иван должен иметь 3 соседа (castle, anna, ch1)"
        );

        // Default возвращает пустую KB.
        let empty = KnowledgeBase::default();
        assert_eq!(empty.node_count(), 0);
        assert_eq!(empty.edge_count(), 0);
    }

    #[test]
    fn test_neighbors_returns_directly_connected_nodes() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        // ivan → 3 соседа: anna, castle, ch1.
        let ivan_n = kb.neighbors("ivan");
        let ivan_ids: Vec<&str> = ivan_n.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ivan_n.len(), 3, "Иван должен иметь 3 соседа");
        assert!(ivan_ids.contains(&"anna"));
        assert!(ivan_ids.contains(&"castle"));
        assert!(ivan_ids.contains(&"ch1"));

        // anna → 2 соседа: castle, ivan.
        let anna_n = kb.neighbors("anna");
        let anna_ids: Vec<&str> = anna_n.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(anna_n.len(), 2, "Анна должна иметь 2 соседа");
        assert!(anna_ids.contains(&"castle"));
        assert!(anna_ids.contains(&"ivan"));

        // castle → 2 соседа: anna, ivan.
        let castle_n = kb.neighbors("castle");
        assert_eq!(castle_n.len(), 2, "Замок должен иметь 2 соседа");

        // ch1 → 1 сосед: ivan.
        let ch1_n = kb.neighbors("ch1");
        assert_eq!(ch1_n.len(), 1, "Глава 1 должна иметь 1 соседа");
        assert_eq!(ch1_n[0].id, "ivan");

        // Несуществующий ID → пусто.
        assert!(kb.neighbors("nonexistent").is_empty());
    }

    #[test]
    fn test_neighbors_filtered_by_edge_kind() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        // ivan через "location" → только castle (e1: ivan→castle).
        let loc = kb.neighbors_filtered("ivan", "location");
        assert_eq!(loc.len(), 1, "ivan должен иметь 1 соседа через location");
        assert_eq!(loc[0].id, "castle");

        // ivan через "character" → только anna (e3: ivan→anna).
        let chr = kb.neighbors_filtered("ivan", "character");
        assert_eq!(chr.len(), 1);
        assert_eq!(chr[0].id, "anna");

        // ivan через "reference" → только ch1 (e4: ch1→ivan, неориентированно).
        let ref_ = kb.neighbors_filtered("ivan", "reference");
        assert_eq!(ref_.len(), 1);
        assert_eq!(ref_[0].id, "ch1");

        // Несуществующий kind → пусто.
        assert!(kb.neighbors_filtered("ivan", "nonexistent_kind").is_empty());

        // anna через "location" → castle.
        let anna_loc = kb.neighbors_filtered("anna", "location");
        assert_eq!(anna_loc.len(), 1);
        assert_eq!(anna_loc[0].id, "castle");
    }

    #[test]
    fn test_events_involving_finds_actor_and_target() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        // ivan: actor в Speak (E1) и Kill (E3). Target ни в одном не указан.
        // → 2 события.
        let ivan_events = kb.events_involving("ivan");
        assert_eq!(
            ivan_events.len(),
            2,
            "Иван должен быть вовлечён в 2 события (Speak + Kill)"
        );

        // anna: target в Speak (E1), actor в Arrive (E2), target в Kill (E3).
        // → 3 события.
        let anna_events = kb.events_involving("anna");
        assert_eq!(
            anna_events.len(),
            3,
            "Анна должна быть вовлечена в 3 события (Speak target + Arrive actor + Kill target)"
        );

        // castle: не actor и не target ни в одном событии.
        let castle_events = kb.events_involving("castle");
        assert!(
            castle_events.is_empty(),
            "Замок не должен быть вовлечён в события"
        );

        // Несуществующая сущность → пусто.
        assert!(kb.events_involving("nonexistent").is_empty());
    }

    #[test]
    fn test_related_entities_bfs_within_max_hops() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        // ivan, max_hops=1 → прямые соседи: anna, castle, ch1 (3).
        let ivan_1 = kb.related_entities("ivan", 1);
        assert_eq!(ivan_1.len(), 3, "ivan hop 1 должен дать 3 сущности");
        assert!(ivan_1.contains(&"anna".to_string()));
        assert!(ivan_1.contains(&"castle".to_string()));
        assert!(ivan_1.contains(&"ch1".to_string()));

        // ivan, max_hops=2 → те же 3 (граф маленький, новых нет).
        let ivan_2 = kb.related_entities("ivan", 2);
        assert_eq!(ivan_2.len(), 3, "ivan hop 2 должен дать те же 3 сущности");

        // ivan, max_hops=0 → никого (только сам ivan, исключается).
        let ivan_0 = kb.related_entities("ivan", 0);
        assert!(ivan_0.is_empty(), "ivan hop 0 должен дать пустой список");

        // anna, max_hops=1 → 2 соседа: castle, ivan.
        let anna_1 = kb.related_entities("anna", 1);
        assert_eq!(anna_1.len(), 2);
        assert!(anna_1.contains(&"castle".to_string()));
        assert!(anna_1.contains(&"ivan".to_string()));

        // anna, max_hops=2 → +ch1 (через ivan). Итого 3.
        let anna_2 = kb.related_entities("anna", 2);
        assert_eq!(anna_2.len(), 3, "anna hop 2 должен дать 3 сущности (включая ch1 через ivan)");
        assert!(anna_2.contains(&"ch1".to_string()));

        // Сама сущность никогда не возвращается.
        assert!(!ivan_1.contains(&"ivan".to_string()));
        assert!(!anna_1.contains(&"anna".to_string()));
    }

    #[test]
    fn test_subgraph_collects_nodes_edges_facts_events() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        let sg = kb.subgraph("ivan", 2);

        // Все 4 узла достижимы в радиусе 2 от ivan.
        assert_eq!(sg.nodes.len(), 4, "подграф ivan hop 2 должен содержать 4 узла");
        let node_ids: Vec<&str> = sg.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(node_ids.contains(&"ivan"));
        assert!(node_ids.contains(&"anna"));
        assert!(node_ids.contains(&"castle"));
        assert!(node_ids.contains(&"ch1"));

        // Все 4 ребра имеют оба конца в frontier.
        assert_eq!(sg.edges.len(), 4, "подграф должен содержать 4 ребра");

        // 4 факта (ivan/anna × alive/location).
        assert_eq!(sg.facts.len(), 4, "подграф должен содержать 4 факта");

        // 3 события (ivan Speak, anna Arrive, ivan Kill).
        assert_eq!(sg.events.len(), 3, "подграф должен содержать 3 события");

        // Метаданные.
        assert_eq!(sg.center, "ivan");
        assert_eq!(sg.max_hops, 2);
        assert!(!sg.is_empty(), "подграф не должен быть пустым");

        // Summary содержит ключевые элементы.
        let s = sg.summary();
        assert!(s.contains("Иван") || s.contains("ivan"), "summary должен содержать center");
        assert!(s.contains("4 узла"), "summary должен сообщать 4 узла");
        assert!(s.contains("4 ребра"), "summary должен сообщать 4 ребра");
        assert!(s.contains("4 факта"), "summary должен сообщать 4 факта");
        assert!(s.contains("3 события"), "summary должен сообщать 3 события");
        assert!(s.contains("2 прыжка"), "summary должен сообщать 2 прыжка");
    }

    #[test]
    fn test_retrieve_relevant_finds_matching_node() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        // Case-insensitive: "иван" matches "Иван".
        let sg = kb.retrieve_relevant("иван", 10);
        assert_eq!(sg.center, "ivan", "центр должен быть ivan (первое совпадение)");
        assert_eq!(sg.nodes.len(), 4, "подграф должен содержать все 4 узла (max_nodes=10)");
        assert_eq!(sg.edges.len(), 4);
        assert!(!sg.is_empty());

        // Title-case: "Иван" тоже находит.
        let sg2 = kb.retrieve_relevant("Иван", 10);
        assert_eq!(sg2.center, "ivan");

        // Partial match: "ан" находит и "Иван", и "Анну".
        // search_by_name отсортирован по ID: "anna" < "ivan", so center="anna".
        let sg3 = kb.retrieve_relevant("ан", 10);
        assert_eq!(sg3.center, "anna", "первое совпадение по ID должно быть anna");

        // Нет совпадений → пустой Subgraph, center=query.
        let sg_empty = kb.retrieve_relevant("nonexistent", 10);
        assert!(sg_empty.is_empty(), "несуществующий запрос должен дать пустой подграф");
        assert_eq!(sg_empty.center, "nonexistent");

        // Trim: max_nodes=2 → обрезка по степени.
        // Степени в подграфе ivan (4 узла, 4 ребра):
        //   ivan: 3 (e1 ivan-castle, e3 ivan-anna, e4 ch1-ivan)
        //   anna: 2 (e2 anna-castle, e3 ivan-anna)
        //   castle: 2 (e1 ivan-castle, e2 anna-castle)
        //   ch1: 1 (e4 ch1-ivan)
        // Топ-2 по степени: ivan (3), затем anna или castle (оба 2 — anna < castle
        // по ID, так что anna).
        let sg_trim = kb.retrieve_relevant("иван", 2);
        assert_eq!(sg_trim.nodes.len(), 2, "trim должен оставить 2 узла");
        let trim_ids: Vec<&str> = sg_trim.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(trim_ids.contains(&"ivan"), "ivan (макс. степень) должен остаться");
        // anna или castle — кто-то из них должен остаться.
        assert!(
            trim_ids.contains(&"anna") || trim_ids.contains(&"castle"),
            "второй узел должен быть anna или castle (степень 2)"
        );
        // Рёбра только между оставшимися.
        for e in &sg_trim.edges {
            assert!(trim_ids.contains(&e.source.as_str()));
            assert!(trim_ids.contains(&e.target.as_str()));
        }
    }

    #[test]
    fn test_retrieve_for_question_handles_multi_word_query() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        // Мульти-слово: "Иван Анна" → оба совпадают, объединение подграфов.
        let sg = kb.retrieve_for_question("Иван Анна", 10);
        assert!(!sg.is_empty(), "multi-word запрос должен найти совпадения");
        assert_eq!(
            sg.center, "ivan",
            "center должен быть первым совпадением по ID (ivan < anna неверно, \
             но search_by_name возвращает [anna, ivan] по ID — anna первая)"
        );
        // Проверим: токены "Иван" и "Анна" дают match_ids в порядке токенов:
        // "Иван" → ivan, "Анна" → anna. match_ids = [ivan, anna].
        // Но search_by_name сортирует по ID. anna < ivan, но мы итерируем
        // по токенам по порядку, так что match_ids = [ivan (из Иван), anna (из Анна)].
        // center = match_ids[0] = ivan.
        assert_eq!(sg.center, "ivan");

        // Все 4 узла должны быть в объединённом подграфе.
        let node_ids: Vec<&str> = sg.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(node_ids.contains(&"ivan"), "ivan должен быть в объединении");
        assert!(node_ids.contains(&"anna"), "anna должна быть в объединении");
        assert!(node_ids.contains(&"castle"), "castle должен быть (сосед обоих)");
        assert!(node_ids.contains(&"ch1"), "ch1 должен быть (сосед ivan)");
        assert_eq!(sg.nodes.len(), 4);

        // Одно слово-совпадение: "Иван" → подграф ivan hop 2.
        let sg_single = kb.retrieve_for_question("Иван", 10);
        assert_eq!(sg_single.center, "ivan");
        assert_eq!(sg_single.nodes.len(), 4);

        // Нет совпадений: "nonexistent word" → пустой.
        let sg_empty = kb.retrieve_for_question("nonexistent word", 10);
        assert!(sg_empty.is_empty());
        assert_eq!(sg_empty.center, "nonexistent word");

        // Trim: "Иван Анна" с max_nodes=1 → только 1 узел (топ по степени).
        let sg_trim = kb.retrieve_for_question("Иван Анна", 1);
        assert_eq!(sg_trim.nodes.len(), 1, "trim до 1 узла");
        // Самый высокосвязный в объединённом подграфе — ivan (3 ребра).
        assert_eq!(sg_trim.nodes[0].id, "ivan");
        // Рёбер не должно остаться (нужно 2 узла для ребра).
        assert!(sg_trim.edges.is_empty(), "при 1 узле рёбер быть не должно");
    }

    // ── Дополнительные smoke-тесты ────────────────────────────────────

    #[test]
    fn test_subgraph_for_nonexistent_center_is_empty_or_self_only() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);

        // center не существует ни в nodes, ни в adjacency, ни в facts.
        let sg = kb.subgraph("nonexistent", 2);
        assert!(sg.nodes.is_empty(), "для несуществующего center нет узлов");
        assert!(sg.edges.is_empty());
        assert!(sg.facts.is_empty());
        assert!(sg.events.is_empty());
        assert_eq!(sg.center, "nonexistent");
        assert!(sg.is_empty());
    }

    #[test]
    fn test_pluralize_ru_forms() {
        // one (1, 21, 31, ...).
        assert_eq!(pluralize_ru(1, "прыжок", "прыжка", "прыжков"), "прыжок");
        assert_eq!(pluralize_ru(21, "прыжок", "прыжка", "прыжков"), "прыжок");
        assert_eq!(pluralize_ru(31, "прыжок", "прыжка", "прыжков"), "прыжок");

        // few (2-4, 22-24, ...).
        assert_eq!(pluralize_ru(2, "прыжок", "прыжка", "прыжков"), "прыжка");
        assert_eq!(pluralize_ru(3, "прыжок", "прыжка", "прыжков"), "прыжка");
        assert_eq!(pluralize_ru(4, "прыжок", "прыжка", "прыжков"), "прыжка");
        assert_eq!(pluralize_ru(22, "прыжок", "прыжка", "прыжков"), "прыжка");

        // many (0, 5-19, 20, ...).
        assert_eq!(pluralize_ru(0, "прыжок", "прыжка", "прыжков"), "прыжков");
        assert_eq!(pluralize_ru(5, "прыжок", "прыжка", "прыжков"), "прыжков");
        assert_eq!(pluralize_ru(11, "прыжок", "прыжка", "прыжков"), "прыжков");
        assert_eq!(pluralize_ru(12, "прыжок", "прыжка", "прыжков"), "прыжков");
        assert_eq!(pluralize_ru(14, "прыжок", "прыжка", "прыжков"), "прыжков");
        assert_eq!(pluralize_ru(20, "прыжок", "прыжка", "прыжков"), "прыжков");
        assert_eq!(pluralize_ru(100, "прыжок", "прыжка", "прыжков"), "прыжков");
    }

    #[test]
    fn test_subgraph_serializes_to_json() {
        let (project, facts) = build_fixture();
        let kb = KnowledgeBase::from_project(&project, facts);
        let sg = kb.subgraph("ivan", 2);

        // Subgraph должен сериализоваться в JSON (derive Serialize).
        let json = serde_json::to_string(&sg).expect("Subgraph должен сериализоваться");
        assert!(json.contains("ivan"));
        assert!(json.contains("max_hops"));
        assert!(json.contains("nodes"));

        // И десериализоваться обратно.
        let back: Subgraph =
            serde_json::from_str(&json).expect("Subgraph должен десериализоваться");
        assert_eq!(back.center, "ivan");
        assert_eq!(back.nodes.len(), sg.nodes.len());
        assert_eq!(back.max_hops, 2);
    }
}
