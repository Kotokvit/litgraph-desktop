//! # Causality — распространение причинно-следственных связей
//!
//! Этот модуль реализует **движок каузальной пропагации** в нарративном графе.
//!
//! В LitGraph рёбра `cause` связывают события: `Event_A → cause → Event_B`
//! означает «A породило B». Когда факт утверждается или ретракнится,
//! causality-движок распространяет это изменение по цепочке: каждое событие,
//! причинно вытекающее из изменённого, тоже может потребовать пересмотра.
//!
//! ## Возможности
//!
//! - Построение [`CausalityEngine`] из существующего графа рёбер
//!   ([`CausalityEngine::from_edges`]) и журнала фактов.
//! - Прямые запросы: кто caused данное событие ([`direct_causes_of`]) и что
//!   оно само caused ([`direct_effects_of`]).
//! - Транзитивное замыкание вверх ([`transitive_causes`]) и вниз
//!   ([`transitive_effects`]) по цепочке, с защитой от циклов.
//! - Обнаружение петель ([`detect_causal_loops`]) через DFS с маркерами
//!   `visiting`/`visited`.
//! - Поиск кратчайшего причинного пути между двумя событиями
//!   ([`explain_chain`]) через BFS.
//!
//! ## Принцип
//!
//! Движок полностью синхронный и детерминированный. Никаких LLM-вызовов,
//! никакого `tokio`, никаких `unwrap()` на данных извне. Все алгоритмы
//! устойчивы к циклам в графе (защита через `HashSet<EventId>`).
//!
//! См. `docs/reasoning/SPEC.md` §2.10 (`CausalLoop`) и §1 (карта модулей).
//!
//! [`direct_causes_of`]: CausalityEngine::direct_causes_of
//! [`direct_effects_of`]: CausalityEngine::direct_effects_of
//! [`transitive_causes`]: CausalityEngine::transitive_causes
//! [`transitive_effects`]: CausalityEngine::transitive_effects
//! [`detect_causal_loops`]: CausalityEngine::detect_causal_loops
//! [`explain_chain`]: CausalityEngine::explain_chain

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::models::LitEdge;
use crate::reasoning::facts::{EventId, FactLog};
use crate::reasoning::timeline::TemporalAnchor;

/// Одно направленное причинно-следственное ребро между двумя событиями:
/// `cause_event_id` породило `effect_event_id`.
///
/// В отличие от «сырого» `LitEdge` (где source/target — это ID узлов графа),
/// здесь обе стороны — уже ID событий из [`FactLog`]. Преобразование
/// выполняется в [`CausalityEngine::from_edges`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalLink {
    /// Событие-причина.
    pub cause_event_id: EventId,
    /// Событие-следствие.
    pub effect_event_id: EventId,
    /// Человекочитаемое описание связи («Пётр убил Анну → Анна умерла»).
    pub description: String,
}

/// Замкнутая причинно-следственная петля: A → B → C → A.
///
/// Поле `chain` содержит последовательность EventId, замыкаясь последним
/// элементом (равным первому). Например, для петли A→B→C→A `chain` равен
/// `[A, B, C, A]`.
///
/// Этот тип переэкспортируется позже модулем `contradictions.rs` (см.
/// SPEC §2.10), поэтому объявлен `pub`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalLoop {
    /// Цепочка EventId, замыкающаяся на первом элементе.
    pub chain: Vec<EventId>,
    /// Человекочитаемое описание петли.
    pub description: String,
}

/// Движок каузальной пропагации. Хранит набор [`CausalLink`] и предоставляет
/// алгоритмы обхода/поиска/обнаружения петель.
///
/// Инварианты:
/// - `links` может содержать дубликаты (это допустимо — они не ломают
///   алгоритмы, т.к. везде используются `HashSet` для посещённых узлов).
/// - Self-loops (cause == effect) технически допустимы и считаются петлёй
///   длины 1 (chain = [X, X]).
pub struct CausalityEngine {
    links: Vec<CausalLink>,
}

impl CausalityEngine {
    /// Создать пустой движок.
    pub fn new() -> Self {
        Self { links: Vec::new() }
    }

    /// Построить движок из рёбер графа и журнала фактов.
    ///
    /// # Алгоритм
    ///
    /// 1. Перебираем все `edges`. Нас интересуют только те, у которых
    ///    `data.kind == Some("cause")`.
    /// 2. Для каждого такого ребра `source` и `target` — это ID узлов графа
    ///    (EntityId == LitNode.id). Нужно сопоставить их с EventId.
    /// 3. Ищем в `facts.all_events()` событие, чей `actor` или `target`
    ///    совпадает с узлом. Если таких событий несколько — берём самое
    ///    раннее по `time` (TemporalAnchor ordering, см. SPEC §2.2).
    /// 4. Если для `source` или `target` не нашлось события — ребро
    ///    пропускается (мы не можем построить CausalLink без EventId).
    /// 5. `description` берётся из `data.note` ребра; если note нет —
    ///    формируется шаблонная строка «Причинно-следственная связь:
    ///    {source} → {target}».
    pub fn from_edges(edges: &[LitEdge], facts: &FactLog) -> Self {
        let mut links = Vec::new();

        for edge in edges {
            // Фильтруем только cause-рёбра.
            let is_cause = edge
                .data
                .as_ref()
                .and_then(|d| d.kind.as_deref())
                .map(|k| k == "cause")
                .unwrap_or(false);
            if !is_cause {
                continue;
            }

            // Сопоставляем source → EventId (самое раннее событие, где
            // узел выступает actor или target).
            let cause_event_id = match find_earliest_event_for_entity(facts, &edge.source) {
                Some(id) => id,
                None => continue,
            };
            let effect_event_id = match find_earliest_event_for_entity(facts, &edge.target) {
                Some(id) => id,
                None => continue,
            };

            // Описание: prefer note из ребра, иначе — шаблонная строка.
            let description = edge
                .data
                .as_ref()
                .and_then(|d| d.note.clone())
                .unwrap_or_else(|| {
                    format!(
                        "Причинно-следственная связь: {} → {}",
                        edge.source, edge.target
                    )
                });

            links.push(CausalLink {
                cause_event_id,
                effect_event_id,
                description,
            });
        }

        Self { links }
    }

    /// Добавить готовую причинно-следственную связь в движок.
    pub fn add_link(&mut self, link: CausalLink) {
        self.links.push(link);
    }

    /// Все связи в порядке вставки.
    pub fn links(&self) -> &[CausalLink] {
        &self.links
    }

    /// Прямые причины события: все связи, у которых `effect_event_id == event_id`.
    ///
    /// Возвращает «что caused данное событие».
    pub fn direct_causes_of(&self, event_id: EventId) -> Vec<&CausalLink> {
        self.links
            .iter()
            .filter(|l| l.effect_event_id == event_id)
            .collect()
    }

    /// Прямые следствия события: все связи, у которых `cause_event_id == event_id`.
    ///
    /// Возвращает «что caused данное событие».
    pub fn direct_effects_of(&self, event_id: EventId) -> Vec<&CausalLink> {
        self.links
            .iter()
            .filter(|l| l.cause_event_id == event_id)
            .collect()
    }

    /// Транзитивное замыкание вверх: все события, которые (прямо или косвенно)
    /// причинно предшествуют `event_id`.
    ///
    /// Сам `event_id` в результат не входит. Алгоритм — рекурсивный DFS с
    /// `visited: HashSet<EventId>` для защиты от бесконечного цикла при
    /// наличии петель в графе.
    pub fn transitive_causes(&self, event_id: EventId) -> Vec<EventId> {
        let mut visited: HashSet<EventId> = HashSet::new();
        // Стартовый узел сразу помечаем посещённым — он не должен попасть
        // в свой собственный список причин, даже если в графе есть петля
        // A→B→A (тогда результат для A = [B], без A).
        visited.insert(event_id);
        let mut result: Vec<EventId> = Vec::new();
        self.walk_causes(event_id, &mut visited, &mut result);
        result
    }

    /// Транзитивное замыкание вниз: все события, которые (прямо или косвенно)
    /// причинно вытекают из `event_id`.
    ///
    /// Сам `event_id` в результат не входит. Алгоритм — рекурсивный DFS с
    /// `visited: HashSet<EventId>` для защиты от петель.
    pub fn transitive_effects(&self, event_id: EventId) -> Vec<EventId> {
        let mut visited: HashSet<EventId> = HashSet::new();
        visited.insert(event_id);
        let mut result: Vec<EventId> = Vec::new();
        self.walk_effects(event_id, &mut visited, &mut result);
        result
    }

    /// Обнаружить все причинно-следственные петли в графе.
    ///
    /// Алгоритм: DFS с двумя маркерами:
    /// - `visiting` — узлы на текущем пути рекурсии (back-edge → цикл).
    /// - `visited` — узлы, полностью обработанные (не нужно повторять).
    ///
    /// Когда DFS встречает узел, уже находящийся в `visiting` — найден цикл.
    /// Цепочка извлекается из стека рекурсии, начиная с позиции повторного
    /// узла, и замыкается добавлением этого узла в конец.
    ///
    /// Узлы перебираются в отсортированном порядке (по EventId), чтобы
    /// результат был детерминированным — иначе порядок `HashSet`-итератора
    /// делал бы тесты недетерминированными.
    pub fn detect_causal_loops(&self) -> Vec<CausalLoop> {
        // Собираем все уникальные узлы графа.
        let mut all_nodes: Vec<EventId> = Vec::new();
        for link in &self.links {
            all_nodes.push(link.cause_event_id);
            all_nodes.push(link.effect_event_id);
        }
        all_nodes.sort_unstable();
        all_nodes.dedup();

        let mut visiting: HashSet<EventId> = HashSet::new();
        let mut visited: HashSet<EventId> = HashSet::new();
        let mut stack: Vec<EventId> = Vec::new();
        let mut loops: Vec<CausalLoop> = Vec::new();

        for &start in &all_nodes {
            if visited.contains(&start) {
                continue;
            }
            self.dfs_cycle(
                start,
                &mut visiting,
                &mut visited,
                &mut stack,
                &mut loops,
            );
        }

        loops
    }

    /// Найти кратчайший причинный путь от `from` к `to` (по направлению рёбер).
    ///
    /// Возвращает `None`, если:
    /// - `from == to` (путь к самому себе не имеет смысла);
    /// - пути не существует;
    /// - путь длиннее 20 хопов (защита от runaway-поиска).
    ///
    /// Алгоритм: BFS с очередью `VecDeque`, предками через `HashMap<EventId,
    /// EventId>`. После нахождения `to` путь восстанавливается обратным
    /// проходом по предкам.
    pub fn explain_chain(&self, from: EventId, to: EventId) -> Option<Vec<EventId>> {
        if from == to {
            return None;
        }

        const MAX_HOPS: usize = 20;

        let mut queue: VecDeque<(EventId, usize)> = VecDeque::new();
        let mut parent: HashMap<EventId, EventId> = HashMap::new();
        let mut visited: HashSet<EventId> = HashSet::new();

        queue.push_back((from, 0));
        visited.insert(from);

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= MAX_HOPS {
                // Достигли лимита глубины — не расширяем дальше.
                continue;
            }

            // Перебираем все рёбра, исходящие из current (по направлению
            // причинности: cause → effect).
            for link in &self.links {
                if link.cause_event_id != current {
                    continue;
                }
                let next = link.effect_event_id;
                if visited.contains(&next) {
                    continue;
                }
                visited.insert(next);
                parent.insert(next, current);

                if next == to {
                    // Восстанавливаем путь от to обратно к from.
                    let mut path: Vec<EventId> = vec![to];
                    let mut cursor = to;
                    while let Some(&p) = parent.get(&cursor) {
                        path.push(p);
                        cursor = p;
                    }
                    path.reverse();
                    return Some(path);
                }

                queue.push_back((next, depth + 1));
            }
        }

        None
    }

    // ── Внутренние рекурсивные помощники ───────────────────────────────────

    /// Рекурсивный обход вверх (causes). Для каждого прямого cause события
    /// `event_id` добавляет его в `result` (если ещё не посещён) и идёт
    /// дальше от него вверх.
    fn walk_causes(
        &self,
        event_id: EventId,
        visited: &mut HashSet<EventId>,
        result: &mut Vec<EventId>,
    ) {
        for link in &self.links {
            if link.effect_event_id != event_id {
                continue;
            }
            let cause = link.cause_event_id;
            if visited.contains(&cause) {
                continue;
            }
            visited.insert(cause);
            result.push(cause);
            self.walk_causes(cause, visited, result);
        }
    }

    /// Рекурсивный обход вниз (effects). Симметричен `walk_causes`.
    fn walk_effects(
        &self,
        event_id: EventId,
        visited: &mut HashSet<EventId>,
        result: &mut Vec<EventId>,
    ) {
        for link in &self.links {
            if link.cause_event_id != event_id {
                continue;
            }
            let effect = link.effect_event_id;
            if visited.contains(&effect) {
                continue;
            }
            visited.insert(effect);
            result.push(effect);
            self.walk_effects(effect, visited, result);
        }
    }

    /// DFS-обход для поиска петель. При входе в узел он добавляется в
    /// `visiting` и в `stack`; при выходе — удаляется оттуда и
    /// добавляется в `visited`. Если сосед уже в `visiting` — найден цикл.
    fn dfs_cycle(
        &self,
        node: EventId,
        visiting: &mut HashSet<EventId>,
        visited: &mut HashSet<EventId>,
        stack: &mut Vec<EventId>,
        loops: &mut Vec<CausalLoop>,
    ) {
        // Узел уже полностью обработан — пропускаем.
        if visited.contains(&node) {
            return;
        }
        // Узел в текущем пути рекурсии — нашли цикл.
        if visiting.contains(&node) {
            if let Some(idx) = stack.iter().position(|&n| n == node) {
                // Цепочка цикла: от первого вхождения node до конца стека,
                // плюс node для замыкания (A → B → C → A).
                let mut chain: Vec<EventId> = stack[idx..].to_vec();
                chain.push(node);
                let description = format!(
                    "Обнаружена причинно-следственная петля длины {}: {:?}",
                    chain.len().saturating_sub(1),
                    chain
                );
                loops.push(CausalLoop { chain, description });
            }
            return;
        }

        visiting.insert(node);
        stack.push(node);

        // Обходим всех прямых потомков (effects).
        for link in &self.links {
            if link.cause_event_id == node {
                self.dfs_cycle(
                    link.effect_event_id,
                    visiting,
                    visited,
                    stack,
                    loops,
                );
            }
        }

        stack.pop();
        visiting.remove(&node);
        visited.insert(node);
    }
}

impl Default for CausalityEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Внутренние хелперы
// ──────────────────────────────────────────────────────────────────────────

/// Найти ID самого раннего события в `facts`, где `entity_id` выступает
/// в роли `actor` или `target`. Если несколько событий подходят, выбирается
/// то, у которого `time` (TemporalAnchor) наименьший.
///
/// Возвращает `None`, если ни одного события не нашлось.
fn find_earliest_event_for_entity(facts: &FactLog, entity_id: &str) -> Option<EventId> {
    // Храним пару (EventId, TemporalAnchor) — anchor нужен для сравнения.
    let mut best: Option<(EventId, TemporalAnchor)> = None;
    for event in facts.all_events() {
        let matches = event.actor == entity_id
            || event.target.as_deref() == Some(entity_id);
        if !matches {
            continue;
        }
        match &best {
            None => {
                best = Some((event.id, event.time.clone()));
            }
            Some((_, best_time)) => {
                // before() = строго раньше. При равных временных метках
                // сохраняем первый встретившийся (стабильность сортировки).
                if event.time.before(best_time) {
                    best = Some((event.id, event.time.clone()));
                }
            }
        }
    }
    best.map(|(id, _)| id)
}

// ──────────────────────────────────────────────────────────────────────────
// Тесты
// ──────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Хелпер: создать CausalLink с пустым описанием.
    fn link(cause: EventId, effect: EventId) -> CausalLink {
        CausalLink {
            cause_event_id: cause,
            effect_event_id: effect,
            description: String::new(),
        }
    }

    /// Хелпер: создать CausalLink с описанием.
    fn link_with_desc(cause: EventId, effect: EventId, desc: &str) -> CausalLink {
        CausalLink {
            cause_event_id: cause,
            effect_event_id: effect,
            description: desc.to_string(),
        }
    }

    #[test]
    fn test_add_and_query_direct_links() {
        let mut engine = CausalityEngine::new();
        // Граф: 1 → 2, 1 → 3, 2 → 4.
        engine.add_link(link(1, 2));
        engine.add_link(link(1, 3));
        engine.add_link(link(2, 4));

        // Прямые причины события 2: только связь 1 → 2.
        let causes_of_2 = engine.direct_causes_of(2);
        assert_eq!(causes_of_2.len(), 1, "у события 2 должна быть 1 причина");
        assert_eq!(causes_of_2[0].cause_event_id, 1);
        assert_eq!(causes_of_2[0].effect_event_id, 2);

        // Прямые следствия события 1: 2 и 3.
        let effects_of_1 = engine.direct_effects_of(1);
        assert_eq!(effects_of_1.len(), 2, "у события 1 должно быть 2 следствия");
        let mut effects: Vec<EventId> = effects_of_1
            .iter()
            .map(|l| l.effect_event_id)
            .collect();
        effects.sort_unstable();
        assert_eq!(effects, vec![2, 3]);

        // У события 4 нет следствий.
        assert!(
            engine.direct_effects_of(4).is_empty(),
            "у события 4 не должно быть следствий"
        );
        // У события 1 нет причин.
        assert!(
            engine.direct_causes_of(1).is_empty(),
            "у события 1 не должно быть причин"
        );

        // Проверка links() — все 3 связи на месте.
        assert_eq!(engine.links().len(), 3);
    }

    #[test]
    fn test_transitive_causes_walks_upstream() {
        let mut engine = CausalityEngine::new();
        // Граф: 1 → 2 → 3 → 4. Спрашиваем транзитивные причины для 4.
        engine.add_link(link(1, 2));
        engine.add_link(link(2, 3));
        engine.add_link(link(3, 4));

        let causes = engine.transitive_causes(4);
        // Ожидаем: {1, 2, 3} (само 4 не входит).
        assert_eq!(causes.len(), 3, "транзитивные причины 4: {{1,2,3}}");
        let mut sorted = causes.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![1, 2, 3]);

        // Для корневого события (1) причин нет.
        let root_causes = engine.transitive_causes(1);
        assert!(root_causes.is_empty(), "у события 1 нет причин");
    }

    #[test]
    fn test_transitive_effects_walks_downstream() {
        let mut engine = CausalityEngine::new();
        // Граф: 1 → 2 → 3, 1 → 4. Спрашиваем транзитивные следствия для 1.
        engine.add_link(link(1, 2));
        engine.add_link(link(2, 3));
        engine.add_link(link(1, 4));

        let effects = engine.transitive_effects(1);
        // Ожидаем: {2, 3, 4} (само 1 не входит).
        assert_eq!(effects.len(), 3, "транзитивные следствия 1: {{2,3,4}}");
        let mut sorted = effects.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![2, 3, 4]);

        // Листовой узел (3) не имеет следствий.
        let leaf_effects = engine.transitive_effects(3);
        assert!(leaf_effects.is_empty(), "у события 3 нет следствий");
    }

    #[test]
    fn test_detect_causal_loops_finds_simple_cycle() {
        // A → B → C → A (где A=1, B=2, C=3).
        let mut engine = CausalityEngine::new();
        engine.add_link(link(1, 2)); // A → B
        engine.add_link(link(2, 3)); // B → C
        engine.add_link(link(3, 1)); // C → A

        let loops = engine.detect_causal_loops();
        assert_eq!(
            loops.len(),
            1,
            "должна быть обнаружена ровно 1 петля (A→B→C→A)"
        );

        let chain = &loops[0].chain;
        // Длина цепочки = 4 (3 уникальных + 1 замыкающий).
        assert_eq!(
            chain.len(),
            4,
            "цепочка должна иметь 4 элемента: [A, B, C, A], получено {:?}",
            chain
        );
        // Первый и последний элементы совпадают (замыкание).
        assert_eq!(
            chain.first(),
            chain.last(),
            "цепочка должна замыкаться: first == last"
        );
        // Все три узла (1, 2, 3) присутствуют в уникальной части.
        let unique: HashSet<EventId> = chain.iter().copied().collect();
        assert!(unique.contains(&1), "цепочка должна содержать A=1");
        assert!(unique.contains(&2), "цепочка должна содержать B=2");
        assert!(unique.contains(&3), "цепочка должна содержать C=3");
        // Описание не пустое.
        assert!(!loops[0].description.is_empty());
    }

    #[test]
    fn test_detect_causal_loops_returns_empty_for_acyclic() {
        // Линейный ацикличный граф: 1 → 2 → 3 → 4.
        let mut engine = CausalityEngine::new();
        engine.add_link(link(1, 2));
        engine.add_link(link(2, 3));
        engine.add_link(link(3, 4));

        let loops = engine.detect_causal_loops();
        assert!(
            loops.is_empty(),
            "ацикличный граф не должен содержать петель, получено: {:?}",
            loops
        );

        // Дополнительная проверка: «ромб» 1 → 2,3 → 4 (тоже ацикличный).
        let mut engine2 = CausalityEngine::new();
        engine2.add_link(link(1, 2));
        engine2.add_link(link(1, 3));
        engine2.add_link(link(2, 4));
        engine2.add_link(link(3, 4));
        let loops2 = engine2.detect_causal_loops();
        assert!(loops2.is_empty(), "ромб без петель: {:?}", loops2);
    }

    #[test]
    fn test_explain_chain_finds_shortest_path() {
        // Граф с двумя путями 1 → 5:
        //   длинный:  1 → 2 → 4 → 5   (3 хопа)
        //   короткий: 1 → 3 → 5       (2 хопа)
        let mut engine = CausalityEngine::new();
        engine.add_link(link(1, 2));
        engine.add_link(link(2, 4));
        engine.add_link(link(4, 5));
        engine.add_link(link(1, 3));
        engine.add_link(link(3, 5));

        let path = engine
            .explain_chain(1, 5)
            .expect("путь от 1 к 5 должен существовать");

        // Кратчайший путь: [1, 3, 5] (длина 2 хопа, 3 узла).
        assert_eq!(
            path,
            vec![1, 3, 5],
            "ожидался кратчайший путь [1, 3, 5], получено {:?}",
            path
        );

        // from == to → None.
        assert!(
            engine.explain_chain(1, 1).is_none(),
            "from == to должно возвращать None"
        );

        // Путь к недостижимому узлу → None.
        assert!(
            engine.explain_chain(5, 1).is_none(),
            "обратного пути 5 → 1 не существует"
        );

        // Путь к несуществующему узлу → None.
        assert!(
            engine.explain_chain(1, 999).is_none(),
            "пути к несуществующему узлу быть не должно"
        );
    }

    #[test]
    fn test_transitive_causes_handles_cycles_safely() {
        // Граф с циклом: 1 → 2 → 3 → 1 (все три узла в петле),
        // плюс хвост 0 → 1.
        let mut engine = CausalityEngine::new();
        engine.add_link(link(0, 1));
        engine.add_link(link(1, 2));
        engine.add_link(link(2, 3));
        engine.add_link(link(3, 1)); // обратное ребро — петля

        // transitive_causes(1) должен завершиться, не зациклившись, и
        // вернуть {0, 2, 3} (само 1 не входит).
        let causes = engine.transitive_causes(1);
        let mut sorted = causes.clone();
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            vec![0, 2, 3],
            "транзитивные причины 1 с учётом цикла: {{0, 2, 3}}, получено {:?}",
            sorted
        );

        // Симметричная проверка для transitive_effects: 1 → {2, 3} (через
        // петлю возвращаемся в 1, но 1 исключён как стартовый узел).
        let effects = engine.transitive_effects(1);
        let mut sorted_eff = effects.clone();
        sorted_eff.sort_unstable();
        assert_eq!(
            sorted_eff,
            vec![2, 3],
            "транзитивные следствия 1 с учётом цикла: {{2, 3}}, получено {:?}",
            sorted_eff
        );

        // Цикл также должен быть обнаружен detect_causal_loops.
        let loops = engine.detect_causal_loops();
        assert_eq!(
            loops.len(),
            1,
            "должна быть обнаружена 1 петля (1→2→3→1)"
        );
    }

    // ── Дополнительные smoke-тесты (выходят за минимум 7) ─────────────────

    #[test]
    fn test_from_edges_extracts_cause_links() {
        use crate::models::{EdgeData, LitEdge};
        use crate::reasoning::facts::{Action, Event, Provenance};

        // Готовим FactLog с тремя событиями для узлов "alice", "bob", "carol".
        let mut log = FactLog::new();
        let t1 = TemporalAnchor::new(1);
        let t2 = TemporalAnchor::new(2);
        let t3 = TemporalAnchor::new(3);

        // alice действует в главе 1 (id=1).
        log.record_event(Event {
            id: 0,
            actor: "alice".to_string(),
            action: Action::Speak { topic: None },
            target: None,
            instrument: None,
            time: t1.clone(),
            source_text: String::new(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        });
        // bob в главе 2 (id=2).
        log.record_event(Event {
            id: 0,
            actor: "bob".to_string(),
            action: Action::Kill,
            target: Some("alice".to_string()),
            instrument: None,
            time: t2.clone(),
            source_text: String::new(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        });
        // carol в главе 3 (id=3). alice здесь — target, но событие более
        // позднее, поэтому для alice должно выбраться событие id=1.
        log.record_event(Event {
            id: 0,
            actor: "carol".to_string(),
            action: Action::Discover {
                fact: "truth".to_string(),
            },
            target: Some("alice".to_string()),
            instrument: None,
            time: t3.clone(),
            source_text: String::new(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        });

        // Рёбра: alice → bob (cause), bob → carol (cause), carol → dave (не cause).
        let edges = vec![
            LitEdge {
                id: "e1".to_string(),
                source: "alice".to_string(),
                target: "bob".to_string(),
                source_handle: None,
                target_handle: None,
                edge_type: None,
                animated: None,
                data: Some(EdgeData {
                    kind: Some("cause".to_string()),
                    note: Some("Элис спровоцировала Боба".to_string()),
                }),
            },
            LitEdge {
                id: "e2".to_string(),
                source: "bob".to_string(),
                target: "carol".to_string(),
                source_handle: None,
                target_handle: None,
                edge_type: None,
                animated: None,
                data: Some(EdgeData {
                    kind: Some("cause".to_string()),
                    note: None,
                }),
            },
            // Не cause — должно быть проигнорировано.
            LitEdge {
                id: "e3".to_string(),
                source: "carol".to_string(),
                target: "dave".to_string(),
                source_handle: None,
                target_handle: None,
                edge_type: None,
                animated: None,
                data: Some(EdgeData {
                    kind: Some("flow".to_string()),
                    note: None,
                }),
            },
            // cause, но target "dave" не имеет события в FactLog —
            // должно быть пропущено.
            LitEdge {
                id: "e4".to_string(),
                source: "alice".to_string(),
                target: "dave".to_string(),
                source_handle: None,
                target_handle: None,
                edge_type: None,
                animated: None,
                data: Some(EdgeData {
                    kind: Some("cause".to_string()),
                    note: None,
                }),
            },
        ];

        let engine = CausalityEngine::from_edges(&edges, &log);
        // Должно остаться 2 связи: alice→bob (id 1→2) и bob→carol (id 2→3).
        assert_eq!(
            engine.links().len(),
            2,
            "ожидалось 2 cause-связи после фильтрации"
        );

        // alice → bob: cause_event_id=1 (самое раннее для alice), effect=2.
        assert_eq!(engine.links()[0].cause_event_id, 1);
        assert_eq!(engine.links()[0].effect_event_id, 2);
        assert_eq!(
            engine.links()[0].description,
            "Элис спровоцировала Боба",
            "note из ребра должен попасть в description"
        );

        // bob → carol: cause=2, effect=3.
        assert_eq!(engine.links()[1].cause_event_id, 2);
        assert_eq!(engine.links()[1].effect_event_id, 3);
        // description = шаблонная строка (note был None).
        assert!(
            engine.links()[1].description.contains("bob")
                && engine.links()[1].description.contains("carol"),
            "шаблонная description должна упоминать source/target: {}",
            engine.links()[1].description
        );
    }

    #[test]
    fn test_default_is_empty() {
        let engine = CausalityEngine::default();
        assert!(engine.links().is_empty());
        assert!(engine.direct_causes_of(1).is_empty());
        assert!(engine.transitive_causes(1).is_empty());
        assert!(engine.detect_causal_loops().is_empty());
        assert!(engine.explain_chain(1, 2).is_none());
    }

    #[test]
    fn test_self_loop_is_detected() {
        // Self-loop: 1 → 1. Это вырожденная петля длины 1.
        let mut engine = CausalityEngine::new();
        engine.add_link(link(1, 1));

        let loops = engine.detect_causal_loops();
        assert_eq!(loops.len(), 1, "self-loop должен быть обнаружен");
        assert_eq!(loops[0].chain, vec![1, 1]);
    }

    #[test]
    fn test_explain_chain_respects_max_hops() {
        // Длинная цепочка из 25 узлов: 1 → 2 → ... → 25.
        // explain_chain(1, 25) не должен найти путь (25 хопов > MAX_HOPS=20).
        let mut engine = CausalityEngine::new();
        for i in 1..25u64 {
            engine.add_link(link(i, i + 1));
        }

        // 21 → 25 (4 хопа) — должно находиться.
        let short = engine.explain_chain(21, 25);
        assert!(
            short.is_some(),
            "путь 21 → 25 (4 хопа) должен находиться"
        );
        assert_eq!(short.unwrap(), vec![21, 22, 23, 24, 25]);

        // 1 → 25 (24 хопа) — не должно (превышает MAX_HOPS=20).
        let long = engine.explain_chain(1, 25);
        assert!(
            long.is_none(),
            "путь 1 → 25 (24 хопа) не должен находиться из-за лимита в 20 хопов"
        );

        // 1 → 21 (20 хопов) — граничный случай, должен находиться (depth=20 < MAX_HOPS=20, то есть
        // узел 21 добавляется в очередь на depth=20, но не расширяется — это ОК,
        // т.к. целевой узел не нужно расширять).
        let boundary = engine.explain_chain(1, 21);
        assert!(
            boundary.is_some(),
            "путь 1 → 21 (ровно 20 хопов) должен находиться на границе лимита"
        );
    }

    #[test]
    fn test_link_description_round_trips_through_serde() {
        let link = link_with_desc(7, 42, "Пётр убил Анну — это породило её смерть");
        let json = serde_json::to_string(&link).expect("serde_json должен сериализовать");
        let back: CausalLink =
            serde_json::from_str(&json).expect("serde_json должен десериализовать");
        assert_eq!(back.cause_event_id, 7);
        assert_eq!(back.effect_event_id, 42);
        assert_eq!(back.description, "Пётр убил Анну — это породило её смерть");
    }

    #[test]
    fn test_multiple_independent_cycles_all_detected() {
        // Две независимые петли: 1→2→1 и 10→20→30→10.
        let mut engine = CausalityEngine::new();
        engine.add_link(link(1, 2));
        engine.add_link(link(2, 1));
        engine.add_link(link(10, 20));
        engine.add_link(link(20, 30));
        engine.add_link(link(30, 10));

        let loops = engine.detect_causal_loops();
        assert_eq!(loops.len(), 2, "должно быть обнаружено 2 независимые петли");

        // Проверяем, что обе петли найдены (без проверки порядка, т.к.
        // детерминизм гарантирует порядок по стартовому узлу — но всё равно
        // проверяем множество уникальных узлов).
        let all_unique_nodes: HashSet<EventId> = loops
            .iter()
            .flat_map(|l| l.chain.iter().copied())
            .collect();
        for node in [1u64, 2, 10, 20, 30] {
            assert!(
                all_unique_nodes.contains(&node),
                "узел {} должен быть в одной из петель",
                node
            );
        }
    }
}
