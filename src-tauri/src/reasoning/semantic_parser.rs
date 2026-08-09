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

/// Генерирует падежные формы русских имён собственных для автоматического
/// разрешения винительного/родительного/дательного/творительного/предложного
/// падежей в [`EntityResolver`].
///
/// # Контекст проблемы (Wave 6)
///
/// В художественном тексте при действии Kill объект почти всегда стоит в
/// винительном или родительном падеже: «убил Грака», «убил Ревуна», «убил
/// Петра». Каноническое имя персонажа в графе — именительный падеж: «Грак»,
/// «Ревун», «Пётр». Без генерации падежей `EntityResolver::resolve("Грака")`
/// возвращает `None`, и `cycle.rs` пропускает ~12 kill-событий в романе
/// «Сфера Предела» с логом:
///
/// ```text
/// [inference] SetAttribute: RuleEntity::Target не разрешим
/// ```
///
/// # Алгоритм
///
/// Детерминированные правила по окончанию канонического имени (lowercase).
/// Не использует внешних морфологических библиотек (pymorphy3 / spaCy) —
/// это сознательное решение: deterministic first (SPEC §5), скорость, no I/O.
/// Покрытие — ~95% типичных русских имён собственных. Неточности для
/// экзотических случаев (Игорь → Игоря, Лев → Льва) обрабатываются через
/// явное указание форм в `node.data.meta.forms`.
///
/// # Поддерживаемые правила
///
/// | Окончание | Пример | Генерируемые формы |
/// |-----------|--------|-------------------|
/// | -ия (ж.)  | Мария  | марии, марию, марией |
/// | -а (ж.)   | Марта  | марты, марте, марту, мартой |
/// | -я (ж./м.)| Катя   | кати, кате, катю, катей |
/// | -й (м.)   | Алексей| алексея, алексею, алексеем, алексее |
/// | -ь (м./ж.)| Игорь  | игоря, игорю, игорем, игоре |
/// | согласная (м.) | Грак | грака, граку, граком, граке, гракы |
/// | Особый: Пётр/Лев | Пётр | петра, петру, петром, петре |
///
/// # Возвращаемое
///
/// `Vec<String>` в lowercase. Может содержать дубликаты (если имя попадает
/// под несколько правил — теоретически невозможно, но `sort + dedup` в конце
/// гарантирует уникальность). Пустое имя → пустой вектор.
pub fn generate_russian_declensions(name: &str) -> Vec<String> {
    let name_trim = name.trim();
    if name_trim.is_empty() {
        return Vec::new();
    }

    let lc = name_trim.to_lowercase();
    let chars: Vec<char> = lc.chars().collect();
    let len = chars.len();
    if len < 2 {
        return vec![lc];
    }

    let mut forms = Vec::new();

    // 1. Женские имена на -ия (Мария -> марии, марию, марией)
    if lc.ends_with("ия") && len > 3 {
        let stem: String = chars[..len - 2].iter().collect();
        forms.push(format!("{}ии", stem));
        forms.push(format!("{}ию", stem));
        forms.push(format!("{}ией", stem));
    }
    // 2. Женские имена на -а (Марта -> марты, марте, марту, мартой)
    else if lc.ends_with('а') {
        let stem: String = chars[..len - 1].iter().collect();
        forms.push(format!("{}ы", stem));
        forms.push(format!("{}и", stem));
        forms.push(format!("{}е", stem));
        forms.push(format!("{}у", stem));
        forms.push(format!("{}ой", stem));
    }
    // 3. Женские/мужские на -я (Катя -> кати, кате, катю, катей)
    else if lc.ends_with('я') {
        let stem: String = chars[..len - 1].iter().collect();
        forms.push(format!("{}и", stem));
        forms.push(format!("{}е", stem));
        forms.push(format!("{}ю", stem));
        forms.push(format!("{}ей", stem));
    }
    // 4. Мужские на -ей / -ай / -ой / -й (Алексей -> алексея, алексею, алексеем, алексее)
    else if lc.ends_with('й') {
        let stem: String = chars[..len - 1].iter().collect();
        forms.push(format!("{}я", stem));
        forms.push(format!("{}ю", stem));
        forms.push(format!("{}ем", stem));
        forms.push(format!("{}е", stem));
    }
    // 5. Мужские/женские на -ь (Игорь -> игоря, игорю, игорем, игоре)
    else if lc.ends_with('ь') {
        let stem: String = chars[..len - 1].iter().collect();
        forms.push(format!("{}я", stem));
        forms.push(format!("{}ю", stem));
        forms.push(format!("{}ем", stem));
        forms.push(format!("{}е", stem));
    }
    // 6. Мужские на твердую согласную (Грак, Ревун, Пётр, Иван)
    else {
        let last = chars[len - 1];
        // Проверяем, что согласная
        if "бвгджзклмнпрстфхцчшщ".contains(last) {
            // Особый случай: Пётр -> Петра (беглая гласная ё→е)
            if lc == "пётр" || lc == "петр" {
                forms.push("петра".to_string());
                forms.push("петру".to_string());
                forms.push("петром".to_string());
                forms.push("петре".to_string());
            }
            // Особый случай: Лев -> Льва (беглая гласная)
            else if lc == "лев" {
                forms.push("льва".to_string());
                forms.push("льву".to_string());
                forms.push("львом".to_string());
                forms.push("льве".to_string());
            } else {
                forms.push(format!("{}а", lc));
                forms.push(format!("{}у", lc));
                forms.push(format!("{}ом", lc));
                forms.push(format!("{}е", lc));
                forms.push(format!("{}ы", lc));
            }
        }
        // Если окончание не распознано (например, гласная "о" или "э"),
        // падежи не генерируем — пользователь должен явно указать forms.
    }

    forms.sort();
    forms.dedup();
    forms
}

// ============ Wave 7: Stop-Words & Dialogue Stripping ============

/// Проверяет, является ли слово служебным (местоимением, союзом, предлогом
/// или вводным словом), которое нельзя использовать в качестве фантомного
/// актёра.
///
/// # Контекст проблемы (Wave 7)
///
/// Анализ текста «Сфера Предела» выявил каскадный взрыв ложных парадоксов
/// и нарушений:
/// - `actor: "Не", dead_cannot_speak` — союз «Не» становился актёром.
/// - `actor: "Но", dead_cannot_speak` — союз «Но» становился актёром.
/// - `Парадокс #1: Он мёртв с Глава 0, но совершает действие Arrive в Глава 0`
/// — местоимение «Он» становилось фантомным персонажем, случайно получало
///   `alive = false` в какой-то точке, а затем 5000 последующих предложений,
///   начинающихся с «Он...», генерировали ложные парадоксы.
///
/// # Правило
///
/// Если слово входит в этот чёрный список И **не записано в графе персонажей**
/// (т.е. `EntityResolver::resolve` вернул `None`), оно **никогда** не должно
/// становиться актором события. Событие пропускается (`continue`), вместо
/// того чтобы плодить фантомные сущности.
///
/// Если же слово всё же записано в графе персонажей (например, автор назвал
/// персонажа «Но» — редкий, но возможный случай), `EntityResolver::resolve`
/// вернёт `Some(id)`, и [`is_russian_stop_word`] не будет вызван (или будет
/// вызван, но `resolver.resolve(cap).is_some()` в caller-коде перебивает
/// stop-word-фильтр).
///
/// # Список
///
/// Включает:
/// - личные местоимения: он, она, оно, они, я, мы, вы, ты;
/// - союзы и предлоги: но, или, как, когда, потому, что, чтобы, и, а, за,
///   на, из, при, до, после, о, об, для;
/// - указательные и вводные слова: это, тут, там, здесь, если, только,
///   однако, также, тоже, так;
/// - частицы и краткие ответы: не, нет, да;
/// - глагол-обращения (часто стоят в начале реплики): послушайте, слушай;
/// - формы глагола «быть»: была, было, были, быть;
/// - местоимения-прилагательные: все, всё, всех, всем, всю, один, одна,
///   одно, одни, такие, такой, который, которая, которое, которые, первый,
///   второй.
///
/// Список не претендует на полноту (русский язык богат), но покрывает ~95%
/// мусорных актёров, наблюдаемых на реальном тексте «Сферы Предела».
pub fn is_russian_stop_word(word: &str) -> bool {
    let lc = word.trim().to_lowercase();
    matches!(
        lc.as_str(),
        // Личные местоимения
        "он" | "она" | "оно" | "они" | "я" | "мы" | "вы" | "ты"
        // Союзы
        | "не" | "но" | "или" | "как" | "когда" | "потому" | "что" | "чтобы"
        | "если" | "только" | "однако" | "также" | "тоже" | "и" | "а"
        // Предлоги
        | "за" | "на" | "из" | "при" | "до" | "после" | "о" | "об" | "для"
        // Указательные и вводные слова
        | "это" | "тут" | "там" | "здесь" | "так"
        // Частицы и короткие ответы
        | "нет" | "да"
        // Обращения (часто в начале реплики)
        | "послушайте" | "слушай"
        // Формы глагола «быть»
        | "была" | "было" | "были" | "быть"
        // Местоимения-прилагательные и числительные
        | "один" | "одна" | "одно" | "одни"
        | "такие" | "такой"
        | "который" | "которая" | "которое" | "которые"
        | "первый" | "второй"
        // Обобщающие местоимения
        | "все" | "всё" | "всех" | "всем" | "всю"
    )
}

/// Удаляет из предложения прямую речь (диалоговое содержимое), оставляя
/// только авторский текст, в котором нужно искать актёра.
///
/// # Что удаляется
///
/// 1. **Содержимое кавычек**: `«...»`, `"..."` (typographic), `"..."` (ASCII).
///    Например, `Веня сказал: «Привет».` → `Веня сказал: .`
///
/// 2. **Диалог по тире**: в русском языке реплики часто оформляются так:
///    `— Талант у тебя есть, — сказал он.`
///    Здесь `—` открывает и закрывает реплику, а после второго `—` идёт
///    авторская атрибуция. Алгоритм берёт **текст после последнего `—`**,
///    если предложение начинается с `—` и содержит ≥ 2 тире (т.е. реплика
///    закрыта).
///
///    Если предложение начинается с `—`, но содержит только 1 тире
///    (диалог без авторской атрибуции: `— Привет.`), возвращается пустая
///    строка — событие будет пропущено в caller'е, т.к. нет актёра.
///
/// # Что НЕ удаляется
///
/// - Обычные em-dash в авторском тексте (`Паша — мой друг.`) — предложение
///   не начинается с `—`, поэтому правила диалога не применяются.
/// - Вложенные кавычки пока не поддерживаются (упрощение).
///
/// # Возвращаемое
///
/// Очищенная строка (авторский текст). Может быть пустой, если всё
/// предложение состояло из реплики — в этом случае caller должен пропустить
/// событие (нет актёра).
pub fn strip_dialogue_content(sentence: &str) -> String {
    // ── Шаг 1: вырезаем содержимое кавычек ──────────────────────────
    let mut no_quotes = String::with_capacity(sentence.len());
    let chars: Vec<char> = sentence.chars().collect();
    let mut i = 0;
    let mut in_quote = false;
    let mut quote_close: char = '\0';

    while i < chars.len() {
        let c = chars[i];
        if !in_quote {
            match c {
                '«' => {
                    in_quote = true;
                    quote_close = '»';
                    i += 1;
                    continue;
                }
                '\u{201C}' /* " */ => {
                    in_quote = true;
                    quote_close = '\u{201D}';
                    i += 1;
                    continue;
                }
                '"' => {
                    in_quote = true;
                    quote_close = '"';
                    i += 1;
                    continue;
                }
                _ => {}
            }
            no_quotes.push(c);
        } else if c == quote_close {
            in_quote = false;
            quote_close = '\0';
        }
        i += 1;
    }

    // ── Шаг 2: если начинается с тире (диалог) — берём авторскую часть ──
    let trimmed = no_quotes.trim_start();
    if trimmed.starts_with('—') {
        // Считаем количество тире в строке.
        let dash_count = no_quotes.matches('—').count();
        if dash_count >= 2 {
            // Диалог закрыт: берём текст после последнего тире.
            if let Some(last_dash_pos) = no_quotes.rfind('—') {
                let after_last_dash = &no_quotes[last_dash_pos + '—'.len_utf8()..];
                return after_last_dash.trim().to_string();
            }
        } else {
            // Только 1 тире. Два под-случая:
            // (a) «— Привет.» — чистая реплика без атрибуции → пропустить
            //     (возвращаем пустую строку).
            // (b) «— спросила Анна.» — авторское продолжение диалога, который
            //     был разбит на 2 предложения символом `?` или `!` внутри
            //     реплики (например, исходное «— Где? — спросила Анна.»
            //     разбилось на «— Где?» и «— спросила Анна.»).
            // Эвристика: если первое слово после `—` — глагол говорения,
            // считаем строку авторским текстом.
            let after_dash = trimmed['—'.len_utf8()..].trim_start();
            let first_word: String = after_dash
                .chars()
                .take_while(|c| c.is_alphabetic())
                .collect();
            let fw_lc = first_word.to_lowercase();
            let is_speech_verb = fw_lc.contains("сказал")
                || fw_lc.contains("говорит")
                || fw_lc.contains("говорят")
                || fw_lc.contains("ответил")
                || fw_lc.contains("ответила")
                || fw_lc.contains("ответили")
                || fw_lc.contains("спросил")
                || fw_lc.contains("спросила")
                || fw_lc.contains("спросили");
            if is_speech_verb {
                return after_dash.trim().to_string();
            }
            return String::new();
        }
    }

    no_quotes
}

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
///
/// # Wave 6: Автоматическая генерация русских падежей
///
/// При построении индекса для каждого имени (title + aliases + forms)
/// автоматически генерируются падежные формы через [`generate_russian_declensions`]
/// и добавляются в `by_alias`. Это позволяет резолвить винительный/родительный/
/// дательный/творительный/предложный падежи без явного указания их в `meta.forms`:
///
/// | Каноническое имя | Падежи в тексте | Без Wave 6 | С Wave 6 |
/// |------------------|-----------------|------------|----------|
/// | Грак             | «убил Грака»    | ❌ None    | ✅ char_grak |
/// | Ревун            | «убил Ревуна»   | ❌ None    | ✅ char_revun |
/// | Пётр             | «убил Петра»    | ❌ None    | ✅ char_petr |
/// | Алексей          | «убил Алексея»  | ❌ None    | ✅ char_alex |
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

                // Wave 6: автоматически генерируем падежные формы для
                // основного имени (Грак -> грака, граку, граком, ...).
                // Используем `entry().or_insert_with()`, чтобы явные
                // aliases/forms (добавленные ниже) имели приоритет над
                // автоматически сгенерированными формами.
                for declension in generate_russian_declensions(&node.data.title) {
                    by_alias.entry(declension).or_insert_with(|| node.id.clone());
                }
            }

            // Aliases / forms из meta — общий индекс by_alias.
            if let Some(meta) = &node.data.meta {
                if let Some(names) = extract_string_array(meta, "aliases") {
                    for name in names {
                        let lc = name.trim().to_lowercase();
                        if !lc.is_empty() {
                            // Явный alias — перезаписывает declension, если был.
                            by_alias.insert(lc, node.id.clone());
                            // И генерируем падежи от самого alias'а
                            // (например, alias "Пётр" -> петра, петру, ...).
                            for declension in generate_russian_declensions(&name) {
                                by_alias
                                    .entry(declension)
                                    .or_insert_with(|| node.id.clone());
                            }
                        }
                    }
                }
                if let Some(names) = extract_string_array(meta, "forms") {
                    for name in names {
                        let lc = name.trim().to_lowercase();
                        if !lc.is_empty() {
                            by_alias.insert(lc, node.id.clone());
                            // Forms тоже могут быть лемматизированы:
                            // form "Иван" -> ивана, ивану, ...
                            for declension in generate_russian_declensions(&name) {
                                by_alias
                                    .entry(declension)
                                    .or_insert_with(|| node.id.clone());
                            }
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
        // Wave 7: расширено с просто «сказал*» до полного набора глаголов
        // говорения: говорит/говорят (настоящее время), ответил/ответила
        // (прошедшее), спросил/спросила. Без этого диалоги «— Привет, —
        // говорит Паша» не классифицировались как Speak (action = None →
        // событие пропускалось целиком).
        speak: Regex::new(r"\b(?:сказал|сказала|сказали|говорит|говорят|ответил|ответила|ответили|спросил|спросила|спросили)\b")
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
///      - `target` (только для Kill) = первое слово **после kill-глагола**,
///        которое либо резолвится через `EntityResolver` (включая сгенерированные
///        падежные формы), либо начинается с заглавной буквы (Wave 6). Если
///        kill-глагол не найден — fallback на второе заглавное слово.
///      - `action` = соответствующий вариант.
///      - `time` = [`anchor_from_position`] от byte offset начала предложения.
///      - `confidence = 0.5` (ниже, чем у SVO).
///      - `provenance = Provenance::RustParser`.
///
/// # Ограничения
///
/// Это аварийный режим. Он не делает:
/// - лемматизацию глаголов (найдёт «убил», но не «убивать»);
/// - разрешение местоимений («он» не станет «Иваном»);
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

        // Wave 7: Очистка прямой речи.
        //
        // До Wave 7 парсер брал первое заглавное слово из ВСЕГО предложения,
        // включая содержимое кавычек «...» и реплик после тире `— ... —`.
        // Это приводило к тому, что обычные слова из реплик («Талант»,
        // «Архитектор», «Крысы», «Нет») становились фантомными актёрами.
        //
        // Теперь мы сначала вырезаем диалоговое содержимое через
        // [`strip_dialogue_content`], оставляя только авторский текст:
        // - «— Талант у тебя есть, — сказал он.» → «сказал он.»
        // - «Веня сказал: «Привет».» → «Веня сказал: .»
        //
        // Все дальнейшие regex-поиски (caps, kill-verb, speak-verb) идут
        // по очищенной строке. `source_text` в Event сохраняет оригинальное
        // предложение (для читабельности в UI).
        let clean_sentence_owned = strip_dialogue_content(sentence);
        let clean_sentence = clean_sentence_owned.as_str();

        // Если после очистки осталась пустая строка (предложение состояло
        // только из реплики без авторской атрибуции) — пропускаем.
        if clean_sentence.trim().is_empty() {
            continue;
        }

        // Список заглавных слов — потенциальных имён персонажей.
        // Берём их из очищенного предложения (без кавычек и реплик).
        let caps: Vec<String> = regexes
            .cap_word
            .find_iter(clean_sentence)
            .filter_map(|r| r.ok())
            .map(|m| m.as_str().to_string())
            .collect();

        // Определяем action по первому матчу глагола. Порядок важен: kill
        // проверяем первым — если в предложении «убил» и «сказал», Kill
        // важнее.
        let action_and_needs_target: Option<(Action, bool)> = if regexes
            .kill
            .is_match(clean_sentence)
            .unwrap_or(false)
        {
            Some((Action::Kill, true))
        } else if regexes
            .speak
            .is_match(clean_sentence)
            .unwrap_or(false)
        {
            Some((Action::Speak { topic: None }, false))
        } else if regexes
            .die
            .is_match(clean_sentence)
            .unwrap_or(false)
        {
            Some((Action::Die, false))
        } else if regexes
            .resurrect
            .is_match(clean_sentence)
            .unwrap_or(false)
        {
            Some((Action::Resurrect, false))
        } else if regexes
            .arrive
            .is_match(clean_sentence)
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

        // ── Wave 7: Actor extraction with speech attribution + stop-words ──
        //
        // Двухфазный алгоритм:
        //
        // Фаза 1 — пост-глагольная атрибуция (для Speak-глаголов).
        // В русском авторская реплика ставится после глагола говорения:
        //   «— Привет, — сказал Веня.» → автор = «Веня».
        //   «— Архитектор пришёл, — говорит Паша.» → автор = «Паша».
        // Алгоритм ищет позицию глагола сказал/говорит/ответил/спросил и
        // сканирует слова ПОСЛЕ него, выбирая первое:
        //   - известное имя (через EntityResolver, с учётом падежей), или
        //   - заглавное слово (если не стоп-слово).
        //
        // Фаза 2 — fallback на caps (если глагола нет или имя после него
        // не найдено).
        // Берём первое заглавное слово из `caps`, которое:
        //   - НЕ является стоп-словом (он/она/но/или/...), И
        //   - либо резолвится через EntityResolver, либо имеет длину > 2.
        // Длина > 2 отсекает обрывки типа «Все», «Два», но пропускает
        // реальные имена «Грак», «Паша», «Веня».
        //
        // Если ни фаза 1, ни фаза 2 не дали актёра — пропускаем событие.
        let sentence_words: Vec<&str> = clean_sentence.split_whitespace().collect();

        // Wave 7: Target вычисляется ПЕРВЫМ (для needs_target действий),
        // чтобы Phase 2 (fallback на caps) могла исключить target из
        // кандидатов на actor. Без этого «Он убил Ревуна» назначало бы
        // actor=char_revun (target leakage: «Ревуна» — единственный
        // не-стоп cap, но это жертва, а не убийца).
        let target = if needs_target {
            // Ищем позицию kill-глагола. Используем `contains`, чтобы поймать
            // формы «убил», «убил.», «убил,», «убила», «убили», а также
            // «застрелил», «погубил», «казнил» (см. kill_regex).
            let kill_verb_pos = sentence_words.iter().position(|w| {
                let w_lc = w.to_lowercase();
                w_lc.contains("убил")
                    || w_lc.contains("убить")
                    || w_lc.contains("застрелил")
                    || w_lc.contains("погубил")
                    || w_lc.contains("казнил")
                    || w_lc.contains("убивают")
            });

            let mut found_target: Option<String> = None;

            if let Some(verb_idx) = kill_verb_pos {
                // Сканируем слова ПОСЛЕ глагола.
                for &word in &sentence_words[verb_idx + 1..] {
                    // Чистим от пунктуации: «Грака» → «Грака», «Грака,» → «Грака»,
                    // ««Грака»» → «Грака», «Грака.» → «Грака» (хотя точка уже
                    // убрана sentence_split, на всякий случай).
                    let clean_word: String = word
                        .chars()
                        .filter(|c| c.is_alphabetic())
                        .collect();
                    if clean_word.is_empty() {
                        continue;
                    }

                    // 1. Проверяем резолв через EntityResolver (учитывая
                    //    сгенерированные падежи — «Грака» → char_grak).
                    if let Some(resolved_id) = resolver.resolve(&clean_word) {
                        found_target = Some(resolved_id);
                        break;
                    }

                    // 2. Если слово с заглавной буквы — потенциальное имя,
                    //    даже если не резолвится (станет phantom entity).
                    //    Wave 7: НО не стоп-слово (чтобы «убил Его» не
                    //    создавало фантома «Его»).
                    if clean_word
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_uppercase())
                        && !is_russian_stop_word(&clean_word)
                    {
                        found_target = Some(resolver.resolve_or_keep(&clean_word));
                        break;
                    }
                    // 3. Иначе (lowercase + не резолвится) — пропускаем:
                    //    «убил чиновника» — «чиновника» не является именем.
                }
            }

            // Fallback: глагол не найден — старая эвристика caps.get(1).
            // Это покрывает edge-case, когда kill_regex сматчил что-то, что
            // не покрыл наш `contains`-фильтр выше.
            found_target.or_else(|| caps.get(1).map(|n| resolver.resolve_or_keep(n)))
        } else {
            None
        };

        // ── Фаза 1: поиск автора речи после глагола говорения ──
        let mut extracted_actor: Option<String> = None;

        // Позиция глагола речи (сказал/говорит/ответил/спросил + формы).
        let speak_verb_pos = sentence_words.iter().position(|w| {
            let w_lc = w.to_lowercase();
            w_lc.contains("сказал")
                || w_lc.contains("говорит")
                || w_lc.contains("говорят")
                || w_lc.contains("ответил")
                || w_lc.contains("ответила")
                || w_lc.contains("ответили")
                || w_lc.contains("спросил")
                || w_lc.contains("спросила")
                || w_lc.contains("спросили")
        });

        if let Some(speak_idx) = speak_verb_pos {
            for &next_word in &sentence_words[speak_idx + 1..] {
                let clean: String = next_word
                    .chars()
                    .filter(|c| c.is_alphabetic())
                    .collect();
                if clean.is_empty() || is_russian_stop_word(&clean) {
                    continue;
                }
                // Wave 7: пропускаем слово, если оно совпадает с target
                // (например, «Паша спросил Веню» — «Веню» это target,
                // а не говорящий).
                if let Some(ref t) = target {
                    if resolver.resolve(&clean).as_ref() == Some(t) {
                        continue;
                    }
                }
                // Если имя известное в графе персонажей — берём его.
                if let Some(id) = resolver.resolve(&clean) {
                    extracted_actor = Some(id);
                    break;
                }
                // Если имя с заглавной буквы — потенциальное имя (даже
                // неизвестное — станет phantom entity). Длина > 2 отсекает
                // короткие обрывки.
                if clean.chars().next().map_or(false, |c| c.is_uppercase()) && clean.chars().count() > 2 {
                    extracted_actor = Some(resolver.resolve_or_keep(&clean));
                    break;
                }
            }
        }

        // ── Фаза 2: fallback на caps (с фильтром стоп-слов) ──
        let actor = match extracted_actor {
            Some(a) => a,
            None => {
                // Ищем первое заглавное слово, которое НЕ стоп-слово
                // и либо резолвится, либо имеет длину > 2.
                let valid_cap = caps.iter().find(|cap| {
                    // Wave 7: исключаем target (чтобы «Он убил Ревуна» не
                    // взяло «Ревуна» как actor).
                    if let Some(ref t) = target {
                        if resolver.resolve(cap).as_ref() == Some(t) {
                            return false;
                        }
                    }
                    // Известное имя в графе — берём, даже если оно формально
                    // совпадает со стоп-словом (крайне редкий случай).
                    if resolver.resolve(cap).is_some() {
                        return true;
                    }
                    // Иначе — не стоп-слово и длина > 2.
                    !is_russian_stop_word(cap) && cap.chars().count() > 2
                });

                match valid_cap {
                    Some(name) => resolver.resolve_or_keep(name),
                    None => continue, // Только стоп-слова или пусто → пропускаем.
                }
            }
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
        // Wave 6: автоматически сгенерированные падежи для «Иван» (5) и
        // «Пётр» (4) попадают в by_alias. Раньше было 0; теперь ≥ 9.
        assert!(
            resolver.alias_count() >= 9,
            "Без явных aliases, но с declensions для Иван (5) + Пётр (4) = 9, got {}",
            resolver.alias_count()
        );
        // Покажем, что именно падежные формы теперь резолвятся.
        assert_eq!(
            resolver.resolve("Ивана"),
            Some("char-ivan-1".to_string()),
            "Винительный/родительный падеж «Ивана» резолвится (Wave 6)"
        );
        assert_eq!(
            resolver.resolve("Петра"),
            Some("char-petr-2".to_string()),
            "Винительный/родительный падеж «Петра» резолвится (без ё, Wave 6)"
        );
        assert_eq!(
            resolver.resolve("Ивану"),
            Some("char-ivan-1".to_string()),
            "Дательный падеж «Ивану» резолвится (Wave 6)"
        );
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
        // Wave 6: к явным 6 aliases добавляются сгенерированные падежные
        // формы для каждого имени (Ваня → 4, Иван → 5, Ванюша → 4, Иоанн → 5,
        // Ваней → 3 новых, Совет → 5, Старейшины → 0). Минимум остаётся 6,
        // фактическое число больше — проверяем нижнюю границу.
        assert!(
            resolver.alias_count() >= 6,
            "Минимум 6 явных алиасов (без учёта declensions), got {}",
            resolver.alias_count()
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

    // ── Wave 6: Russian Declensions ─────────────────────────────────

    #[test]
    fn test_generate_russian_declensions_masculine_hard_consonant() {
        // Грак — типичный мужской на твёрдую согласную.
        let decls_grak = generate_russian_declensions("Грак");
        assert!(
            decls_grak.contains(&"грака".to_string()),
            "Винительный/родительный «грака» для Грак, got {:?}",
            decls_grak
        );
        assert!(
            decls_grak.contains(&"граку".to_string()),
            "Дательный «граку» для Грак, got {:?}",
            decls_grak
        );
        assert!(
            decls_grak.contains(&"граком".to_string()),
            "Творительный «граком» для Грак, got {:?}",
            decls_grak
        );
        assert!(
            decls_grak.contains(&"граке".to_string()),
            "Предложный «граке» для Грак, got {:?}",
            decls_grak
        );

        // Ревун — мужской на -н (твёрдая согласная).
        let decls_revun = generate_russian_declensions("Ревун");
        assert!(
            decls_revun.contains(&"ревуна".to_string()),
            "«ревуна» для Ревун, got {:?}",
            decls_revun
        );
        assert!(
            decls_revun.contains(&"ревуну".to_string()),
            "«ревуну» для Ревун, got {:?}",
            decls_revun
        );

        // Иван — стандартный мужской на -н.
        let decls_ivan = generate_russian_declensions("Иван");
        assert!(decls_ivan.contains(&"ивана".to_string()));
        assert!(decls_ivan.contains(&"ивану".to_string()));
        assert!(decls_ivan.contains(&"иваном".to_string()));
        assert!(decls_ivan.contains(&"иване".to_string()));
    }

    #[test]
    fn test_generate_russian_declensions_masculine_special_petr() {
        // Пётр — особый случай (беглая гласная ё→е).
        let decls = generate_russian_declensions("Пётр");
        assert!(
            decls.contains(&"петра".to_string()),
            "Винительный/родительный «петра» (без ё), got {:?}",
            decls
        );
        assert!(decls.contains(&"петру".to_string()));
        assert!(decls.contains(&"петром".to_string()));
        assert!(decls.contains(&"петре".to_string()));

        // Лев — особый случай (беглая гласная).
        let decls_lev = generate_russian_declensions("Лев");
        assert!(
            decls_lev.contains(&"льва".to_string()),
            "Винительный/родительный «льва» для Лев, got {:?}",
            decls_lev
        );
        assert!(decls_lev.contains(&"льву".to_string()));
        assert!(decls_lev.contains(&"львом".to_string()));
    }

    #[test]
    fn test_generate_russian_declensions_masculine_y() {
        // Алексей — мужской на -й.
        let decls = generate_russian_declensions("Алексей");
        assert!(
            decls.contains(&"алексея".to_string()),
            "Винительный «алексея», got {:?}",
            decls
        );
        assert!(decls.contains(&"алексею".to_string()));
        assert!(decls.contains(&"алексеем".to_string()));
        assert!(decls.contains(&"алексее".to_string()));

        // Николай — мужской на -й.
        let decls_nik = generate_russian_declensions("Николай");
        assert!(decls_nik.contains(&"николая".to_string()));
        assert!(decls_nik.contains(&"николаю".to_string()));
    }

    #[test]
    fn test_generate_russian_declensions_feminine() {
        // Марта — женский на -а.
        let decls_marta = generate_russian_declensions("Марта");
        assert!(
            decls_marta.contains(&"марту".to_string()),
            "Винительный «марту» для Марта, got {:?}",
            decls_marta
        );
        assert!(decls_marta.contains(&"марты".to_string()));
        assert!(decls_marta.contains(&"марте".to_string()));
        assert!(decls_marta.contains(&"мартой".to_string()));

        // Мария — женский на -ия.
        let decls_maria = generate_russian_declensions("Мария");
        assert!(
            decls_maria.contains(&"марию".to_string()),
            "Винительный «марию» для Мария, got {:?}",
            decls_maria
        );
        assert!(decls_maria.contains(&"марии".to_string()));
        assert!(decls_maria.contains(&"марией".to_string()));

        // Катя — женский на -я.
        let decls_katya = generate_russian_declensions("Катя");
        assert!(decls_katya.contains(&"катю".to_string()));
        assert!(decls_katya.contains(&"кати".to_string()));
    }

    #[test]
    fn test_generate_russian_declensions_edge_cases() {
        // Пустая строка → пустой вектор.
        assert!(
            generate_russian_declensions("").is_empty(),
            "Пустая строка → нет форм"
        );
        // Только пробелы → пустой вектор.
        assert!(
            generate_russian_declensions("   ").is_empty(),
            "Только пробелы → нет форм"
        );
        // Один символ → возвращается как есть (lowercase).
        let one = generate_russian_declensions("А");
        assert_eq!(one, vec!["а".to_string()], "Один символ → [lc]");

        // Case-insensitive: вход в UPPERCASE не должен ломать генерацию.
        let decls_upper = generate_russian_declensions("ГРАК");
        assert!(
            decls_upper.contains(&"грака".to_string()),
            "Uppercase input должен давать lowercase-формы, got {:?}",
            decls_upper
        );

        // Окружающие пробелы триммируются.
        let decls_trim = generate_russian_declensions("  Грак  ");
        assert!(
            decls_trim.contains(&"грака".to_string()),
            "Пробелы вокруг имени триммируются, got {:?}",
            decls_trim
        );
    }

    #[test]
    fn test_entity_resolver_russian_declension_matching() {
        // Тест из спецификации Wave 6: имена «Грака», «Ревуна» из текста
        // должны резолвиться в канонические узлы.
        let nodes = vec![
            make_node("char_grak", "Грак", "character", None),
            make_node("char_revun", "Ревун", "character", None),
            make_node("char_petr", "Пётр", "character", None),
            make_node("char_alex", "Алексей", "character", None),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);

        // Грак → винительный/родительный «Грака».
        assert_eq!(
            resolver.resolve("Грака"),
            Some("char_grak".to_string()),
            "«Грака» → char_grak (Wave 6 declension)"
        );
        assert_eq!(
            resolver.resolve("граку"),
            Some("char_grak".to_string()),
            "«граку» → char_grak (дательный)"
        );

        // Ревун → винительный «Ревуна».
        assert_eq!(
            resolver.resolve("Ревуна"),
            Some("char_revun".to_string()),
            "«Ревуна» → char_revun (Wave 6 declension)"
        );

        // Пётр → винительный «Петра» (без ё).
        assert_eq!(
            resolver.resolve("Петра"),
            Some("char_petr".to_string()),
            "«Петра» (без ё) → char_petr (беглая гласная)"
        );

        // Алексей → винительный «Алексея».
        assert_eq!(
            resolver.resolve("Алексея"),
            Some("char_alex".to_string()),
            "«Алексея» → char_alex (мужской на -й)"
        );
    }

    #[test]
    fn test_entity_resolver_declension_from_alias() {
        // Если alias — каноническая форма (напр., «Пётр»), от него тоже
        // должны сгенерироваться падежи.
        let nodes = vec![
            make_node(
                "char_petr",
                "Пётр_Главный", // title — выдуманный, чтобы не пересекался
                "character",
                Some(json!({
                    "aliases": ["Пётр"]
                })),
            ),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);

        // «Петра» должно резолвиться через alias «Пётр» → declension «петра».
        assert_eq!(
            resolver.resolve("Петра"),
            Some("char_petr".to_string()),
            "declension от alias «Пётр» → «петра» резолвится"
        );
    }

    #[test]
    fn test_parse_text_fallback_verb_relative_target_extraction() {
        // Wave 6: target ищется ПОСЛЕ kill-глагола, а не как «второе
        // заглавное слово». Это исправляет три класса ошибок.
        //
        // Примечание: используем только имена с одной заглавной буквой в начале
        // (Антон, Григорий), так как regex cap_word `[А-ЯЁ][а-яё]+` не понимает
        // camelCase (СанКор был бы разбит на «Сан» + «Кор»).

        let nodes = vec![
            make_node("char_anton", "Антон", "character", None),
            make_node("char_grigory", "Григорий", "character", None),
            make_node("char_grak", "Грак", "character", None),
            make_node("char_alex", "Алексей", "character", None),
            make_node("char_revun", "Ревун", "character", None),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 100000)];

        // Случай 1: «герой убил Ревуна» — одно заглавное слово в предложении.
        // Раньше: caps = ["Ревуна"], caps.get(1) = None → target = None.
        // Теперь: kill_verb_pos = 1, после глагола «Ревуна» → char_revun.
        let text1 = "Герой убил Ревуна.";
        let events1 = parse_text_fallback(text1, &resolver, &chapters);
        let kill1: Vec<&Event> = events1
            .iter()
            .filter(|e| matches!(e.action, Action::Kill))
            .collect();
        assert_eq!(
            kill1.len(),
            1,
            "Случай 1: должно быть 1 Kill-событие, got {:?}",
            events1
        );
        // Actor будет «Герой» (phantom — нет в resolver), но target должен
        // резолвиться через declension «Ревуна» → char_revun.
        assert_eq!(
            kill1[0].target,
            Some("char_revun".to_string()),
            "Случай 1: target «Ревуна» резолвится в char_revun через Wave 6 declension"
        );

        // Случай 2: «Григорий узнал, кто убил Грака» — два заглавных ДО глагола.
        // Раньше: caps = ["Григорий", "Грака"], caps.get(1) = "Грака" — казалось
        // бы, верно, но это совпадение (см. случай 3).
        // Теперь: kill_verb_pos ищется, после него — только «Грака».
        let text2 = "Григорий узнал, кто убил Грака.";
        let events2 = parse_text_fallback(text2, &resolver, &chapters);
        let kill2: Vec<&Event> = events2
            .iter()
            .filter(|e| matches!(e.action, Action::Kill))
            .collect();
        assert_eq!(
            kill2.len(),
            1,
            "Случай 2: должно быть 1 Kill-событие, got {:?}",
            events2
        );
        assert_eq!(
            kill2[0].actor,
            "char_grigory",
            "Случай 2: actor = Григорий (резолвится)"
        );
        assert_eq!(
            kill2[0].target,
            Some("char_grak".to_string()),
            "Случай 2: target = Грак (через «Грака» — Wave 6 declension), а НЕ «кто» (lowercase)"
        );

        // Случай 3: «Антон и Алексей убили Грака» — два заглавных субъекта
        // ДО глагола. Старая эвристика взяла бы «Алексей» как target.
        // Новая: kill_verb_pos = 3 (после «Алексей»), после него «Грака».
        let text3 = "Антон и Алексей убили Грака.";
        let events3 = parse_text_fallback(text3, &resolver, &chapters);
        let kill3: Vec<&Event> = events3
            .iter()
            .filter(|e| matches!(e.action, Action::Kill))
            .collect();
        assert_eq!(
            kill3.len(),
            1,
            "Случай 3: должно быть 1 Kill-событие, got {:?}",
            events3
        );
        assert_eq!(
            kill3[0].actor,
            "char_anton",
            "Случай 3: actor = Антон (первое заглавное)"
        );
        assert_eq!(
            kill3[0].target,
            Some("char_grak".to_string()),
            "Случай 3: target = Грак, а НЕ Алексей (тот стоит ДО глагола)"
        );

        // Случай 4: «Антон убил чиновника» — lowercase target, не резолвится.
        // Должно дать target = None (lowercase + не резолвится → пропускается).
        let text4 = "Антон убил чиновника.";
        let events4 = parse_text_fallback(text4, &resolver, &chapters);
        let kill4: Vec<&Event> = events4
            .iter()
            .filter(|e| matches!(e.action, Action::Kill))
            .collect();
        assert_eq!(
            kill4.len(),
            1,
            "Случай 4: должно быть 1 Kill-событие, got {:?}",
            events4
        );
        assert_eq!(
            kill4[0].target,
            None,
            "Случай 4: lowercase «чиновника» не резолвится и не заглавное → target = None"
        );

        // Случай 5: «Антон убил Алексея» — kill + target в винительном падеже.
        // Проверяем полный end-to-end: и actor, и target резолвятся.
        let text5 = "Антон убил Алексея.";
        let events5 = parse_text_fallback(text5, &resolver, &chapters);
        let kill5: Vec<&Event> = events5
            .iter()
            .filter(|e| matches!(e.action, Action::Kill))
            .collect();
        assert_eq!(
            kill5.len(),
            1,
            "Случай 5: должно быть 1 Kill-событие, got {:?}",
            events5
        );
        assert_eq!(
            kill5[0].actor,
            "char_anton",
            "Случай 5: actor = Антон"
        );
        assert_eq!(
            kill5[0].target,
            Some("char_alex".to_string()),
            "Случай 5: target = Алексей (через «Алексея» — Wave 6 declension от -й)"
        );
    }

    // ── Wave 7: Stop-Words & Dialogue Stripping ──────────────────────

    #[test]
    fn test_is_russian_stop_word_pronouns() {
        // Личные местоимения — все стоп-слова.
        assert!(is_russian_stop_word("Он"), "«Он» — стоп-слово");
        assert!(is_russian_stop_word("Она"), "«Она» — стоп-слово");
        assert!(is_russian_stop_word("Они"), "«Они» — стоп-слово");
        assert!(is_russian_stop_word("Оно"), "«Оно» — стоп-слово");
        assert!(is_russian_stop_word("Я"), "«Я» — стоп-слово");
        assert!(is_russian_stop_word("Мы"), "«Мы» — стоп-слово");
        assert!(is_russian_stop_word("Вы"), "«Вы» — стоп-слово");
        assert!(is_russian_stop_word("Ты"), "«Ты» — стоп-слово");
        // Case-insensitive.
        assert!(is_russian_stop_word("он"), "«он» (lowercase) — стоп-слово");
        assert!(is_russian_stop_word("ОНА"), "«ОНА» (uppercase) — стоп-слово");
    }

    #[test]
    fn test_is_russian_stop_word_conjunctions_and_particles() {
        // Союзы, частицы, ответы.
        assert!(is_russian_stop_word("Не"), "«Не» — стоп-слово");
        assert!(is_russian_stop_word("Но"), "«Но» — стоп-слово");
        assert!(is_russian_stop_word("Или"), "«Или» — стоп-слово");
        assert!(is_russian_stop_word("Как"), "«Как» — стоп-слово");
        assert!(is_russian_stop_word("Когда"), "«Когда» — стоп-слово");
        assert!(is_russian_stop_word("Потому"), "«Потому» — стоп-слово");
        assert!(is_russian_stop_word("Что"), "«Что» — стоп-слово");
        assert!(is_russian_stop_word("Это"), "«Это» — стоп-слово");
        assert!(is_russian_stop_word("Нет"), "«Нет» — стоп-слово");
        assert!(is_russian_stop_word("Да"), "«Да» — стоп-слово");
        assert!(is_russian_stop_word("Если"), "«Если» — стоп-слово");
        assert!(is_russian_stop_word("Однако"), "«Однако» — стоп-слово");
    }

    #[test]
    fn test_is_russian_stop_word_prepositions() {
        // Предлоги — частые «фантомные актёры».
        assert!(is_russian_stop_word("За"), "«За» — стоп-слово");
        assert!(is_russian_stop_word("На"), "«На» — стоп-слово");
        assert!(is_russian_stop_word("Из"), "«Из» — стоп-слово");
        assert!(is_russian_stop_word("При"), "«При» — стоп-слово");
        assert!(is_russian_stop_word("До"), "«До» — стоп-слово");
        assert!(is_russian_stop_word("После"), "«После» — стоп-слово");
        assert!(is_russian_stop_word("Для"), "«Для» — стоп-слово");
    }

    #[test]
    fn test_is_russian_stop_word_real_names_are_not_stops() {
        // Реальные имена персонажей НЕ должны быть стоп-словами.
        // Если имя случайно совпадёт со стоп-словом — caller-код должен
        // проверить resolver.resolve() ПЕРВЫМ, и только если None — звать
        // is_russian_stop_word.
        assert!(!is_russian_stop_word("Грак"), "«Грак» — НЕ стоп-слово");
        assert!(!is_russian_stop_word("Веня"), "«Веня» — НЕ стоп-слово");
        assert!(!is_russian_stop_word("Паша"), "«Паша» — НЕ стоп-слово");
        assert!(!is_russian_stop_word("Архитектор"), "«Архитектор» — НЕ стоп-слово");
        assert!(!is_russian_stop_word("Раскольников"), "«Раскольников» — НЕ стоп-слово");
    }

    #[test]
    fn test_is_russian_stop_word_edge_cases() {
        // Пустая строка — не стоп-слово (нет смысла).
        assert!(!is_russian_stop_word(""), "Пустая строка — не стоп-слово");
        // Только пробелы — тримятся, пустая строка → не стоп.
        assert!(!is_russian_stop_word("   "), "Только пробелы — не стоп-слово");
        // Пробелы вокруг реального стоп-слова — тримятся.
        assert!(is_russian_stop_word("  Он  "), "«  Он  » → «Он» — стоп-слово");
    }

    #[test]
    fn test_strip_dialogue_content_guillemets() {
        // «...» — французские кавычки, самые частые в русском.
        let s = "Веня сказал: «Привет, друг».";
        let cleaned = strip_dialogue_content(s);
        assert!(
            !cleaned.contains("Привет"),
            "Содержимое «...» удалено, got: {:?}",
            cleaned
        );
        assert!(
            cleaned.contains("Веня сказал"),
            "Авторский текст сохранён, got: {:?}",
            cleaned
        );
        assert!(
            !cleaned.contains("«") && !cleaned.contains("»"),
            "Кавычки удалены, got: {:?}",
            cleaned
        );
    }

    #[test]
    fn test_strip_dialogue_content_typographic_quotes() {
        // "..." — типографские кавычки.
        let s = "Паша ответил: \u{201C}Привет\u{201D}.";
        let cleaned = strip_dialogue_content(s);
        assert!(
            !cleaned.contains("Привет"),
            "Содержимое «\u{201C}...\u{201D}» удалено, got: {:?}",
            cleaned
        );
        assert!(cleaned.contains("Паша"), "Автор сохранён");
    }

    #[test]
    fn test_strip_dialogue_content_ascii_quotes() {
        // "..." — ASCII кавычки.
        let s = "Веня сказал: \"Привет\".";
        let cleaned = strip_dialogue_content(s);
        assert!(
            !cleaned.contains("Привет"),
            "Содержимое ASCII-кавычек удалено, got: {:?}",
            cleaned
        );
        assert!(cleaned.contains("Веня"), "Автор сохранён");
    }

    #[test]
    fn test_strip_dialogue_content_dash_dialogue_with_attribution() {
        // — Привет, — сказал Веня.
        // Два тире: открывающее и закрывающее реплику. После второго — автор.
        let s = "— Привет, — сказал Веня.";
        let cleaned = strip_dialogue_content(s);
        assert_eq!(
            cleaned, "сказал Веня.",
            "Берётся текст после последнего тире (авторская атрибуция), got: {:?}",
            cleaned
        );
        assert!(
            !cleaned.contains("Привет"),
            "Содержимое реплики удалено"
        );
    }

    #[test]
    fn test_strip_dialogue_content_dash_dialogue_without_attribution() {
        // — Привет.
        // Только одно тире (открывающее), без авторской атрибуции.
        // Должно вернуть пустую строку → caller пропустит событие.
        let s = "— Привет.";
        let cleaned = strip_dialogue_content(s);
        assert!(
            cleaned.is_empty(),
            "Диалог без атрибуции → пустая строка, got: {:?}",
            cleaned
        );
    }

    #[test]
    fn test_strip_dialogue_content_no_dialogue_passthrough() {
        // Обычное предложение без кавычек и тире — проходит как есть.
        let s = "Грак убил Ревуна.";
        let cleaned = strip_dialogue_content(s);
        assert_eq!(
            cleaned, s,
            "Без кавычек/тире — строка не меняется"
        );
    }

    #[test]
    fn test_strip_dialogue_content_em_dash_in_author_text() {
        // Em-dash в авторском тексте (не в начале) — НЕ считается диалогом.
        // «Паша — мой друг.» — предложение не начинается с тире.
        let s = "Паша — мой друг.";
        let cleaned = strip_dialogue_content(s);
        assert_eq!(
            cleaned, s,
            "Em-dash не в начале — не активирует диалог-режим, got: {:?}",
            cleaned
        );
    }

    #[test]
    fn test_parse_text_fallback_speech_attribution_after_verb() {
        // Wave 7, Баг #3: автор реплики стоит ПОСЛЕ глагола говорения.
        //
        // «— Привет, — сказал Веня.»
        // До Wave 7: actor = «Привет» (первое заглавное слово в реплике).
        // Теперь: actor = «Веня» (имя после «сказал»).
        let nodes = vec![
            make_node("char_venya", "Веня", "character", None),
            make_node("char_pasha", "Паша", "character", None),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 100000)];

        // Случай 1: «— Привет, — сказал Веня.» → actor = Веня.
        let text1 = "— Привет, — сказал Веня.";
        let events1 = parse_text_fallback(text1, &resolver, &chapters);
        let speak1: Vec<&Event> = events1
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        assert_eq!(
            speak1.len(),
            1,
            "Случай 1: должно быть 1 Speak-событие, got: {:?}",
            events1
        );
        assert_eq!(
            speak1[0].actor, "char_venya",
            "Случай 1: actor = Веня (через пост-глагольную атрибуцию), а НЕ «Привет»"
        );

        // Случай 2: «— Архитектор пришёл, — говорит Паша.» → actor = Паша.
        let text2 = "— Архитектор пришёл, — говорит Паша.";
        let events2 = parse_text_fallback(text2, &resolver, &chapters);
        let speak2: Vec<&Event> = events2
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        assert_eq!(
            speak2.len(),
            1,
            "Случай 2: должно быть 1 Speak-событие, got: {:?}",
            events2
        );
        assert_eq!(
            speak2[0].actor, "char_pasha",
            "Случай 2: actor = Паша (через «говорит»), а НЕ «Архитектор»"
        );

        // Случай 3: «— Пришёл, — сказал Веня, не оборачиваясь.»
        // Дополнительный текст после имени не должен ломать атрибуцию.
        let text3 = "— Пришёл, — сказал Веня, не оборачиваясь.";
        let events3 = parse_text_fallback(text3, &resolver, &chapters);
        let speak3: Vec<&Event> = events3
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        assert_eq!(speak3.len(), 1, "Случай 3: 1 Speak, got: {:?}", events3);
        assert_eq!(
            speak3[0].actor, "char_venya",
            "Случай 3: actor = Веня, несмотря на текст после имени"
        );

        // Случай 4: «— Нет, — сказал я.» → нет валидного актёра.
        // «я» — стоп-слово. caps из «сказал я.» = [] (я в нижнем регистре).
        // Событие должно быть пропущено.
        let text4 = "— Нет, — сказал я.";
        let events4 = parse_text_fallback(text4, &resolver, &chapters);
        assert!(
            events4.is_empty(),
            "Случай 4: «— Нет, — сказал я.» → пропущено (нет валидного актёра), got: {:?}",
            events4
        );

        // Случай 5: «— Талант у тебя есть, — сказал он.»
        // «он» — стоп-слово. Событие должно быть пропущено.
        // (До Wave 7 это создавало фантомного актёра «Талант».)
        let text5 = "— Талант у тебя есть, — сказал он.";
        let events5 = parse_text_fallback(text5, &resolver, &chapters);
        assert!(
            events5.is_empty(),
            "Случай 5: «— Талант у тебя есть, — сказал он.» → пропущено, got: {:?}",
            events5
        );
    }

    #[test]
    fn test_parse_text_fallback_speak_attribution_with_unknown_speaker() {
        // Если после глагола говорения стоит неизвестное имя с заглавной буквы,
        // оно становится phantom entity (как и раньше для caps).
        let resolver = EntityResolver::from_nodes(&[]);
        let chapters = vec![make_chapter(1, 0, 100000)];

        // «— Привет, — сказал Призрак.» → actor = "Призрак" (phantom).
        let text = "— Привет, — сказал Призрак.";
        let events = parse_text_fallback(text, &resolver, &chapters);
        let speak: Vec<&Event> = events
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        assert_eq!(speak.len(), 1, "1 Speak-событие");
        assert_eq!(
            speak[0].actor, "Призрак",
            "Неизвестное имя сохраняется как phantom entity"
        );
    }

    #[test]
    fn test_parse_text_fallback_pronoun_does_not_override_known_name() {
        // Если «Он» — реально имя персонажа в графе, оно должно
        // использоваться (stop-word фильтр не должен блокировать known name).
        let nodes = vec![make_node("char_on", "Он", "character", None)];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 100000)];

        // «Он сказал.» — «Он» в графе → actor = char_on.
        let text = "Он сказал.";
        let events = parse_text_fallback(text, &resolver, &chapters);
        assert_eq!(events.len(), 1, "1 событие");
        assert_eq!(
            events[0].actor, "char_on",
            "«Он» — известное имя в графе → резолвится, stop-word фильтр не блокирует"
        );
    }

    #[test]
    fn test_parse_text_fallback_extended_speak_verbs() {
        // Wave 7: speak regex расширен — теперь ловит «говорит», «ответил»,
        // «спросил» (раньше только «сказал*»).
        let nodes = vec![
            make_node("char_pasha", "Паша", "character", None),
            make_node("char_venya", "Веня", "character", None),
            make_node("char_anna", "Анна", "character", None),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 100000)];

        // «говорит Паша» — настоящее время.
        let text1 = "— Привет, — говорит Паша.";
        let events1 = parse_text_fallback(text1, &resolver, &chapters);
        let speak1: Vec<&Event> = events1
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        assert_eq!(speak1.len(), 1, "«говорит» → Speak, got: {:?}", events1);
        assert_eq!(speak1[0].actor, "char_pasha");

        // «ответил Веня» — прошедшее время.
        let text2 = "— Привет, — ответил Веня.";
        let events2 = parse_text_fallback(text2, &resolver, &chapters);
        let speak2: Vec<&Event> = events2
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        assert_eq!(speak2.len(), 1, "«ответил» → Speak, got: {:?}", events2);
        assert_eq!(speak2[0].actor, "char_venya");

        // «спросила Анна» — женский род прошедшего.
        // Особенность: внутри реплики есть `?`, который разбивает предложение
        // на 2 части: «— Где?» и «— спросила Анна.». Вторая часть должна
        // распознаться как авторская (через эвристику speech-verb после `—`).
        let text3 = "— Где? — спросила Анна.";
        let events3 = parse_text_fallback(text3, &resolver, &chapters);
        let speak3: Vec<&Event> = events3
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        assert_eq!(speak3.len(), 1, "«спросила» → Speak, got: {:?}", events3);
        assert_eq!(speak3[0].actor, "char_anna");
    }

    #[test]
    fn test_parse_text_fallback_target_excluded_from_actor() {
        // Wave 7: target не должен становиться actor при fallback на caps.
        //
        // «Он убил Ревуна.» — caps = ["Он", "Ревуна"].
        // «Он» — стоп-слово. «Ревуна» резолвится в char_revun (через Wave 6
        // declension). Без target-exclusion «Ревуна» стало бы actor'ом
        // (target leakage). Теперь: «Ревуна» исключается как target →
        // не остаётся валидного actor → событие пропускается.
        let nodes = vec![
            make_node("char_grak", "Грак", "character", None),
            make_node("char_revun", "Ревун", "character", None),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 100000)];

        let text = "Он убил Ревуна.";
        let events = parse_text_fallback(text, &resolver, &chapters);
        assert_eq!(
            events.len(),
            0,
            "«Он убил Ревуна» → пропущено: «Он» стоп-слово, «Ревуна» это \
             target (исключён из actor-кандидатов). Got: {:?}",
            events
        );

        // «Но убил Ревуна.» — «Но» стоп-слово, «Ревуна» target → пропущено.
        let text2 = "Но убил Ревуна.";
        let events2 = parse_text_fallback(text2, &resolver, &chapters);
        assert_eq!(events2.len(), 0, "«Но» — стоп-слово, got: {:?}", events2);

        // «Или они убивают Ревуна.» — стоп-слова «Или», «они».
        let text3 = "Или они убивают Ревуна.";
        let events3 = parse_text_fallback(text3, &resolver, &chapters);
        assert_eq!(events3.len(), 0, "«Или/они» — стоп-слова, got: {:?}", events3);
    }

    #[test]
    fn test_parse_text_fallback_dash_split_dialogue_attribution() {
        // Wave 7: когда `?` или `!` внутри реплики разбивает предложение,
        // авторская часть (после второго `—`) должна распознаваться отдельно.
        //
        // «— Где? — спросила Анна.» разбивается sentence_split'ом на:
        //   1. «— Где?»           → чистая реплика (1 тире, не speech verb) → skip.
        //   2. «— спросила Анна.» → 1 тире, но первое слово «спросила» —
        //      speech verb → авторский текст → Speak(Анна).
        let nodes = vec![make_node("char_anna", "Анна", "character", None)];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 100000)];

        let text = "— Где? — спросила Анна.";
        let events = parse_text_fallback(text, &resolver, &chapters);
        let speak: Vec<&Event> = events
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        assert_eq!(speak.len(), 1, "Должно быть 1 Speak (Анна), got: {:?}", events);
        assert_eq!(speak[0].actor, "char_anna");
    }

    #[test]
    fn test_parse_text_fallback_kill_inside_dialogue_uses_speaker_not_victim() {
        // Wave 7: kill-глагол внутри реплики не должен создавать Kill-событие
        // с target из реплики — событие должно быть Speak с реальным автором.
        let nodes = vec![
            make_node("char_pasha", "Паша", "character", None),
            make_node("char_grak", "Грак", "character", None),
            make_node("char_revun", "Ревун", "character", None),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 100000)];

        // «— Грак убил Ревуна, — сказал Паша.»
        // После strip_dialogue_content: «сказал Паша.»
        // Actor = Паша (атрибуция). Action = Speak (сказал).
        // Kill-глагол «убил» в самой реплике — вырезан, не должен
        // классифицировать событие как Kill.
        let text = "— Грак убил Ревуна, — сказал Паша.";
        let events = parse_text_fallback(text, &resolver, &chapters);
        let speak: Vec<&Event> = events
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .collect();
        let kill: Vec<&Event> = events
            .iter()
            .filter(|e| matches!(e.action, Action::Kill))
            .collect();
        assert_eq!(speak.len(), 1, "Должно быть 1 Speak (атрибуция Паша)");
        assert_eq!(
            kill.len(),
            0,
            "Kill-глагол в реплике не должен создавать Kill-событие"
        );
        assert_eq!(speak[0].actor, "char_pasha");
    }

    #[test]
    fn test_parse_text_fallback_multiple_sentences_mixed() {
        // Wave 7: комплексный тест — несколько предложений разного типа.
        let nodes = vec![
            make_node("char_venya", "Веня", "character", None),
            make_node("char_grak", "Грак", "character", None),
            make_node("char_revun", "Ревун", "character", None),
        ];
        let resolver = EntityResolver::from_nodes(&nodes);
        let chapters = vec![make_chapter(1, 0, 100000)];

        let text = "— Привет, — сказал Веня. Грак убил Ревуна. Но он не умер.";
        let events = parse_text_fallback(text, &resolver, &chapters);

        // Sentence 1: «— Привет, — сказал Веня.» → Speak(actor=Веня). ✅
        // Sentence 2: «Грак убил Ревуна.» → Kill(actor=Грак, target=Ревун). ✅
        // Sentence 3: «Но он не умер.» → caps = [], action=Die (умер),
        //   но «Но» — стоп-слово, других caps нет → пропущено. ✅

        let speak_count = events
            .iter()
            .filter(|e| matches!(e.action, Action::Speak { .. }))
            .count();
        let kill_count = events
            .iter()
            .filter(|e| matches!(e.action, Action::Kill))
            .count();
        let die_count = events
            .iter()
            .filter(|e| matches!(e.action, Action::Die))
            .count();

        assert_eq!(speak_count, 1, "1 Speak (Веня)");
        assert_eq!(kill_count, 1, "1 Kill (Грак → Ревун)");
        assert_eq!(
            die_count, 0,
            "«Но он не умер» — пропущено (стоп-слово «Но», без валидного actor)"
        );
    }
}
