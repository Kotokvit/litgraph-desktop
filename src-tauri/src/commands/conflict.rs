//! Конфликт-граф: SVO → J-матрица → directed graph агрессоров/жертв.
//!
//! Tauri-команда `get_conflict_graph(text)` запускает Python-пайплайн:
//!   1. NER (извлечение персонажей через spaCy)
//!   2. SVO (Subject-Verb-Object triplets через dependency parsing)
//!   3. J-matrix builder (антисимметричная матрица направленных взаимодействий)
//!
//! Возвращает структуру ConflictGraph с узлами-персонажами (разделёнными на
//! агрессоров/жертв/нейтралов по netAggression) и направленными рёбрами
//! (кто → кого, с весом, глаголами и контекстом предложения).
//!
//! Используется UI-компонентом ConflictGraphDialog для рендера "рентгена
//! конфликта" прямо в приложении.

use serde::{Deserialize, Serialize};

use crate::commands::ner::run_python_with_text_file;

/// Роль персонажа в конфликте, вычисленная по netAggression (сумма строки
/// J-матрицы): > 0 — агрессор, < 0 — жертва, ≈ 0 — нейтрал.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConflictRole {
    Aggressor,
    Victim,
    Neutral,
}

/// Узел конфликт-графа — персонаж с агрегированными метриками.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictNode {
    /// Lemma персонажа (например, "Алексей", "Марина Игоревна").
    pub character: String,
    /// Суммарный вес исходящих действий (aggression out).
    pub outgoing: f64,
    /// Суммарный вес входящих действий (aggression in).
    pub incoming: f64,
    /// out − in: +агрессор, −жертва.
    pub balance: f64,
    /// Классификация по balance.
    pub role: ConflictRole,
}

/// Направленное ребро: subject → object с весом и контекстом.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictEdge {
    /// Кто действовал (subjectLemma).
    #[serde(rename = "from")]
    pub from_char: String,
    /// На ком/чём действовали (objectLemma).
    #[serde(rename = "to")]
    pub to_char: String,
    /// Суммарный вес (по полярности и negated-флагу).
    pub weight: f64,
    /// Число сыгравших глаголов.
    #[serde(rename = "verbCount")]
    pub verb_count: usize,
    /// Список уникальных лемм глаголов.
    pub verbs: Vec<String>,
    /// Полярность действия: negative / positive / neutral.
    pub polarity: String,
    /// true если было negation ("не остановил").
    pub negated: bool,
    /// true если объект был pronoun-ом (его/её/их), разрешённым в PER.
    #[serde(rename = "pronounResolved")]
    pub pronoun_resolved: bool,
    /// Контекст предложения (обрезан 200 символов).
    pub sentence: String,
}

/// Сводная статистика по графу.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictStats {
    #[serde(rename = "nodeCount")]
    pub node_count: usize,
    #[serde(rename = "edgeCount")]
    pub edge_count: usize,
    #[serde(rename = "rawTripletCount")]
    pub raw_triplet_count: usize,
    /// [(character, balance)] отсортированный DESC — главные агрессоры.
    pub aggressors: Vec<(String, f64)>,
    /// [(character, balance)] отсортированный ASC — главные жертвы.
    pub victims: Vec<(String, f64)>,
    /// Персонажи с |balance| < 0.1.
    pub neutral: Vec<String>,
}

/// Полный ответ команды.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictGraph {
    pub nodes: Vec<ConflictNode>,
    pub edges: Vec<ConflictEdge>,
    /// Антисимметричная матрица J[i,j] = +w, J[j,i] = -w.
    pub matrix: Vec<Vec<f64>>,
    /// Исходный порядок узлов (для индексации в matrix).
    #[serde(rename = "nodeOrder")]
    pub node_order: Vec<String>,
    pub stats: ConflictStats,
    pub model: String,
    pub version: String,
    #[serde(rename = "svoVersion")]
    pub svo_version: String,
    #[serde(rename = "textLength")]
    pub text_length: usize,
}

/// Tauri команда: построить конфликт-граф из текста.
///
/// Запускает conflict_graph.py, который внутри себя вызывает:
///   1. extract_svo() из svo_extract.py (NER + SVO-триплеты)
///   2. build_j_matrix() из conflict_graph.py (J-матрица + агрегация рёбер)
///
/// Возвращает типизированный ConflictGraph для фронтенда.
#[tauri::command]
pub async fn get_conflict_graph(text: String) -> Result<ConflictGraph, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    let script = include_str!("../../python/conflict_graph.py");
    // conflict_graph.py импортирует svo_extract, который в свою очередь
    // импортирует ner_extract — кладём все файлы рядом.
    // Phase 1B: v2 (ner_extract_v2.py) копируется под именем ner_extract.py
    // (контракт имени модуля сохранён для conflict_graph.py и svo_extract.py).
    let ner_script = include_str!("../../python/ner_extract_v2.py");
    let svo_script = include_str!("../../python/svo_extract.py");
    let person_script = include_str!("../../../scripts/dev/grammar/person.py");
    let extra_files = vec![
        ("ner_extract.py", ner_script),  // v2 под именем v1
        ("svo_extract.py", svo_script),
        ("person.py", person_script),    // v2 зависит от person.py
    ];
    let stdout = run_python_with_text_file(script, &text, &extra_files)?;

    // Сначала пробуем распарсить как ConflictGraph.
    let result: ConflictGraph = serde_json::from_str(&stdout).map_err(|e| {
        // Если Python упал с error-полем — покажем его пользователю.
        let trimmed = &stdout[..stdout.len().min(800)];
        format!(
            "Не удалось распарсить JSON конфликт-графа: {}.\n\
             Первые 800 символов вывода: {}",
            e, trimmed
        )
    })?;

    Ok(result)
}
