//! Семантический компилятор: текст → `Event` для Reasoning Engine.
//!
//! Этот модуль — мост между «сырым» слоем (Markdown + Python SVO JSON) и
//! «формальным» слоем фактов/событий. Он превращает триплеты Subject-Verb-Object,
//! которые находит Python-парсер `src-tauri/python/svo_extract.py`, в типизированные
//! [`crate::reasoning::facts::Event`] с правильным [`Action`], [`TemporalAnchor`]
//! и `actor`/`target`, привязанными к `LitNode.id`.
//!
//! # Два режима работы
//!
//! 1. **Основной (Python SVO)** — [`triplets_to_events`]. Принимает уже
//!    извлечённые SVO-триплеты (высокая достоверность, `confidence = 0.9`,
//!    `Provenance::SvoParser`). Это основной путь в production-сценарии.
//!
//! 2. **Резервный (Rust regex)** — [`parse_text_fallback`]. Если Python
//!    недоступен (нет spaCy, нет pymorphy3, нет интерпретатора), модуль
//!    пытается сам вытащить несколько ключевых русских глаголов (убил/сказал/
//!    умер/воскрес/пришёл) и построить из них события с `confidence = 0.5`,
//!    `Provenance::RustParser`. Это аварийный режим — полный SVO-анализ
//!    остаётся за Python.
//!
//! # Лексикон глаголов
//!
//! [`verb_to_action`] содержит статическую таблицу соответствий «русская лемма
//! глагола → [`Action`]». Леммы взяты из множеств `POSITIVE_VERBS`,
//! `NEGATIVE_VERBS`, `NEUTRAL_VERBS` в `svo_extract.py` (lines 90-150). Для
//! глаголов вне явной таблицы решение принимается по полярности: если лемма
//! лежит в одном из множеств — берётся соответствующая полярность; если лемма
//! совсем неизвестна — используется поле `polarity` из триплета (последний
//! уровень fallback).
//!
//! # EntityResolver
//!
//! Чтобы превратить «Иван» в `LitNode.id` (например, `"char-ivan-42"`),
//! используется [`EntityResolver`] — построенный один раз из списка узлов графа
//! хэш-таблица «lowercase lemma → id» с дополнительным индексом по `aliases`
//! и `forms` из `node.data.meta`. Поиск — точный, без fuzzy matching (по
//! SPEC §5: «determinism first»). Если имя не найдено — `resolve_or_keep`
//! возвращает исходную строку как «фантомную сущность», которую Wave 4
//! (`cycle.rs`) сможет позже разрешить или отбросить.
//!
//! # Связь с другими модулями
//!
//! - Импортирует [`Event`], [`Action`], [`Provenance`], [`VerbPolarity`],
//!   [`EventId`] из `facts.rs` (Wave 1).
//! - Импортирует [`TemporalAnchor`] из `timeline.rs` (Wave 1).
//! - Импортирует [`LitNode`] из `crate::models` (существующий слой данных).
//! - Импортирует [`ParsedChapter`] из `crate::parser::chapters` (существующий
//!   парсер глав — даёт `pos`/`end` для маппинга byte offset → chapter_num).
//! - Не импортирует и не `pub use`-ит ничего из других reasoning-модулей
//!   (SPEC §4.6 — предотвращение циклических зависимостей).
//!
//! # Детерминизм
//!
//! Все функции — синхронные, без tokio, без LLM, без внешнего состояния.
//! `EntityResolver` иммутабелен после построения. Тесты покрывают 9 сценариев
//! из brief'а (см. `mod tests`).

use std::collections::HashMap;

use fancy_regex::Regex;
use serde::{Deserialize, Serialize};

use crate::models::LitNode;
use crate::parser::chapters::ParsedChapter;
use crate::reasoning::facts::{Action, Event, EventId, Provenance, VerbPolarity};
use crate::reasoning::timeline::TemporalAnchor;

// ============ SvoTriplet — зеркало Python JSON shape ============

/// Один триплет SVO (Subject-Verb-Object), как его отдаёт Python-парсер
/// `svo_extract.py`.
///
/// Поля названы в snake_case (Rust convention), но (де)сериализуются в
/// camelCase (Python convention) через `#[serde(rename = "...")]`. Это
/// позволяет `serde_json::from_slice::<Vec<SvoTriplet>>` напрямую читать
/// JSON из stdout Python-парсера.
///
/// Все «опциональные» поля (gender, position, tense, polarity, negated,
/// pronoun_resolved) помечены `#[serde(default)]` — если Python-парсер не
/// заполнил их (старая версия, ошибка разбора), Rust-сторона получит
/// осмысленные дефолты (`None`/`0`/`""`/`false`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SvoTriplet {
    /// Исходная форма субъекта (как в тексте): «Раскольников», «Алёну».
    pub subject: String,
    /// Лемма субъекта: «Раскольников», «Алёна».
    #[serde(rename = "subjectLemma")]
    pub subject_lemma: String,
    /// Грамматический род субъекта (Python-side spaCy): `Some("Masc")`,
    /// `Some("Fem")`, `Some("Neut")`, `None` если неизвестен.
    #[serde(rename = "subjectGender", default)]
    pub subject_gender: Option<String>,
    /// Исходная форма глагола: «ударил».
    pub verb: String,
    /// Лемма глагола: «ударить». Ключевое поле для [`verb_to_action`].
    #[serde(rename = "verbLemma")]
    pub verb_lemma: String,
    /// Исходная форма объекта: «Алёну», «топором».
    pub object: String,
    /// Лемма объекта: «Алёна», «топор».
    #[serde(rename = "objectLemma")]
    pub object_lemma: String,
    /// Грамматический род объекта.
    #[serde(rename = "objectGender", default)]
    pub object_gender: Option<String>,
    /// Полное предложение, из которого извлечён триплет. Сохраняется в
    /// `Event.source_text` — нужно для audit trail и для LLM feedback.
    pub sentence: String,
    /// Байтовое смещение триплета в исходном тексте. Используется для
    /// построения [`TemporalAnchor`] (поиск главы через `ParsedChapter`).
    #[serde(default)]
    pub position: usize,
    /// Время глагола: «past», «pres», «fut», «unknown».
    #[serde(default)]
    pub tense: String,
    /// Полярность глагола: «positive», «negative», «neutral».
    /// Используется только если лемма не найдена в статическом лексиконе.
    #[serde(default)]
    pub polarity: String,
    /// `true` если при глаголе было отрицание «не/ни»: «не убил».
    /// Принимается в [`verb_to_action`] для потенциального flip'а полярности
    /// неизвестных глаголов; для известных лемм в текущей версии не влияет
    /// на выбор `Action` (семантика отрицания делегирована Wave 4 inference).
    #[serde(default)]
    pub negated: bool,
    /// `true` если Python-парсер заменил местоимение на конкретное имя
    /// (3-е лицо: «его» → «Ивана»). Поле только информационное — на
    /// маппинг в `Action` не влияет.
    #[serde(default)]
    pub pronoun_resolved: bool,
}

// ============ Лексикон глаголов: лемма → Action ============

/// Множество позитивных русских глаголов (мирные действия: помощь, любовь,
/// созидание). Зеркало `POSITIVE_VERBS` из `svo_extract.py` (lines 99-108).
const POSITIVE_VERBS: &[&str] = &[
    "любить", "помочь", "помогать", "спасти", "спасать", "защитить", "защищать",
    "обнять", "обнимать", "поцеловать", "целовать", "подарить", "дарить",
    "утешить", "утешать", "простить", "прощать", "поздравить", "похвалить",
    "наградить", "благословить", "вылечить", "лечить", "кормить", "накормить",
    "одеть", "успокоить", "радовать", "обрадовать", "восхищать", "восхитить",
    "пригласить", "встретить", "проводить", "навестить", "навещать",
    "согласиться", "поддержать", "верить", "доверять", "посочувствовать",
    "сочувствовать", "выслушать", "послушать", "ответить", "сказать", "молвить",
];

/// Множество негативных русских глаголов (насилие, ложь, разрушение).
/// Зеркало `NEGATIVE_VERBS` из `svo_extract.py` (lines 111-123).
const NEGATIVE_VERBS: &[&str] = &[
    "убить", "убивать", "ударить", "бить", "избить", "ранить", "ранять",
    "обидеть", "обижать", "оскорбить", "оскорблять", "предать", "предавать",
    "обмануть", "обманывать", "солгать", "лгать", "украсть", "красть",
    "разрушить", "разрушать", "сжечь", "поджечь", "отнять", "отнимать",
    "выгнать", "гонять", "прогнать", "наказать", "казнить", "пытать",
    "ненавидеть", "презирать", "проклясть", "проклинать", "угрожать",
    "напасть", "атаковать", "воевать", "бороться", "запретить", "запрещать",
    "запереть", "запирать", "арестовать", "судить", "осуждать", "осудить",
    "оттолкнуть", "толкать", "плакать", "страдать",
    "изменить", "изменять", "соблазнить", "соблазнять", "подкупить",
    "шантажировать", "давить", "подозревать", "обвинить", "обвинять",
];

/// Множество нейтральных русских глаголов (движение, коммуникация, восприятие).
/// Зеркало `NEUTRAL_VERBS` из `svo_extract.py` (lines 126-138).
const NEUTRAL_VERBS: &[&str] = &[
    "пойти", "идти", "прийти", "приходить", "уйти", "уходить", "выйти",
    "входить", "войти", "поехать", "ехать", "приехать", "уехать",
    "стоять", "сидеть", "лежать", "встать", "вставать", "опуститься",
    "посмотреть", "смотреть", "увидеть", "видеть", "найти", "искать",
    "взять", "дать", "получить", "передать", "принести", "унести",
    "открыть", "закрыть", "постучать", "позвать", "позвонить",
    "написать", "читать", "прочитать", "нарисовать", "сделать",
    "начать", "кончить", "продолжать", "остановиться", "вернуться",
    "познакомиться", "встретиться", "поговорить", "спросить", "ответить",
    "вспомнить", "забыть", "подумать", "понять", "узнать", "заметить",
    "почувствовать", "услышать", "понюхать", "попробовать",
];

/// Возвращает `true` если лемма входит в [`POSITIVE_VERBS`].
fn is_positive_verb(v: &str) -> bool {
    POSITIVE_VERBS.contains(&v)
}

/// Возвращает `true` если лемма входит в [`NEGATIVE_VERBS`].
fn is_negative_verb(v: &str) -> bool {
    NEGATIVE_VERBS.contains(&v)
}

/// Возвращает `true` если лемма входит в [`NEUTRAL_VERBS`].
fn is_neutral_verb(v: &str) -> bool {
    NEUTRAL_VERBS.contains(&v)
}

/// Преобразует лемму русского глагола в [`Action`] для `Event.action`.
///
/// Алгоритм (4 уровня fallback):
/// 1. **Явная таблица** — лемма в hardcoded `match` (53 уникальные леммы,
///    см. таблицу в brief'е). Если найдено — вернуть соответствующий `Action`.
/// 2. **Полярность из множеств** — лемма не в явной таблице, но лежит в
///    [`POSITIVE_VERBS`]/[`NEGATIVE_VERBS`]/[`NEUTRAL_VERBS`]. Вернуть
///    `Action::Custom { verb_lemma, polarity }`.
/// 3. **`polarity` из триплета** — лемма совсем неизвестна. Использовать поле
///    `polarity` ("positive"/"negative"/"neutral" из Python). Если `negated`
///    — флипнуть positive↔negative (neutral остаётся neutral). Вернуть
///    `Action::Custom`.
/// 4. **Total unknown** — `polarity` пуста или непонятна → neutral.
///
/// # Аргументы
/// - `verb_lemma` — лемма глагола из Python SVO (например, «убить»).
/// - `polarity` — полярность из триплета: «positive»/«negative»/«neutral»/«».
/// - `negated` — был ли при глаголе маркер отрицания «не/ни».
///
/// # Замечание о `negated`
///
/// Для **известных лемм** (явная таблица + множества) `negated` игнорируется:
/// «не убил» всё ещё маппится в `Action::Kill`, потому что правилами
/// инференса (rules.rs, Wave 2) должно заниматься отдельное правило
/// «если action=Kill и triplet.negated → не порождать факт alive=false».
/// Семантический эффект отрицания — responsibility Wave 4 (`cycle.rs`), не
/// семантического компилятора.
///
/// Для **полностью неизвестных глаголов** `negated` flip'ает полярность —
/// это минимальная семантика, которую мы можем дать на этом уровне.
pub fn verb_to_action(verb_lemma: &str, polarity: &str, negated: bool) -> Action {
    // Леммы в Python-side уже приходят в lowercase, но защищаемся на всякий.
    let v = verb_lemma.trim().to_lowercase();

    // 1) Явная таблица — 53 уникальные леммы.
    match v.as_str() {
        // ── Физическое насилие (летальное и нелетальное) ──
        "убить" | "убивать" | "казнить" => return Action::Kill,
        "ранить" | "ранять" => return Action::Wound,
        "ударить" | "бить" | "избить" | "пытать" => return Action::Hit,
        "арестовать" => return Action::Capture,
        "запереть" | "запирать" => return Action::Imprison,
        "вылечить" | "лечить" => return Action::Heal,

        // ── Перемещение (destination/source заполняются в populate_action_payload) ──
        "пойти" | "идти" | "прийти" | "приходить" | "поехать" | "ехать" | "приехать" => {
            return Action::Arrive {
                destination: String::new(),
            };
        }
        "уйти" | "уходить" | "выйти" | "уехать" => {
            return Action::Leave {
                source: String::new(),
            };
        }

        // ── Коммуникация ──
        "сказать" | "ответить" | "молвить" | "спросить" => {
            return Action::Speak { topic: None };
        }

        // ── Тактильный контакт / нежность ──
        // Task brief: «choose Touch to be safe» — не FallInLove.
        "обнять" | "обнимать" | "поцеловать" | "целовать" => return Action::Touch,

        // ── Прощение — позитивный, но не стандартный физический ──
        "простить" | "прощать" => {
            return Action::Custom {
                verb_lemma: "простить".to_string(),
                polarity: VerbPolarity::Positive,
            };
        }

        // ── Предательство / измена ──
        "предать" | "предавать" | "изменить" | "изменять" => {
            return Action::Betray {
                victim: String::new(),
            };
        }

        // ── Ложь — негативный, нестандартный ──
        "обмануть" | "обманывать" | "солгать" | "лгать" => {
            return Action::Custom {
                verb_lemma: "лгать".to_string(),
                polarity: VerbPolarity::Negative,
            };
        }

        // ── Кража — негативный, нестандартный ──
        "украсть" | "красть" => {
            return Action::Custom {
                verb_lemma: "красть".to_string(),
                polarity: VerbPolarity::Negative,
            };
        }

        // ── Брак ──
        "жениться" | "выйти замуж" => {
            return Action::Marry {
                partner: String::new(),
            };
        }

        // ── Смерть / воскресение (непереходные) ──
        "умереть" => return Action::Die,
        "воскреснуть" | "воскресать" => return Action::Resurrect,

        // ── Когнитивные: забыть / вспомнить ──
        "забыть" => {
            return Action::Forget {
                fact: String::new(),
            };
        }
        "вспомнить" => {
            return Action::Know {
                fact: String::new(),
            };
        }

        // ── Эмоциональные: влюбиться / ненавидеть ──
        "полюбить" => {
            return Action::FallInLove {
                partner: String::new(),
            };
        }
        "ненавидеть" => {
            return Action::Hate {
                target: String::new(),
            };
        }

        // ── Fallback на множества / polarity ──
        _ => {}
    }

    // 2) Лемма в одном из множеств Python — берём полярность оттуда.
    if is_positive_verb(&v) {
        return Action::Custom {
            verb_lemma: v,
            polarity: VerbPolarity::Positive,
        };
    }
    if is_negative_verb(&v) {
        return Action::Custom {
            verb_lemma: v,
            polarity: VerbPolarity::Negative,
        };
    }
    if is_neutral_verb(&v) {
        return Action::Custom {
            verb_lemma: v,
            polarity: VerbPolarity::Neutral,
        };
    }

    // 3) Полностью неизвестная лемма — используем поле `polarity` из триплета.
    //    Если negated=true — flip'аем positive↔negative (neutral остаётся).
    let inferred = match polarity.trim().to_lowercase().as_str() {
        "positive" | "pos" => VerbPolarity::Positive,
        "negative" | "neg" => VerbPolarity::Negative,
        "neutral" | "neu" => VerbPolarity::Neutral,
        // Пусто или непонятно — neutral (безопасный дефолт).
        _ => VerbPolarity::Neutral,
    };
    let final_polarity = if negated {
        match inferred {
            VerbPolarity::Positive => VerbPolarity::Negative,
            VerbPolarity::Negative => VerbPolarity::Positive,
            VerbPolarity::Neutral => VerbPolarity::Neutral,
        }
    } else {
        inferred
    };

    Action::Custom {
        verb_lemma: v,
        polarity: final_polarity,
    }
}

// ============ EntityResolver — name → LitNode.id ============

/// Резолвер имён в `LitNode.id`. Строится один раз из списка узлов графа и
/// затем используется иммутабельно во всех вызовах [`triplets_to_events`] /
/// [`parse_text_fallback`].
///
/// Хранит два индекса:
/// - `by_lemma` — `lowercase(node.data.title) → node.id`. Для «Иван» (title)
///   и триплета с `subject_lemma = "Иван"` — точное совпадение.
/// - `by_alias` — `lowercase(alias) → node.id`. Алиасы берутся из
///   `node.data.meta.aliases` (массив строк) и `node.data.meta.forms`
///   (массив строк,grammar forms). Например, для «Ваня» с aliases
///   `["Иван", "Ванюша"]` триплет с `subject_lemma = "Ванюша"` тоже найдётся.
///
/// # Поиск
///
/// [`EntityResolver::resolve`] сначала ищет в `by_lemma`, потом в `by_alias`.
/// Точное совпадение по lowercase — никакого fuzzy matching (SPEC §5:
/// «Determinism first», и SPEC §5.4: «no `unwrap()` on external data»).
/// Если имя не найдено — `None`.
///
/// [`EntityResolver::resolve_or_keep`] — то же самое, но при `None` возвращает
/// исходную строку. Это позволяет сохранить «фантомные сущности» (имена,
/// которые упомянуты в тексте, но не имеют узла в графе) и передать их в
/// Wave 4 (`cycle.rs`), где будет решено: создать узел или отбросить событие.
///
/// # Какие узлы индексируются
///
/// Только узлы с `node_type ∈ {"character", "organization"}`. Локации, темы,
/// сцены и т.д. не являются актантами в SVO-триплетах, поэтому их имена не
/// попадают в индекс. Это сознательное ограничение — иначе «Лес» (location)
/// может случайно стать target'ом для Kill.
#[derive(Debug, Clone, Default)]
pub struct EntityResolver {
    /// `lowercase(node.data.title) → node.id` для персонажей и организаций.
    by_lemma: HashMap<String, String>,
    /// `lowercase(alias_or_form) → node.id` для всех aliases/forms из meta.
    by_alias: HashMap<String, String>,
}

impl EntityResolver {
    /// Построить резолвер из списка узлов графа.
    ///
    /// Узлы с `node_type ∉ {"character", "organization"}` пропускаются.
    /// Если у узла несколько aliases/forms — добавляются все. Если два узла
    /// имеют одинаковое имя/alias — последний выигрывает (это редко случается
    /// и обычно означает дубль в графе, который должен быть вычищен на этапе
    /// NER, а не здесь).
    pub fn from_nodes(nodes: &[LitNode]) -> Self {
        let mut by_lemma: HashMap<String, String> = HashMap::new();
        let mut by_alias: HashMap<String, String> = HashMap::new();

        for node in nodes {
            // Только персонажи и организации — актанты в SVO.
            let is_actor = matches!(
                node.node_type.as_str(),
                "character" | "organization"
            );
            if !is_actor {
                continue;
            }

            // Лемма = title в lowercase.
            let title_lc = node.data.title.trim().to_lowercase();
            if !title_lc.is_empty() {
                by_lemma.insert(title_lc, node.id.clone());
            }

            // Aliases / forms из meta — общий индекс by_alias.
            if let Some(meta) = &node.data.meta {
                if let Some(names) = extract_string_array(meta, "aliases") {
                    for name in names {
                        let lc = name.trim().to_lowercase();
                        if !lc.is_empty() {
                            by_alias.insert(lc, node.id.clone());
                        }
                    }
                }
                if let Some(names) = extract_string_array(meta, "forms") {
                    for name in names {
                        let lc = name.trim().to_lowercase();
                        if !lc.is_empty() {
                            by_alias.insert(lc, node.id.clone());
                        }
                    }
                }
            }
        }

        Self {
            by_lemma,
            by_alias,
        }
    }

    /// Точный поиск имени в индексах. Case-insensitive (через lowercase).
    /// Возвращает `Some(LitNode.id)` при совпадении, `None` иначе.
    pub fn resolve(&self, name: &str) -> Option<String> {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            return None;
        }
        if let Some(id) = self.by_lemma.get(&key) {
            return Some(id.clone());
        }
        if let Some(id) = self.by_alias.get(&key) {
            return Some(id.clone());
        }
        None
    }

    /// Поиск с fallback'ом на исходную строку. Если [`Self::resolve`]
    /// вернул `None` — возвращается `name` как есть. Получившаяся строка
    /// становится «фантомной сущностью» (см. doc-comment структуры).
    pub fn resolve_or_keep(&self, name: &str) -> String {
        match self.resolve(name) {
            Some(id) => id,
            None => name.to_string(),
        }
    }

    /// Количество уникальных лемм в индексе (для отладки/тестов).
    pub fn lemma_count(&self) -> usize {
        self.by_lemma.len()
    }

    /// Количество уникальных алиасов в индексе (для отладки/тестов).
    pub fn alias_count(&self) -> usize {
        self.by_alias.len()
    }
}

/// Извлекает из `serde_json::Value` (объект) массив строк по ключу.
/// Возвращает `None`, если ключа нет, значение не массив, или массив
/// содержит не-строки (в этом случае не-строки пропускаются, а возвращается
/// только валидные строки; пустой массив после фильтрации → `None`).
fn extract_string_array(meta: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    let arr = meta.get(key)?.as_array()?;
    let out: Vec<String> = arr
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

// ============ Основной конвертер: SVO triplets → Events ============

/// Преобразует список SVO-триплетов (из Python) в список [`Event`] для
/// Reasoning Engine.
///
/// Для каждого триплета:
/// 1. Определяется [`TemporalAnchor`] из `triplet.position` (byte offset)
///    через поиск по `ParsedChapter.pos..end`. Если позиция до первой главы —
///    `chapter_num = 0` (пролог).
/// 2. Строится [`Action`] через [`verb_to_action`].
/// 3. Payload действия (destination/fact/partner/victim/...) заполняется
///    из `object_lemma` / `sentence` / резолвера (см. [`populate_action_payload`]).
/// 4. `actor` резолвится через `resolver.resolve_or_keep(&subject_lemma)`.
/// 5. `target` резолвится только для «target-variant» действий
///    (Kill/Wound/Hit/Capture/Imprison/Free/Heal/Touch) — иначе `None`.
/// 6. `instrument = None` (SVO не извлекает орудие — это будущая работа).
/// 7. `source_text = triplet.sentence`.
/// 8. `confidence = 0.9` (SVO — high-quality).
/// 9. `provenance = Provenance::SvoParser`.
/// 10. `id = 0` — назначается [`crate::reasoning::facts::FactLog::record_event`]
///     при добавлении в журнал.
///
/// # Аргументы
/// - `triplets` — срез SVO-триплетов из Python (или из JSON-файла).
/// - `resolver` — построенный из `LitNode`-узлов графа [`EntityResolver`].
/// - `chapters` — список глав из `parser::chapters::detect()`. Нужен для
///   маппинга byte offset → chapter_num.
///
/// # Возвращаемое
///
/// `Vec<Event>` в том же порядке, что и входные триплеты. Пустой вход →
/// пустой выход.
pub fn triplets_to_events(
    triplets: &[SvoTriplet],
    resolver: &EntityResolver,
    chapters: &[ParsedChapter],
) -> Vec<Event> {
    triplets
        .iter()
        .map(|t| {
            let time = anchor_from_position(t.position, chapters);
            let raw_action = verb_to_action(&t.verb_lemma, &t.polarity, t.negated);
            let action = populate_action_payload(raw_action, t, resolver);

            let actor = resolver.resolve_or_keep(&t.subject_lemma);
            let target = target_for_action(&action, &t.object_lemma, resolver);

            Event {
                id: EventId::default(), // 0 — назначается FactLog::record_event
                actor,
                action,
                target,
                instrument: None,
                time,
                source_text: t.sentence.clone(),
                confidence: 0.9,
                provenance: Provenance::SvoParser,
            }
        })
        .collect()
}

/// Строит [`TemporalAnchor`] из byte offset в исходном тексте.
///
/// Алгоритм: linear scan по `chapters` в поисках главы, у которой
/// `pos <= position < end`. Если найдена — `chapter_num = chapter.num`,
/// `chapter_suffix = None` (task brief явно указывает None — суффикс
/// суб-главы хранится в `chapter.title`, но не извлекается здесь), и
/// `char_offset = position`. Если позиция до первой главы —
/// `chapter_num = 0` (пролог), `char_offset = position`.
///
/// Если список глав пуст — также возвращается глава 0 (пролог/sentinel).
fn anchor_from_position(position: usize, chapters: &[ParsedChapter]) -> TemporalAnchor {
    for ch in chapters {
        if ch.pos <= position && position < ch.end {
            return TemporalAnchor {
                chapter_num: ch.num,
                chapter_suffix: None,
                scene_index: None,
                char_offset: position,
            };
        }
    }
    // Не нашли — позиция до первой главы (пролог) ИЛИ chapters пуст.
    // В обоих случаях chapter_num = 0 — sentinel «пролог/до нарратива».
    TemporalAnchor {
        chapter_num: 0,
        chapter_suffix: None,
        scene_index: None,
        char_offset: position,
    }
}

/// Заполняет payload варианта [`Action`], который несёт inline-данные
/// (destination/source/fact/partner/victim/target).
///
/// Эта функция вызывается **после** [`verb_to_action`], которая возвращает
/// варианты с пустыми payload-строками (`String::new()`). Здесь мы
/// подставляем реальные значения из триплета:
///
/// | Action variant | Payload source |
/// |---|---|
/// | `Move { destination }` | `triplet.object_lemma` (если непусто) |
/// | `Arrive { destination }` | `triplet.object_lemma` |
/// | `Leave { source }` | `triplet.object_lemma` |
/// | `Know { fact }` | `triplet.sentence` (всё предложение как «факт») |
/// | `Forget { fact }` | `triplet.sentence` |
/// | `FallInLove { partner }` | `resolver.resolve_or_keep(&triplet.object_lemma)` |
/// | `Hate { target }` | `resolver.resolve_or_keep(&triplet.object_lemma)` |
/// | `Marry { partner }` | `resolver.resolve_or_keep(&triplet.object_lemma)` |
/// | `Betray { victim }` | `resolver.resolve_or_keep(&triplet.object_lemma)` |
///
/// Все остальные варианты возвращаются как есть (их payload не зависит от
/// триплета — например, `Kill` несёт только сам факт, без inline-данных).
///
/// # Почему Know/Forget используют `sentence`, а не `object_lemma`
///
/// Task brief явно говорит: «Know/Forget: fact = triplet.sentence.clone()
/// (full sentence as the "fact" — approximation)». Это аппроксимация: в
/// идеале факт — это пропозиция («Иван знает, что Пётр мёртв»), но SVO даёт
/// только глагол + объект. Полное предложение — лучший доступный субститут.
fn populate_action_payload(
    action: Action,
    t: &SvoTriplet,
    resolver: &EntityResolver,
) -> Action {
    // Резолвим объект один раз — он нужен в нескольких ветках.
    let object_resolved = || resolver.resolve_or_keep(&t.object_lemma);

    match action {
        Action::Move { .. } => Action::Move {
            destination: t.object_lemma.clone(),
        },
        Action::Arrive { .. } => Action::Arrive {
            destination: t.object_lemma.clone(),
        },
        Action::Leave { .. } => Action::Leave {
            source: t.object_lemma.clone(),
        },
        Action::Know { .. } => Action::Know {
            fact: t.sentence.clone(),
        },
        Action::Forget { .. } => Action::Forget {
            fact: t.sentence.clone(),
        },
        Action::FallInLove { .. } => Action::FallInLove {
            partner: object_resolved(),
        },
        Action::Hate { .. } => Action::Hate {
            target: object_resolved(),
        },
        Action::Marry { .. } => Action::Marry {
            partner: object_resolved(),
        },
        Action::Betray { .. } => Action::Betray {
            victim: object_resolved(),
        },
        // Остальные варианты — без inline-payload от триплета.
        other => other,
    }
}

/// Определяет, нужно ли для данного [`Action`] заполнять `Event.target`, и
/// если да — возвращает `Some(resolved_target)`. Иначе `None`.
///
/// `target` заполняется только для «физических» действий, у которых объектом
/// является сущность (Kill, Wound, Hit, Capture, Imprison, Free, Heal,
/// Touch). Для действий с inline-payload (Marry/Betray/Ally/FallInLove/
/// Hate) объект уже хранится внутри `Action`, поэтому `Event.target`
/// остаётся `None` — иначе было бы дублирование.
///
/// Для Move/Arrive/Leave/Speak/Know/Forget/... — `target = None`, потому
/// что объект является локацией/темой/фактом, а не сущностью.
fn target_for_action(
    action: &Action,
    object_lemma: &str,
    resolver: &EntityResolver,
) -> Option<String> {
    match action {
        Action::Kill
        | Action::Wound
        | Action::Hit
        | Action::Capture
        | Action::Imprison
        | Action::Free
        | Action::Heal
        | Action::Touch => Some(resolver.resolve_or_keep(object_lemma)),
        // Marry/Betray/Ally/FallInLove/Hate несут EntityId внутри — не дублируем.
        // Move/Arrive/Leave/Speak/Ask/Tell(only if generated) — не target-variant.
        // Know/Forget/Want/Plan/Discover/Transform/Die/Resurrect/Custom — не target.
        _ => None,
    }
}

// ============ Резервный парсер: regex-based (Rust-only) ============

/// Компилирует regex один раз (многократно используется в [`parse_text_fallback`]).
/// Все паттерны — статические литералы, `unwrap()` здесь безопасен (ошибка
/// возможна только при баге в самом fancy-regex).
fn fallback_regexes() -> FallbackRegexes {
    FallbackRegexes {
        // `\b` в fancy-regex поддерживает Unicode word boundaries (флаг `u`
        // включён по умолчанию). Это позволяет матчить «убил» как отдельное
        // слово, а не как подстроку «убилство».
        kill: Regex::new(r"\b(?:убил|убила|убило|убили)\b")
            .expect("невалидный regex: kill"),
        speak: Regex::new(r"\b(?:сказал|сказала|сказали)\b")
            .expect("невалидный regex: speak"),
        die: Regex::new(r"\b(?:умер|умерла|умерли)\b")
            .expect("невалидный regex: die"),
        resurrect: Regex::new(r"\b(?:воскрес|воскресла|воскресли)\b")
            .expect("невалидный regex: resurrect"),
        arrive: Regex::new(r"\b(?:пошёл|пошла|пошли|пришёл|пришла|пришли)\b")
            .expect("невалидный regex: arrive"),
        // Разделители предложений: точка, !, ?, многоточие (как одна точка
        // или как символ …). Многократные разделители подряд — одна граница.
        sentence_split: Regex::new(r"[.!?…]+").expect("невалидный regex: sentence_split"),
        // Заглавное русское слово: [А-ЯЁ][а-яё]+. Используется для поиска
        // потенциальных имён персонажей в предложении.
        cap_word: Regex::new(r"[А-ЯЁ][а-яё]+").expect("невалидный regex: cap_word"),
    }
}

/// Контейнер для скомпилированных regex'ов fallback-парсера.
struct FallbackRegexes {
    kill: Regex,
    speak: Regex,
    die: Regex,
    resurrect: Regex,
    arrive: Regex,
    sentence_split: Regex,
    cap_word: Regex,
}

/// Резервный парсер: regex-based извлечение событий из русского текста без
/// Python. Используется когда `svo_extract.py` недоступен (нет spaCy /
/// pymorphy3 / интерпретатора).
///
/// # Алгоритм
///
/// 1. Текст разбивается на предложения по `[.!?…]+` с сохранением byte
///    offset'ов.
/// 2. Для каждого предложения:
///    - Находятся все заглавные слова (`[А-ЯЁ][а-яё]+`) — потенциальные имена.
///    - Проверяется наличие одного из ключевых глаголов: `убил/убила/убило/
///      убили`, `сказал/сказала/сказали`, `умер/умерла/умерли`, `воскрес/
///      воскресла/воскресли`, `пошёл/пошла/пошли/пришёл/пришла/пришли`.
///    - Если есть глагол — строится [`Event`]:
///      - `actor` = первое заглавное слово (резолвится через [`EntityResolver`]).
///      - `target` = второе заглавное слово (только для Kill, иначе `None`).
///      - `action` = соответствующий вариант.
///      - `time` = [`anchor_from_position`] от byte offset начала предложения.
///      - `confidence = 0.5` (ниже, чем у SVO).
///      - `provenance = Provenance::RustParser`.
///
/// # Ограничения
///
/// Это аварийный режим. Он не делает:
/// - лемматизацию (найдёт «убил», но не «убивать»);
/// - разрешение местоимений («он» не станет «Иваном»);
/// - извлечение объекта (только «второе заглавное слово» как target для Kill);
/// - извлечение темы Speak / destination Move.
///
/// Полный SVO-анализ — задача Python. Цель этой функции — чтобы reasoning
/// engine вообще работал без Python, хоть и грубо.
pub fn parse_text_fallback(
    text: &str,
    resolver: &EntityResolver,
    chapters: &[ParsedChapter],
) -> Vec<Event> {
    if text.is_empty() {
        return Vec::new();
    }

    let regexes = fallback_regexes();
    let mut events: Vec<Event> = Vec::new();

    // Итерируемся по предложениям через sentence_split regex.
    // Границы: [0, match1.start), [match1.end, match2.start), ..., [last_end, text.len()).
    let mut last_start = 0usize;
    let mut boundaries: Vec<(usize, usize)> = Vec::new();

    // Собираем все границы предложений.
    for r in regexes.sentence_split.find_iter(text) {
        match r {
            Ok(m) => {
                // Предложение заканчивается в m.start(), следующее начнётся в m.end().
                boundaries.push((last_start, m.start()));
                last_start = m.end();
            }
            Err(_) => continue,
        }
    }
    // Последний «хвост» — после последнего разделителя до конца текста.
    if last_start < text.len() {
        boundaries.push((last_start, text.len()));
    } else if boundaries.is_empty() {
        // Вообще нет разделителей — весь текст одно предложение.
        boundaries.push((0, text.len()));
    }

    for (sent_start, sent_end) in boundaries {
        if sent_end <= sent_start {
            continue;
        }
        // Безопасный срез — regex-паттерны ASCII, поэтому границы на char boundaries.
        // Но проверяем на всякий случай.
        let sentence = if text.is_char_boundary(sent_start) && text.is_char_boundary(sent_end) {
            &text[sent_start..sent_end]
        } else {
            continue;
        };

        // Список заглавных слов — потенциальных имён персонажей.
        let caps: Vec<String> = regexes
            .cap_word
            .find_iter(sentence)
            .filter_map(|r| r.ok())
            .map(|m| m.as_str().to_string())
            .collect();

        // Определяем action по первому матчу глагола. Порядок важен: kill
        // проверяем первым — если в предложении «убил» и «сказал», Kill
        // важнее.
        let action_and_needs_target: Option<(Action, bool)> = if regexes
            .kill
            .is_match(sentence)
            .unwrap_or(false)
        {
            Some((Action::Kill, true))
        } else if regexes
            .speak
            .is_match(sentence)
            .unwrap_or(false)
        {
            Some((Action::Speak { topic: None }, false))
        } else if regexes
            .die
            .is_match(sentence)
            .unwrap_or(false)
        {
            Some((Action::Die, false))
        } else if regexes
            .resurrect
            .is_match(sentence)
            .unwrap_or(false)
        {
            Some((Action::Resurrect, false))
        } else if regexes
            .arrive
            .is_match(sentence)
            .unwrap_or(false)
        {
            Some((
                Action::Arrive {
                    destination: String::new(),
                },
                false,
            ))
        } else {
            None
        };

        let (action, needs_target) = match action_and_needs_target {
            Some(x) => x,
            None => continue, // Нет известного глагола — пропускаем предложение.
        };

        // Actor — первое заглавное слово. Если его нет — пропускаем (без
        // актанта событие бессмысленно).
        let actor_name = match caps.first() {
            Some(n) => n,
            None => continue,
        };
        let actor = resolver.resolve_or_keep(actor_name);

        // Target — второе заглавное слово, только для Kill.
        let target = if needs_target {
            caps.get(1).map(|n| resolver.resolve_or_keep(n))
        } else {
            None
        };

        let time = anchor_from_position(sent_start, chapters);

        events.push(Event {
            id: EventId::default(), // 0
            actor,
            action,
            target,
            instrument: None,
            time,
            source_text: sentence.trim().to_string(),
            confidence: 0.5,
            provenance: Provenance::RustParser,
        });
    }

    events
}

// ============ Юнит-тесты ============

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LitNode, LitNodeData, Position};
    use serde_json::json;

    /// Хелпер: строит `LitNode` с указанными id / title / type / meta.
    fn make_node(
        id: &str,
        title: &str,
        node_type: &str,
        meta: Option<serde_json::Value>,
    ) -> LitNode {
        LitNode {
            id: id.to_string(),
            node_type: node_type.to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: title.to_string(),
                body: String::new(),
                node_type: node_type.to_string(),
                tags: vec![],
                meta,
                full_text: None,
                versions: None,
            },
        }
    }

    /// Хелпер: строит `ParsedChapter` с указанными num / pos / end.
    fn make_chapter(num: u32, pos: usize, end: usize) -> ParsedChapter {
        ParsedChapter {
            num,
            title: format!("Глава {}", num),
            body: String::new(),
            full_text: String::new(),
            pos,
            end,
        }
    }

    /// Хелпер: строит `SvoTriplet` с основными полями.
    fn make_triplet(
        subject_lemma: &str,
        verb_lemma: &str,
        object_lemma: &str,
        sentence: &str,
        position: usize,
        polarity: &str,
    ) -> SvoTriplet {
        SvoTriplet {
            subject: subject_lemma.to_string(),
            subject_lemma: subject_lemma.to_string(),
            subject_gender: None,
            verb: verb_lemma.to_string(),
            verb_lemma: verb_lemma.to_string(),
            object: object_lemma.to_string(),
            object_lemma: object_lemma.to_string(),
            object_gender: None,
            sentence: sentence.to_string(),
            position,
            tense: "past".to_string(),
            polarity: polarity.to_string(),
            negated: false,
            pronoun_resolved: false,
        }
    }

    // ── verb_to_action ───────────────────────────────────────────────

    #[test]
    fn test_verb_to_action_kill() {
        // Прямая лемма.
        assert_eq!(
            verb_to_action("убить", "negative", false),
            Action::Kill,
            "«убить» → Kill"
        );
        // Imperfective вид.
        assert_eq!(
            verb_to_action("убивать", "negative", false),
            Action::Kill,
            "«убивать» → Kill"
        );
        // Казнь — тоже Kill (без отдельного Action::Execute).
        assert_eq!(
            verb_to_action("казнить", "negative", false),
            Action::Kill,
            "«казнить» → Kill"
        );
        // Case-insensitive.
        assert_eq!(
            verb_to_action("УБИТЬ", "negative", false),
            Action::Kill,
            "uppercase lemma → Kill"
        );
        // С пробелами.
        assert_eq!(
            verb_to_action("  убить  ", "negative", false),
            Action::Kill,
            "trimmed lemma → Kill"
        );
    }

    #[test]
    fn test_verb_to_action_speak() {
        assert_eq!(
            verb_to_action("сказать", "neutral", false),
            Action::Speak { topic: None },
            "«сказать» → Speak{{None}}"
        );
        assert_eq!(
            verb_to_action("ответить", "neutral", false),
            Action::Speak { topic: None },
            "«ответить» → Speak{{None}}"
        );
        assert_eq!(
            verb_to_action("спросить", "neutral", false),
            Action::Speak { topic: None },
            "«спросить» → Speak{{None}}"
        );
        assert_eq!(
            verb_to_action("молвить", "neutral", false),
            Action::Speak { topic: None },
            "«молвить» → Speak{{None}}"
        );
    }

    #[test]
    fn test_verb_to_action_unknown_verb_uses_polarity() {
        // «Покружиться» — выдуманный глагол, не входит ни в одно из множеств.
        // Должен попасть в ветку 3 (polarity из триплета).
        let pos = verb_to_action("покружиться", "positive", false);
        assert_eq!(
            pos,
            Action::Custom {
                verb_lemma: "покружиться".to_string(),
                polarity: VerbPolarity::Positive,
            },
            "unknown verb + polarity=positive → Custom{{Positive}}"
        );

        let neg = verb_to_action("покружиться", "negative", false);
        assert_eq!(
            neg,
            Action::Custom {
                verb_lemma: "покружиться".to_string(),
                polarity: VerbPolarity::Negative,
            },
            "unknown verb + polarity=negative → Custom{{Negative}}"
        );

        let neu = verb_to_action("покружиться", "neutral", false);
        assert_eq!(
            neu,
            Action::Custom {
                verb_lemma: "покружиться".to_string(),
                polarity: VerbPolarity::Neutral,
            },
            "unknown verb + polarity=neutral → Custom{{Neutral}}"
        );

        // Пустая/непонятная polarity → Neutral (безопасный дефолт).
        let empty = verb_to_action("покружиться", "", false);
        assert_eq!(
            empty,
            Action::Custom {
                verb_lemma: "покружиться".to_string(),
                polarity: VerbPolarity::Neutral,
            },
            "unknown verb + empty polarity → Custom{{Neutral}}"
        );

        // negated=true флипает positive↔negative (для полностью неизвестных).
        let negated_pos = verb_to_action("покружиться", "positive", true);
        assert_eq!(
            negated_pos,
            Action::Custom {
                verb_lemma: "покружиться".to_string(),
                polarity: VerbPolarity::Negative,
            },
            "unknown verb + polarity=positive + negated → Custom{{Negative}}"
        );
    }

    // ── EntityResolver ───────────────────────────────────────────────

    #[test]
    fn test_entity_resolver_finds_by_title() {
        let nodes = vec![
            make_node("char-ivan-1", "Иван", "character", None),
            make_node("char-petr-2", "Пётр", "character", None),
            // Локация — не должна индексироваться.
            make_node("loc-castle-3", "Замок", "location", None),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);

        assert_eq!(
            resolver.resolve("Иван"),
            Some("char-ivan-1".to_string()),
            "Точное совпадение по title"
        );
        assert_eq!(
            resolver.resolve("иван"),
            Some("char-ivan-1".to_string()),
            "Case-insensitive (lowercase)"
        );
        assert_eq!(
            resolver.resolve("ИВАН"),
            Some("char-ivan-1".to_string()),
            "Case-insensitive (uppercase)"
        );
        assert_eq!(
            resolver.resolve("Пётр"),
            Some("char-petr-2".to_string()),
            "Другой персонаж"
        );
        assert_eq!(
            resolver.resolve("Замок"),
            None,
            "Локация не индексируется"
        );
        assert_eq!(resolver.lemma_count(), 2, "Два актанта в индексе");
        assert_eq!(resolver.alias_count(), 0, "Без aliases");
    }

    #[test]
    fn test_entity_resolver_finds_by_alias() {
        let nodes = vec![
            // Ваня — каноническое имя, с aliases и forms.
            make_node(
                "char-vanya-1",
                "Ваня",
                "character",
                Some(json!({
                    "aliases": ["Иван", "Ванюша", "Иоанн"],
                    "forms": ["Ваней", "Ваню"],
                })),
            ),
            // Организация тоже резолвится.
            make_node(
                "org-council-2",
                "Совет",
                "organization",
                Some(json!({
                    "aliases": ["Старейшины"]
                })),
            ),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);

        // Title.
        assert_eq!(
            resolver.resolve("Ваня"),
            Some("char-vanya-1".to_string()),
            "Title найден"
        );
        // Aliases.
        assert_eq!(
            resolver.resolve("Иван"),
            Some("char-vanya-1".to_string()),
            "Alias «Иван» найден"
        );
        assert_eq!(
            resolver.resolve("Ванюша"),
            Some("char-vanya-1".to_string()),
            "Alias «Ванюша» найден"
        );
        assert_eq!(
            resolver.resolve("Иоанн"),
            Some("char-vanya-1".to_string()),
            "Alias «Иоанн» найден"
        );
        // Forms.
        assert_eq!(
            resolver.resolve("Ваней"),
            Some("char-vanya-1".to_string()),
            "Form «Ваней» найдена"
        );
        assert_eq!(
            resolver.resolve("Ваню"),
            Some("char-vanya-1".to_string()),
            "Form «Ваню» найдена"
        );
        // Case-insensitive для aliases.
        assert_eq!(
            resolver.resolve("иван"),
            Some("char-vanya-1".to_string()),
            "Alias «иван» (lowercase) найден"
        );
        // Организация.
        assert_eq!(
            resolver.resolve("Старейшины"),
            Some("org-council-2".to_string()),
            "Alias организации найден"
        );
        assert_eq!(resolver.lemma_count(), 2, "Две леммы (Ваня + Совет)");
        // 3 aliases + 2 forms для Вани + 1 alias для Совета = 6 всего.
        assert_eq!(
            resolver.alias_count(), 6,
            "6 алиасов: 3 (Ваня aliases) + 2 (Ваня forms) + 1 (Совет alias)"
        );
    }

    #[test]
    fn test_entity_resolver_returns_none_for_unknown() {
        let nodes = vec![
            make_node("char-ivan-1", "Иван", "character", None),
            make_node(
                "char-vanya-2",
                "Ваня",
                "character",
                Some(json!({ "aliases": ["Ванюша"] })),
            ),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);

        // Совсем неизвестное имя.
        assert_eq!(
            resolver.resolve("Николай"),
            None,
            "Николай не в индексе"
        );
        // Пустая строка.
        assert_eq!(resolver.resolve(""), None, "Пустая строка → None");
        // Только пробелы.
        assert_eq!(resolver.resolve("   "), None, "Только пробелы → None");

        // resolve_or_keep возвращает исходную строку для «фантомной сущности».
        assert_eq!(
            resolver.resolve_or_keep("Николай"),
            "Николай".to_string(),
            "resolve_or_keep сохраняет неизвестное имя"
        );
        assert_eq!(
            resolver.resolve_or_keep("Иван"),
            "char-ivan-1".to_string(),
            "resolve_or_keep возвращает id для известного"
        );
        // resolve_or_keep на пустой строке → пустая строка (phantom).
        assert_eq!(
            resolver.resolve_or_keep(""),
            "".to_string(),
            "resolve_or_keep(\"\") → \"\""
        );
    }

    // ── triplets_to_events ───────────────────────────────────────────

    #[test]
    fn test_triplets_to_events_assigns_temporal_anchor() {
        let chapters = vec![
            make_chapter(10, 0, 500),
            make_chapter(12, 500, 1500),
            make_chapter(15, 1500, 3000),
        ];
        let resolver = EntityResolver::from_nodes(&[]);

        // Триплет в середине главы 12 (position=1000).
        let t1 = make_triplet(
            "Иван",
            "сказать",
            "",
            "Иван сказал.",
            1000,
            "neutral",
        );
        let events = triplets_to_events(&[t1], &resolver, &chapters);
        assert_eq!(events.len(), 1, "Один триплет → одно событие");
        assert_eq!(
            events[0].time.chapter_num, 12,
            "position=1000 → глава 12"
        );
        assert_eq!(events[0].time.char_offset, 1000, "char_offset = position");
        assert_eq!(events[0].time.chapter_suffix, None, "suffix = None");
        assert_eq!(events[0].time.scene_index, None, "scene_index = None");

        // Триплет в начале главы 15 (position=1500 — граница, должна войти в 15).
        let t2 = make_triplet(
            "Пётр",
            "сказать",
            "",
            "Пётр сказал.",
            1500,
            "neutral",
        );
        let events2 = triplets_to_events(&[t2], &resolver, &chapters);
        assert_eq!(
            events2[0].time.chapter_num, 15,
            "position=1500 (точный start главы 15) → глава 15"
        );

        // Триплет ДО первой главы (position=0, но первая глава начинается в 0).
        // Граница [0, 500) — входит в главу 10.
        let t3 = make_triplet(
            "Николай",
            "сказать",
            "",
            "Николай сказал.",
            0,
            "neutral",
        );
        let events3 = triplets_to_events(&[t3], &resolver, &chapters);
        assert_eq!(
            events3[0].time.chapter_num, 10,
            "position=0 → первая глава (10)"
        );

        // Триплет с position до всех глав (если chapters не пустые, но position < первой).
        // Здесь первая глава начинается в 0, так что этот случай не воспроизводим.
        // Сделаем главы с offset'ом:
        let chapters_offset = vec![make_chapter(5, 100, 500)];
        let t4 = make_triplet(
            "Алексей",
            "сказать",
            "",
            "Алексей сказал.",
            50,
            "neutral",
        );
        let events4 = triplets_to_events(&[t4], &resolver, &chapters_offset);
        assert_eq!(
            events4[0].time.chapter_num, 0,
            "position=50 до первой главы (pos=100) → глава 0 (пролог)"
        );
        assert_eq!(events4[0].time.char_offset, 50, "char_offset сохранён");

        // Пустой список глав → глава 0 для любой позиции.
        let t5 = make_triplet(
            "Борис",
            "сказать",
            "",
            "Борис сказал.",
            9999,
            "neutral",
        );
        let events5 = triplets_to_events(&[t5], &resolver, &[]);
        assert_eq!(
            events5[0].time.chapter_num, 0,
            "Пустой chapters → глава 0"
        );
    }

    #[test]
    fn test_triplets_to_events_resolves_actor_and_target() {
        let nodes = vec![
            make_node("char-raskol-1", "Раскольников", "character", None),
            make_node(
                "char-alyona-2",
                "Алёна",
                "character",
                Some(json!({ "forms": ["Алёну", "Алёны"] })),
            ),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 10000)];

        // «Раскольников убил Алёну.» — subject=Раскольников, verb=убить, object=Алёна.
        let t = make_triplet(
            "Раскольников",
            "убить",
            "Алёна",
            "Раскольников убил Алёну топором.",
            42,
            "negative",
        );
        let events = triplets_to_events(&[t], &resolver, &chapters);
        assert_eq!(events.len(), 1);

        let ev = &events[0];
        // Actor резолвится по subject_lemma.
        assert_eq!(
            ev.actor, "char-raskol-1",
            "actor резолвится в LitNode.id Раскольникова"
        );
        // Action — Kill (явная таблица).
        assert_eq!(ev.action, Action::Kill, "verb «убить» → Action::Kill");
        // Target резолвится по object_lemma (для Kill — это Some).
        assert_eq!(
            ev.target,
            Some("char-alyona-2".to_string()),
            "target резолвится в LitNode.id Алёны (через form «Алёна»)"
        );
        // Provenance и confidence.
        assert_eq!(ev.provenance, Provenance::SvoParser, "SVO provenance");
        assert!(
            (ev.confidence - 0.9).abs() < 1e-6,
            "confidence = 0.9 для SVO"
        );
        assert_eq!(ev.id, 0, "id = 0 — назначается FactLog::record_event");
        assert_eq!(ev.instrument, None, "instrument не извлекается SVO");
        assert_eq!(
            ev.source_text, "Раскольников убил Алёну топором.",
            "source_text = sentence"
        );

        // Проверим Marry — partner резолвится через object_lemma.
        let t2 = make_triplet(
            "Анна",
            "жениться",
            "Иван",
            "Анна вышла замуж за Ивана.",
            100,
            "positive",
        );
        // Добавим Анну и Ивана в resolver.
        let nodes2 = vec![
            make_node("char-anna-3", "Анна", "character", None),
            make_node("char-ivan-4", "Иван", "character", None),
        ];
        let resolver2 = EntityResolver::from_nodes(&nodes2);
        let events2 = triplets_to_events(&[t2], &resolver2, &chapters);
        assert_eq!(events2.len(), 1);
        match &events2[0].action {
            Action::Marry { partner } => {
                assert_eq!(
                    partner, "char-ivan-4",
                    "Marry.partner резолвится в id Ивана"
                );
            }
            other => panic!("Ожидался Action::Marry, получено {:?}", other),
        }
        // Marry не порождает target (partner уже внутри action).
        assert_eq!(
            events2[0].target, None,
            "Marry не дублирует partner в Event.target"
        );

        // Проверим Speak — нет target.
        let t3 = make_triplet(
            "Иван",
            "сказать",
            "",
            "Иван сказал.",
            200,
            "neutral",
        );
        let events3 = triplets_to_events(&[t3], &resolver2, &chapters);
        assert_eq!(events3[0].target, None, "Speak → target = None");
        match &events3[0].action {
            Action::Speak { topic } => {
                assert_eq!(*topic, None, "Speak.topic = None");
            }
            other => panic!("Ожидался Action::Speak, получено {:?}", other),
        }

        // Проверим Die — нет target.
        let t4 = make_triplet("Пётр", "умереть", "", "Пётр умер.", 300, "neutral");
        let events4 = triplets_to_events(&[t4], &resolver2, &chapters);
        assert_eq!(events4[0].target, None, "Die → target = None");
        assert_eq!(events4[0].action, Action::Die, "verb «умереть» → Die");

        // Проверим фантомную сущность — неизвестное имя сохраняется как есть.
        let t5 = make_triplet(
            "Призрак",
            "сказать",
            "",
            "Призрак сказал.",
            400,
            "neutral",
        );
        let events5 = triplets_to_events(&[t5], &resolver2, &chapters);
        assert_eq!(
            events5[0].actor, "Призрак",
            "Неизвестное имя сохраняется как phantom entity"
        );
    }

    // ── parse_text_fallback ──────────────────────────────────────────

    #[test]
    fn test_parse_text_fallback_extracts_kill_event() {
        let nodes = vec![
            make_node("char-raskol-1", "Раскольников", "character", None),
            make_node(
                "char-alyona-2",
                "Алёна",
                "character",
                Some(json!({ "forms": ["Алёну"] })),
            ),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(5, 0, 10000)];

        // Два предложения: первое с Kill, второе без глагола.
        let text = "Раскольников убил Алёну. Потом пошёл дождь.";
        let events = parse_text_fallback(text, &resolver, &chapters);

        // Должно быть ровно одно событие (только первое предложение матчит Kill).
        // Второе предложение «Потом пошёл дождь» матчит arrive (пошёл),
        // но actor — «Потом» (не резолвится, остаётся phantom).
        // Проверим, что Kill-событие присутствует.
        let kill_events: Vec<&Event> = events
            .iter()
            .filter(|e| matches!(e.action, Action::Kill))
            .collect();
        assert_eq!(
            kill_events.len(),
            1,
            "Должно быть ровно одно Kill-событие, got {} events total: {:?}",
            events.len(),
            events
        );

        let kill = kill_events[0];
        assert_eq!(
            kill.actor, "char-raskol-1",
            "Actor = Раскольников (резолвится)"
        );
        assert_eq!(
            kill.target,
            Some("char-alyona-2".to_string()),
            "Target = Алёна (резолвится через form «Алёну» — но в fallback мы \
             берём второе cap_word, а это «Алёну», который резолвится через form)"
        );
        assert_eq!(kill.action, Action::Kill);
        assert_eq!(kill.provenance, Provenance::RustParser, "RustParser provenance");
        assert!(
            (kill.confidence - 0.5).abs() < 1e-6,
            "confidence = 0.5 для fallback"
        );
        assert_eq!(kill.time.chapter_num, 5, "Глава 5 (вся глава покрывает текст)");
        assert!(
            kill.source_text.contains("Раскольников") && kill.source_text.contains("убил"),
            "source_text содержит исходное предложение: {:?}",
            kill.source_text
        );
        assert_eq!(kill.id, 0, "id = 0 — назначается FactLog::record_event");
        assert_eq!(kill.instrument, None, "instrument не извлекается");

        // Проверим, что Die-событие тоже извлекается.
        let text2 = "Иван умер внезапно.";
        let nodes2 = vec![make_node("char-ivan-3", "Иван", "character", None)];
        let resolver2 = EntityResolver::from_nodes(&nodes2);
        let events2 = parse_text_fallback(text2, &resolver2, &chapters);
        assert_eq!(events2.len(), 1, "Должно быть одно событие (Die)");
        assert_eq!(events2[0].action, Action::Die, "«умер» → Die");
        assert_eq!(events2[0].actor, "char-ivan-3", "Actor = Иван");
        assert_eq!(events2[0].target, None, "Die → target = None");
        assert_eq!(events2[0].provenance, Provenance::RustParser);

        // Проверим, что безымянное событие пропускается.
        // Предложение в lowercase — нет заглавных слов, нет actor → пропущено.
        let text3 = "просто сказал что-то.";
        let events3 = parse_text_fallback(text3, &resolver2, &chapters);
        assert!(
            events3.is_empty(),
            "Предложение без заглавного слова-имени → пропущено, got: {:?}",
            events3
        );

        // Пустой текст → пустой список.
        let events4 = parse_text_fallback("", &resolver, &chapters);
        assert!(events4.is_empty(), "Пустой текст → нет событий");
    }
}
