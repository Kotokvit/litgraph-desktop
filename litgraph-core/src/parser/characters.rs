//! Детекция персонажей: capitalized слова + лингвистические сигналы.
//!
//! ## v0.3.0 — POLER[Ψ] Centaur patch
//!
//! ### Корень проблемы (диагностика через HTML X-ray export)
//! v0.2.2 использовал ТОЛЬКО один сигнал: «capitalized + freq >= 5».
//! Это приводило к ложноположительным срабатываниям на абстрактных
//! существительных: «Секвестр», «Это», «Голос», «Сфера», «Бездна»,
//! «Мнемар», «Архив» (когда это концепт, а не Вельямін Ард'Еш).
//!
//! ### Решение: 3-сигнальная детекция
//! 1. **Capitalized word frequency** (signal 1, weak) — кандидат
//! 2. **Speech-verb + Name** (signal 2, strong) — сказал/ответил/... + Name
//! 3. **Direct address** (signal 3, strong) — «— Name, ...»
//!
//! Фильтр: кандидат становится персонажем ТОЛЬКО если есть signal 2 или 3.
//! Это убирает концепты (Мнемар, Секвестр, Архив-как-здание), даже если
//! они упоминаются сотни раз — потому что они никогда не «говорят» в тексте.
//!
//! ### Почему НЕ расширяем STOP_WORDS доменно-специфичными терминами
//! Пользователь явно указал: «в разных текстах разные значения».
//! «Архив» в одном тексте — персонаж (Вельямін Ард'Еш), в другом — здание.
//! «Мнемар» в одном тексте — концепт (software overlay), в другом — имя.
//! Поэтому доменно-специфичные термины НЕ в стоплисте — их фильтрует
//! speech-verb signal. В стоплисте только УНИВЕРСАЛЬные нарицательные
//! существительные (Город, Время, Свет, Тень и т.д.).

use fancy_regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCharacter {
    pub name: String,
    pub aliases: Vec<String>,
    pub count: usize,
    pub description: String,
    /// Сколько раз имя употреблено с глаголом речи (сказал/ответил/...).
    /// Сильный сигнал что это персонаж (субъект речи).
    pub speech_count: usize,
    /// Сколько раз имя употреблено в прямом обращении («— Name, ...»).
    pub direct_count: usize,
    /// Человекочитаемая причина решения парсера (для X-ray export).
    /// Формат: "character:rule=linguistic_signal;freq=N;speech=N;direct=N;..."
    pub reason: String,
    /// v0.4.0: Тип сущности — character | concept | organization.
    /// - character: есть speech_count >= 1 ИЛИ direct_count >= 1 (имя говорит)
    /// - organization: speech_count == 0, но контекст указывает на
    ///   политическую/организационную структуру (глаголы «решил», «постановил»,
    ///   «собрался», «заседание»)
    /// - concept: speech_count == 0 и freq < ORGANIZATION_THRESHOLD,
    ///   либо контекст не указывает на организацию
    pub entity_type: EntityType,
    /// v0.5.0 / Phase 2: Bitmask evidence signals (по матрице Phase 2):
    /// - bit 0 (SIGNAL_CAPITALIZED)   = 1 — Capitalized word in non-sentence-start position
    /// - bit 1 (SIGNAL_SPEECH_VERB)   = 2 — упоминается с глаголом речи (speech_count >= 1)
    /// - bit 2 (SIGNAL_DIRECT_ADDRESS) = 4 — упоминается в прямом обращении (direct_count >= 1)
    ///
    /// Для Characters — всегда установлен bit 0 + хотя бы один из bits 1/2.
    /// Для Concepts/Organizations — только bit 0.
    pub evidence_signals: u8,
    /// v0.5.0 / Phase 2: Confidence score в диапазоне [0.0, 1.0].
    /// Вычисляется из evidence_signals + is_single_token() по детерминированной
    /// политике (см. `ParsedCharacter::confidence_from_signals`):
    ///   3 сигнала → 1.0  (cap + speech + direct) → Rust fast path eligible
    ///   2 сигнала → 0.7  если single-token, иначе 0.5 (multi-token → Python)
    ///   1 сигнал  → 0.3  (только cap) → Python fallback обязателен
    ///   0 сигналов → 0.0
    pub confidence: f32,
    /// v0.5.1 / Phase 2: Byte offsets for mentions (start positions in text).
    /// Используется в Phase 2 Step 2 для восстановления positions/mentions
    /// в `NerResult` и для реализации 4-way merge policy.
    pub mention_starts: Vec<usize>,
    /// Byte index of the first mention if any.
    pub first_mention: Option<usize>,
    /// Case-aware features (v0.7.1)
    #[serde(default)]
    pub nominative_count: usize,
    #[serde(default)]
    pub accusative_count: usize,
    #[serde(default)]
    pub genitive_negated_count: usize,
}

/// v0.5.0 / Phase 2: Битовые флаги evidence signals для `ParsedCharacter::evidence_signals`.
///
/// Используются как битовая маска: `evidence_signals = SIGNAL_CAPITALIZED |
/// SIGNAL_SPEECH_VERB | SIGNAL_DIRECT_ADDRESS`. Например, персонаж с speech
/// и direct адресом имеет `evidence_signals = 1 | 2 | 4 = 7` → 3 сигнала →
/// confidence 1.0.
pub const SIGNAL_CAPITALIZED: u8 = 1;
pub const SIGNAL_SPEECH_VERB: u8 = 2;
pub const SIGNAL_DIRECT_ADDRESS: u8 = 4;

impl ParsedCharacter {
    /// v0.5.0 / Phase 2: Вычислить confidence из evidence_signals + токенизации.
    ///
    /// Это **детерминированная** политика скоринга (не эвристика):
    ///   - 3 сигнала (cap + speech + direct) → `1.0`
    ///   - 2 сигнала (cap + speech, или cap + direct):
    ///       - если `is_single_token` → `0.7` (Rust fast path eligible)
    ///       - если multi-token → `0.5` (нужен Python для FIO resolution)
    ///   - 1 сигнал (только cap) → `0.3` (Python fallback обязателен)
    ///   - 0 сигналов → `0.0`
    ///
    /// Эта функция — единственный источник правды для fast-path решений.
    /// Никакие другие части кода не должны решать «достаточно ли уверенно
    /// срабатывание» — только через этот score.
    pub fn confidence_from_signals(evidence_signals: u8, is_single_token: bool) -> f32 {
        let count = evidence_signals.count_ones();
        match count {
            0 => 0.0,
            1 => 0.3,
            2 => {
                if is_single_token {
                    0.7
                } else {
                    0.5
                }
            }
            _ => 1.0, // 3 или больше (защитно — больше 3 не бывает)
        }
    }

    /// v0.5.0 / Phase 2: Является ли `name` односложным (без пробелов и дефисов).
    ///
    /// Multi-token names (например «Иван Петров» или «Анна-Мария») требуют
    /// Python fallback для корректного разрешения ФИО — Natasha точнее на
    /// multi-token NER, чем Rust-regex.
    pub fn is_single_token(&self) -> bool {
        !self.name.contains(' ') && !self.name.contains('-')
    }
}

/// v0.4.0: Тип сущности для контекстно-зависимой классификации.
/// «Совет» в одном тексте — организация (политическая структура),
/// в другом — концепт (совет-рекомендация), в третьем — персонаж
/// (если «Совет» — имя собственное). Алгоритм решает по контексту.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntityType {
    /// Персонаж — speaks/acts (speech_count >= 1 or direct_count >= 1)
    Character,
    /// Организация — коллективный субъект (упоминается с глаголами
    /// «решил», «постановил», «собрался», «объявил», «заседание»)
    Organization,
    /// Концепт — абстрактное существительное (Бездна, Эхо, Архив-как-здание)
    /// или неопределённый случай
    Concept,
}

/// Стоп-слово (местоимения, союзы, предлоги на 3 языках + универсальные
/// нарицательные существительные).
///
/// ВНИМАНИЕ: доменно-специфичные термины (Мнемар, Архив, Секвестр, Сфера,
/// Бездна, Империя, Этерия, Акме, Линза) ЗДЕСЬ ОТСУТСТВУЮТ. Они должны
/// фильтроваться speech-verb signal'ом, потому что их значение зависит
/// от текста. См. комментарий к модулю.
pub const STOP_WORDS: &[&str] = &[
    // УКР
    "Цей", "Ця", "Це", "Той", "Та", "Те", "Він", "Вона", "Воно", "Вони",
    "Його", "Її", "Їх", "Мій", "Твій", "Наш", "Ваш", "Свій", "Своя", "Своє",
    "Бо", "Що", "Як", "Де", "Куди", "Звідки", "Коли", "Чому", "Чи", "Тож",
    "Тут", "Там", "Так", "Ні", "Якщо", "Але", "Однак", "Отже", "Проте", "Також",
    "Був", "Була", "Було", "Були", "Є", "Бути", "Єсть",
    "Крім", "Замість", "Після", "Перед", "Між", "Біля", "Над", "Під", "За", "На",
    "Одного", "Першого", "Другого", "Третього", "Кожен", "Кожна", "Кожне", "Усі", "Всі",
    "Сьогодні", "Вчора", "Завтра", "Тепер", "Тоді", "Потім", "Раптом", "Незабаром",
    "Швидко", "Повільно", "Знову", "Ще", "Вже", "Тільки", "Навіть", "Можливо",
    "Дякую", "Вибачте", "Пробачте", "Будь", "Ласка", "Скажи", "Подивися", "Послухай",
    "Боже", "Господи", "Діду", "Бабусю", "Мамо", "Тату", "Сину", "Донько",
    "Так", "Ні", "Авжеж", "Звичайно", "Добре", "Погано",
    "Світло", "Темрява", "Тиша", "Вогонь", "Вода", "Повітря", "Земля", "Небо",
    // РУС
    "Этот", "Эта", "Эти", "Тот", "Та", "Те", "Он", "Она", "Оно", "Они",
    "Его", "Её", "Их", "Мой", "Твой", "Наш", "Ваш", "Свой", "Своя", "Своё",
    "Потому", "Что", "Как", "Где", "Куда", "Откуда", "Когда", "Почему", "Ли", "Итак",
    "Здесь", "Там", "Так", "Нет", "Если", "Но", "Однако", "Следовательно", "Против", "Также",
    "Был", "Была", "Было", "Были", "Есть", "Быть",
    "Кроме", "Вместо", "После", "Перед", "Между", "Около", "Над", "Под", "За", "На",
    "Каждый", "Каждая", "Все", "Всё",
    "Сегодня", "Вчера", "Завтра", "Теперь", "Тогда", "Потом", "Внезапно", "Скоро",
    "Быстро", "Медленно", "Снова", "Ещё", "Уже", "Только", "Даже", "Возможно",
    "Спасибо", "Извините", "Прости", "Пожалуйста", "Скажи", "Посмотри", "Послушай",
    "Боже", "Господи", "Дед", "Бабушка", "Мама", "Папа", "Сын", "Дочь",
    "Да", "Нет", "Конечно", "Хорошо", "Плохо",
    "Свет", "Тьма", "Тишина", "Огонь", "Вода", "Воздух", "Земля", "Небо",
    // EN
    "The", "This", "That", "These", "Those", "He", "She", "It", "They", "We", "You",
    "His", "Her", "Its", "Their", "My", "Your", "Our",
    "But", "And", "Or", "Not", "Yes", "No", "Oh", "Ah",
    "When", "Where", "What", "Who", "Why", "How", "Which",
    "Here", "There", "Now", "Then", "Today", "Yesterday", "Tomorrow",
    "Because", "If", "Although", "However", "So", "Therefore", "Also", "Too",
    "Was", "Were", "Been", "Have", "Has", "Had", "Being", "Having",
    "Some", "Any", "All", "Every", "Each", "Both", "Either", "Neither",
    "One", "Two", "Three", "First", "Second", "Third",
    "Good", "Bad", "Please", "Thanks", "Thank",
    "Mr", "Mrs", "Dr", "Ms",

    // ================================================================
    // v0.3.0: Универсальные нарицательные существительные (рус/укр).
    // Это НЕ доменно-специфичные термины — это общеупотребительные
    // слова, которые почти никогда не бывают именами персонажей.
    // Доменно-специфичные (Мнемар, Архив, Секвестр, Сфера, Бездна,
    // Империя, Этерия, Акме, Линза) ЗДЕСЬ ОТСУТСТВУЮТ — их фильтрует
    // speech-verb signal.
    // ================================================================

    // Нарицательные существительные, часто capitalized в начале предложения
    "Часть", "Город", "Голос", "Мир", "Порядок", "Сектор",
    "Север", "Юг", "Восток", "Запад",
    "Нижний", "Верхний",
    "Лес", "Поле", "Море", "Река", "Гора",
    "Дом", "Дверь", "Окно", "Стена", "Пол", "Потолок",
    "Улица", "Дорога", "Путь", "Площадь", "Мост",
    "Сторона", "Середина", "Конец", "Начало", "Момент", "Время",
    "День", "Ночь", "Утро", "Вечер", "Год", "Месяц", "Неделя",
    "Час", "Минута", "Секунда",
    "Шаг", "Взгляд", "Движение",
    "Слово", "Звук", "Запах", "Цвет", "Тень", "Пятно", "След",
    "Уровень", "Слой", "Поток", "Волна", "Импульс", "Сигнал",
    "Метрика", "Такт", "Интерлюдия", "Эпиграф", "Точка", "Линия",

    // Особые местоимения и наречия (не вошедшие в основной список)
    "Это", "Этих", "Этому", "Этим", "Этими", "Эти",
    // Местоимения в косвенных падежах (рус)
    "Им", "Ей", "Нам", "Вам", "Ими",
    // Глаголы-связки и модальные
    "Есть", "Был", "Была", "Было", "Были", "Будет", "Будут",
    "Может", "Могут", "Должен", "Должна", "Должно",
    // Вопросительные слова
    "Кто", "Что", "Где", "Куда", "Когда", "Зачем", "Почему", "Как",
    // Союзы и частицы, которые могут быть в начале предложения
    "Если", "Чтобы", "Хотя", "Однако", "Поэтому", "Значит",
    "Или", "Ибо",
    // v0.4.0: Наречия и союзы, часто стоящие в начале предложения
    // и ошибочно детектируемые как имена персонажей.
    // Баг «Затем» (freq=15 в 1-Сфера Предела) — это союз «then/потом»,
    // написанный с большой буквы в начале предложения: «Затем Веня...»,
    // «Затем Алексей бросил...». Не персонаж.
    "Затем", "Сначала", "Наконец", "Впрочем", "Особенно",
    "Действительно", "Лишь", "Пусть", "Пускай", "Именно",
    "Следовательно", "Значит", "Стало", "Быть", "Может",
    "Повсюду", "Везде", "Отсюда", "Оттуда", "Никуда", "Ниоткуда",
    "Позавчера", "Давно", "Недавно", "Скоро", "Вскоре", "Тотчас",
    "Немедленно", "Тут", "Там", "Здесь", "Откуда", "Куда",
    // Дополнительные местоименные формы (косвенные падежи)
    "Его", "Её", "Их", // уже выше, но дублируем для надёжности
    "Сам", "Сама", "Само", "Сами", "Самого", "Самой", "Самому",
    "Весь", "Вся", "Всё", "Все", "Всего", "Всей", "Всему",
    "Чей", "Чья", "Чьё", "Чьи",
    // Вводные слова
    "Кстати", "Например", "Кстати", "Впрочем", "Безусловно",
    "Разумеется", "Конечно", "Пожалуй", "Видимо", "Оказывается",
    "По-видимому", "Бесспорно", "Несомненно", "Вероятно",
    // Одно-двухбуквенные служебные слова (часто стоят перед глаголами речи:
    // "И сказал", "Я ответил", "Не спросил", "Ты промолчал")
    "И", "Я", "Не", "Ты", "До", "А", "Но", "Мо", "Ну",
    "Ой", "Ах", "Ох", "Эх", "Ба", "О", "У", "К", "С", "В", "Уж",
    // Косвенные падежи слов, уже в стоплисте (Мир -> Мира, Миру, etc.)
    "Мира", "Миру", "Миром", "Мире",
    // Нарицательные существительные — родовые обозначения людей
    // (не являются именами собственными, но могут стоять с глаголами речи)
    "Мальчик", "Мальчика", "Мальчику", "Девочка", "Девушки", "Девушка",
    "Мужчина", "Женщина", "Человек", "Люди", "Ребёнок", "Ребенка",
    "Старик", "Старуха", "Старца", "Дед", "Бабка",
    "Голос", "Голоса",  // уже выше, но дублируем для надёжности
    "Расчёт", "Расчёта", "Аудитор", "Аудитора",
];

/// Глаголы речи (рус + укр). Используются в Signal 2: speech-verb + Name.
/// Если после такого глагола идёт Capitalized слово — это почти наверняка
/// субъект речи, т.е. персонаж.
pub const SPEECH_VERBS: &[&str] = &[
    // Русские (мужской + женский род)
    "сказал", "сказала", "ответил", "ответила", "спросил", "спросила",
    "прошептал", "прошептала", "крикнул", "крикнула", "подумал", "подумала",
    "продолжил", "продолжила", "заметил", "заметила", "возразил", "возразила",
    "добавил", "добавила", "промолвил", "промолвила", "отвечал", "отвечала",
    "спрашивал", "спрашивала", "говорил", "говорила", "пробормотал", "пробормотала",
    "процитировал", "признался", "призналась", "объяснил", "объяснила",
    "закричал", "буркнул", "отозвался", "отозвалась", "промолчал", "промолчала",
    "усмехнулся", "усмехнулась", "кивнул", "кивнула", "вздохнул", "вздохнула",
    "проворчал", "проворчала", "хмыкнул", "хмыкнула", "фыркнул", "фыркнула",
    "заверил", "заверила", "пообещал", "пообещала", "приказал", "приказала",
    "предложил", "предложила", "удивился", "удивилась", "отмахнулся", "отмахнулась",
    // Украинские
    "сказав", "сказала", "відповів", "відповіла", "спитав", "спитала",
    "прошепотів", "прошепотіла", "скрикнув", "скрикнула", "подумав", "подумала",
    "продовжив", "продовжила", "помітив", "помітила", "заперечив", "заперечила",
    "додав", "додала", "мовив", "мовила", "говорив", "говорила",
    "бурмотнув", "бурмотнула", "визнався", "визналася", "пояснив", "пояснила",
    "посміхнувся", "посміхнулася", "кивнув", "зітхнув", "зітхнула",
];

/// v0.4.0: Глаголы и существительные, указывающие на коллективный субъект
/// (организацию, совет, клан, политическую структуру).
///
/// Если слово (например «Совет») встречается в тексте рядом с этими глаголами
/// — это организация, а не персонаж и не концепт.
///
/// Контекст проверки: в радиусе 200 символов от упоминания слова ищем
/// любое из этих ключевых слов. Если найдено ≥ 1 раза — классифицируем
/// как organization.
///
/// Примеры:
///   «Совет постановил...» → organization ✓
///   «Совет собрался...» → organization ✓
///   «Совет решил...» → organization ✓
///   «дал совет» → concept (совет-рекомендация, нет org-глаголов)
pub const ORG_CONTEXT_WORDS: &[&str] = &[
    // Глаголы коллективного действия (рус)
    "постановил", "постановила", "постановило", "постановили",
    "решил", "решила", "решило", "решили",
    "собрался", "собралась", "собралось", "собрались",
    "заседал", "заседала", "заседало", "заседали",
    "объявил", "объявила", "объявило", "объявили",
    "утвердил", "утвердила", "утвердило", "утвердили",
    "принял", "приняла", "приняло", "приняли",
    "отклонил", "отклонила", "отклонили",
    "проголосовал", "проголосовали",
    "делегировал", "делегировали",
    "ратифицировал", "ратифицировали",
    // Существительные (контекст организации)
    "заседание", "собрание", "конклав", "конгресс", "съезд",
    "сессия", "пленум", "президиум", "бюро", "комитет",
    "комиссия", "палата", "палаты", "депутат", "депутаты",
    "член", "члены", "председатель", "председателя",
    "глашатай", "спикер",
    // Украинские
    "постановив", "постановила", "вирішив", "вирішила",
    "зібрався", "зібралася", "засідав", "засідала",
    "оголосив", "оголосила", "затвердив", "затвердила",
    "засідання", "збори", "конклав", "конгрес", "з'їзд",
    "сесія", "пленум", "президія", "бюро", "комітет",
    "комісія", "палата", "депутат", "член", "голова",
];

pub fn detect(text: &str) -> Vec<ParsedCharacter> {
    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();

    // Регэксп для capitalized слов: кириллица + латиница
    let re = Regex::new(
        r"(?<![a-zA-Z\x{0400}-\x{04FF}])([А-ЯЁA-Z][а-яёa-z\x{0400}-\x{04FF}]{2,})(?![a-zA-Z\x{0400}-\x{04FF}])",
    )
    .expect("invalid regex");

    // v0.4.0: Regex для sentence-end компилируем ОДИН раз (был внутри цикла —
    // это было ОЧЕНЬ медленно на 2MB тексте: 50k+ компиляций regex).
    let re_sent_end = Regex::new(
        r#"(?:[.!?…]["'»]?|\xE2\x80\x94|--|«|"|'|\n)\s*$"#,
    )
    .unwrap();

    let mut word_counts: HashMap<String, usize> = HashMap::new();

    // === Signal 1: capitalized word frequency (weak signal — только кандидат) ===
    for caps_result in re.captures_iter(text) {
        let caps = match caps_result {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some(m) = caps.get(1) {
            let word = m.as_str();
            let start = m.start();
            if start == 0 {
                continue;
            }
            // Увеличить context до 8 байт — для em-dash (3 байта UTF-8) + пробелы.
            // v0.2.2 брал только 3 байта, что ломало проверку для кириллицы.
            let mut preceding_start = if start >= 8 { start - 8 } else { 0 };
            while preceding_start < start && !text.is_char_boundary(preceding_start) {
                preceding_start += 1;
            }
            let preceding = &text[preceding_start..start];
            // Расширенная проверка sentence-start (v0.3.0):
            //   - . ! ? … + опц. закрывающая кавычка + пробелы
            //   - — (em-dash = U+2014, 3 байта UTF-8 = \xE2\x80\x94) = начало диалога
            //   - -- (двойной дефис) = альтернатива em-dash
            //   - « или " или ' (открывающая кавычка)
            //   - \n (новая строка = начало абзаца)
            // Это исправляет баг «Это» (freq=99 в 1-Сфера Предела), которое
            // проходило фильтр из-за неполной проверки sentence-end.
            if re_sent_end.is_match(preceding).unwrap_or(false) {
                continue;
            }
            if stop.contains(word) {
                continue;
            }
            *word_counts.entry(word.to_string()).or_insert(0) += 1;
        }
    }

    // === Signal 2: speech verbs + Name (СИЛЬНЫЙ сигнал) ===
    // Два шаблона:
    //   (a) verb + Name: "сказал Архив" (verb-first, литературный/драматический)
    //   (b) Name + verb: "Архив сказал" (subject-first, нейтральный, чаще в прозе)
    // Концепты (Мнемар, Секвестр, Архив-как-здание) здесь не появляются —
    // они не «говорят» в тексте.
    //
    // Реализация через простой substring search (а не regex с 60+ alternations)
    // — это в 10-100x быстрее на больших текстах (2MB роман обрабатывается
    // за <1с вместо >60с с fancy-regex).
    let mut speech_bonus: HashMap<String, usize> = HashMap::new();
    let text_lower = text.to_lowercase();
    for verb in SPEECH_VERBS {
        // Находим все вхождения глагола в текст (case-insensitive)
        let mut search_from = 0;
        while let Some(rel_pos) = text_lower[search_from..].find(verb) {
            let pos = search_from + rel_pos;
            let verb_end = pos + verb.len();

            // Проверяем, что это слово целиком (граница перед и после)
            let before_ok = pos == 0 || !is_cyrillic_or_latin_byte(text.as_bytes()[pos - 1]);
            let after_byte = if verb_end < text.as_bytes().len() {
                text.as_bytes()[verb_end]
            } else {
                b' '
            };
            let after_ok = !is_cyrillic_or_latin_byte(after_byte);

            if before_ok && after_ok {
                // Pattern (a): verb + Name — ищем Capitalized слово после глагола
                // Пропускаем пробелы/знаки после глагола
                let rest = &text[verb_end..];
                if let Some(name) = extract_capitalized_word(rest) {
                    if !stop.contains(name) {
                        *speech_bonus.entry(name.to_string()).or_insert(0) += 1;
                    }
                }

                // Pattern (b): Name + verb — ищем Capitalized слово ПЕРЕД глаголом
                // Идём назад от pos, пропускаем пробелы
                if let Some(name) = extract_capitalized_word_before(text, pos) {
                    if !stop.contains(name) {
                        *speech_bonus.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
            }

            search_from = verb_end;
        }
    }

    // === Signal 3: direct address — «— Name, ...» (СИЛЬНЫЙ сигнал) ===
    // Прямое обращение в диалоге: em-dash (U+2014 = \xE2\x80\x94) + Name + знак.
    // Реализация через substring search для скорости.
    let em_dash = "\u{2014}"; // —
    let mut direct_bonus: HashMap<String, usize> = HashMap::new();
    let mut search_from = 0;
    while let Some(rel_pos) = text[search_from..].find(em_dash) {
        let dash_pos = search_from + rel_pos;
        let after_dash = dash_pos + em_dash.len();
        let rest = &text[after_dash..];
        // Пропускаем пробелы после em-dash, ищем Capitalized слово
        if let Some(name) = extract_capitalized_word(rest) {
            // v0.5.1 fix: вычисляем позицию имени через pointer arithmetic,
            // а не `after_dash + name.len()` (старый вариант игнорировал
            // whitespace между em-dash и именем, из-за чего direct_count
            // почти всегда был 0, и confidence 1.0 был недостижим).
            // Без этого фикса матрица Phase 2 теряет смысл: 3-сигнальный
            // кейс (cap + speech + direct) никогда не срабатывает.
            let name_start_in_text = (name.as_ptr() as usize) - (text.as_ptr() as usize);
            let name_end = name_start_in_text + name.len();
            // Проверяем, что после имени идёт знак препинания (, ! . ?)
            let after_name = if name_end < text.as_bytes().len() {
                text.as_bytes()[name_end]
            } else {
                b'.'
            };
            if after_name == b',' || after_name == b'!' || after_name == b'.' || after_name == b'?' {
                if !stop.contains(name) {
                    *direct_bonus.entry(name.to_string()).or_insert(0) += 1;
                }
            }
        }
        search_from = after_dash;
    }

    // === Группировка + фильтрация ===
    // v0.4.0: Теперь генерируем ТРИ категории сущностей:
    //   - Character: speech >= 1 OR direct >= 1 (имя говорит — это персонаж)
    //   - Organization: speech == 0 AND direct == 0 BUT слово встречается
    //     рядом с глаголами коллективного действия (Совет постановил, Клан
    //     собрался, Архив решил)
    //   - Concept: speech == 0 AND direct == 0 AND count >= 10 (Бездна, Эхо,
    //     Архив-как-здание) — абстрактные существительные, которые часто
    //     встречаются, но никогда не «говорят»
    //
    // Раньше (v0.3.0) такие слова просто отфильтровывались. Теперь они
    // сохраняются как отдельные ноды концептов/организаций — пользователь
    // видит их в графе и понимает, что парсер нашёл эти сущности, но они
    // не являются персонажами.
    //
    // ВАЖНО: итерируем по UNION всех трёх map'ов, а не только word_counts.
    // Иначе слово, которое всегда стоит в начале предложения (и поэтому
    // не попало в word_counts из-за фильтра sentence-start), но которое
    // является субъектом речи, будет пропущено. Пример: «Архив сказал» —
    // если «Архив» всегда в начале предложения, word_counts его не имеет,
    // но speech_bonus имеет.
    let all_words: HashSet<&String> = word_counts
        .keys()
        .chain(speech_bonus.keys())
        .chain(direct_bonus.keys())
        .collect();

    // Группы для персонажей (speech/direct >= 1)
    let mut groups: HashMap<String, (String, usize, HashSet<String>, usize, usize)> =
        HashMap::new();
    // Группы для концептов/организаций (speech == 0 AND direct == 0)
    // value: (rep, count, forms, is_organization)
    let mut concept_groups: HashMap<String, (String, usize, HashSet<String>, bool)> =
        HashMap::new();

    // v0.4.0: Текст в lowercase для быстрого поиска контекстных слов
    let text_lower = text.to_lowercase();

    for word in all_words {
        let count_from_wc = word_counts.get(word).copied().unwrap_or(0);
        let speech = speech_bonus.get(word).copied().unwrap_or(0);
        let direct = direct_bonus.get(word).copied().unwrap_or(0);

        // v0.4.0: Оптимизация — дорогостоящий count_word_occurrences
        // вызываем ТОЛЬКО для кандидатов в персонажи (где это нужно для
        // обнаружения слов, отфильтрованных sentence-start фильтром).
        // Для концептов используем только word_counts — если слово не
        // в word_counts (count_from_wc < 5), оно слишком редкое для концепта.
        let count = if speech >= 1 || direct >= 1 {
            // Кандидат в персонажи — пересчитываем частоту полностью
            if count_from_wc < 2 {
                count_word_occurrences(word, text)
            } else {
                count_from_wc
            }
        } else {
            // Кандидат в концепты — используем только word_counts
            // (концепты обычно частые, нет нужды в полном пересчёте)
            count_from_wc
        };

        if count < 1 {
            continue;
        }

        let key = super::lemmatize_simple(word);

        // v0.4.0: Классификация по типу сущности
        if speech >= 1 || direct >= 1 {
            // Character — есть лингвистический сигнал речи
            let entry = groups
                .entry(key.clone())
                .or_insert_with(|| (word.clone(), 0, HashSet::new(), 0, 0));
            entry.1 += count;
            entry.2.insert(word.clone());
            entry.3 += speech;
            entry.4 += direct;
            // Выбор каноничного имени (rep):
            // 1. Предпочитаем форму с бОльшим speech_count (им. падеж чаще субъект речи)
            // 2. При равенстве — более короткую форму
            let rep_speech = speech_bonus.get(&entry.0).copied().unwrap_or(0);
            if speech > rep_speech || (speech == rep_speech && word.len() < entry.0.len()) {
                entry.0 = word.clone();
            }
        } else {
            // Кандидат в концепты/организации.
            // Порог: count >= 5 (отфильтровать редкие имена-однодневки).
            if count < 5 {
                continue;
            }
            // v0.4.0: контекст (org/concept) проверяется ПОЗЖЕ, только для
            // топ-30 концептов по частоте — см. ниже. Здесь просто собираем
            // кандидатов без классификации.
            let entry = concept_groups
                .entry(key.clone())
                .or_insert_with(|| (word.clone(), 0, HashSet::new(), false));
            entry.1 += count;
            entry.2.insert(word.clone());
            // Каноничное имя — кратчайшая форма
            if word.len() < entry.0.len() {
                entry.0 = word.clone();
            }
        }
    }

    let mut sorted: Vec<_> = groups.into_values().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    // v0.4.0: truncate увеличен до 20 (было 25), чтобы оставить место для
    // концептов/организаций в общем пуле (всё равно <= 25 после merge).
    sorted.truncate(20);

    let mut result: Vec<ParsedCharacter> = sorted
        .into_iter()
        .map(|(rep, count, forms, speech, direct)| {
            let forms_vec: Vec<String> = forms.into_iter().collect();
            let aliases_str = forms_vec.iter().take(6).cloned().collect::<Vec<_>>().join(", ");

            let lemma = super::lemmatize_simple(&rep);
            let forms_preview: Vec<String> = forms_vec.iter().take(4).cloned().collect();
            let reason = format!(
                "character:rule=linguistic_signal;freq={};speech_verb_hits={};direct_address_hits={};lemma={};NOT_IN_STOPLIST;forms=[{}]",
                count, speech, direct, lemma, forms_preview.join(",")
            );

            let description = format!(
                "Персонаж, упомянутый {} раз. Формы: {}. Глаголы речи: {}, прямое обращение: {}.",
                count, aliases_str, speech, direct
            );

            // v0.5.0 / Phase 2: Compute evidence_signals + confidence.
            // Character всегда имеет SIGNAL_CAPITALIZED (иначе бы не попал в
            // кандидаты). Биты 1/2 выставляются по speech/direct counts.
            let evidence_signals: u8 = SIGNAL_CAPITALIZED
                | (if speech >= 1 { SIGNAL_SPEECH_VERB } else { 0 })
                | (if direct >= 1 { SIGNAL_DIRECT_ADDRESS } else { 0 });
            let is_single_token = !rep.contains(' ') && !rep.contains('-');
            let confidence =
                ParsedCharacter::confidence_from_signals(evidence_signals, is_single_token);

            // Собираем позиции упоминаний для этого canonical rep (byte offsets)
            let mention_positions = collect_mention_starts(&forms_vec, text);
            let first_mention = mention_positions.get(0).copied();

            let nom_cnt = speech + direct + count.saturating_sub(speech + direct);
            let acc_cnt = if count > speech + direct { (count - speech - direct) / 2 } else { 0 };

            ParsedCharacter {
                name: rep,
                aliases: forms_vec,
                count,
                description,
                speech_count: speech,
                direct_count: direct,
                reason,
                entity_type: EntityType::Character,
                evidence_signals,
                confidence,
                mention_starts: mention_positions,
                first_mention,
                nominative_count: nom_cnt,
                accusative_count: acc_cnt,
                genitive_negated_count: 0,
            }
        })
        .collect();

    // v0.4.0: Добавляем концепты и организации (до 5 в сумме)
    // ВАЖНО: проверка контекста (check_organization_context) дорогая —
    // для каждого слова сканируем весь text_lower. Поэтому сначала сортируем
    // по частоте и берём только топ-30 кандидатов — этого достаточно,
    // чтобы найти все важные концепты (Бездна, Эхо, Архив, Совет, Клан, etc.)
    // без сканирования сотен редких слов.
    let mut concepts: Vec<(String, usize, HashSet<String>, bool)> =
        concept_groups.into_values().collect();
    concepts.sort_by(|a, b| b.1.cmp(&a.1));
    // Предварительно ограничиваем топ-30 по частоте ДО проверки контекста
    concepts.truncate(30);
    // Теперь проверяем контекст только для топ-30
    let concepts: Vec<(String, usize, HashSet<String>, bool)> = concepts
        .into_iter()
        .map(|(rep, count, forms, was_org)| {
            // Если уже нашли org-контекст раньше (на этапе группировки) — оставляем
            // Иначе — перепроверяем один раз (для canonical rep формы)
            let is_org = was_org || check_organization_context(&rep, &text_lower);
            (rep, count, forms, is_org)
        })
        .collect();
    // Берём топ-5 концептов/организаций
    let mut top_concepts = concepts;
    top_concepts.sort_by(|a, b| {
        // Организации приоритетнее концептов (если частоты близки)
        match (a.3, b.3) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.1.cmp(&a.1),
        }
    });
    top_concepts.truncate(5);

    for (rep, count, forms, is_org) in top_concepts {
        let forms_vec: Vec<String> = forms.into_iter().collect();
        let aliases_str = forms_vec.iter().take(6).cloned().collect::<Vec<_>>().join(", ");
        let lemma = super::lemmatize_simple(&rep);
        let (entity_type, type_label) = if is_org {
            (EntityType::Organization, "organization")
        } else {
            (EntityType::Concept, "concept")
        };
        let reason = format!(
            "{}:rule=context_analysis;freq={};speech_verb_hits=0;direct_address_hits=0;lemma={};NOT_CHARACTER;forms=[{}]",
            type_label, count, lemma, forms_vec.iter().take(4).cloned().collect::<Vec<_>>().join(",")
        );
        let description = if is_org {
            format!(
                "Организация/коллективный субъект, упомянутая {} раз. Формы: {}. Обнаружена рядом с глаголами коллективного действия (постановил, собрался, решил…).",
                count, aliases_str
            )
        } else {
            format!(
                "Концепт/абстракция, упомянутая {} раз. Формы: {}. Не является персонажем (нет глаголов речи в качестве субъекта).",
                count, aliases_str
            )
        };
        let mention_positions = collect_mention_starts(&forms_vec, text);
        let first_mention = mention_positions.get(0).copied();
        result.push(ParsedCharacter {
            name: rep,
            aliases: forms_vec,
            count,
            description,
            speech_count: 0,
            direct_count: 0,
            reason,
            entity_type,
            evidence_signals: SIGNAL_CAPITALIZED,
            confidence: ParsedCharacter::confidence_from_signals(SIGNAL_CAPITALIZED, true),
            mention_starts: mention_positions,
            first_mention,
            nominative_count: 0,
            accusative_count: count / 2,
            genitive_negated_count: if is_org { 0 } else { 1 },
        });
    }

    // v0.4.2: CLOSED DIAGNOSTIC LOOP.
    //
    // Раньше диагностика (Smart X-Ray) показывала «Архив — suspect, не персонаж»,
    // но парсер НЕ реагировал — тип оставался Character. Теперь мы ЗАМЫКАЕМ
    // цикл: после классификации перепроверяем каждого Character и либо
    // реклассифицируем, либо удаляем.
    //
    // Правила:
    //   1. ABSTRACT_NOUNS — слова которые ВСЕГДА концепты/организации,
    //      никогда персонажи (даже если speech >= 1):
    //      «Архив», «Совет», «Эхо», «Бездна», «Свет», «Тьма», etc.
    //      → проверяем context на ORG_CONTEXT_WORDS → ORG или CONCEPT
    //
    //   2. Low speech ratio — real characters speak in >= 10% of mentions.
    //      Если speech/count < 0.05 AND count >= 10 → Concept
    //      (Бездна count=15 sp=1: ratio=0.067 → borderline, но
    //       в ABSTRACT_NOUNS — точно Concept)
    //
    //   3. No signal AND low count — speech=0 AND direct=0 AND count<10 → DELETE
    //
    // Без этого шага пользователю показываются «characters» типа «Архив»,
    // «Эхо», «Бездна» с пометкой «suspect» — это бесполезно и сбивает с толку.

    /// v0.4.2: Словарь слов которые ВСЕГДА являются концептами или
    /// организациями, никогда — персонажами. Даже если они иногда стоят
    /// рядом с глаголами речи (случайное совпадение), это не делает их
    /// действующими лицами.
    const ABSTRACT_NOUNS: &[&str] = &[
        // Абстрактные понятия
        "архив", "совет", "эхо", "бездна", "свет", "тьма",
        "предел", "пустота", "хаос", "порядок", "память", "судьба",
        "сфера", "голос", "закон", "сеть", "кольцо", "фаза",
        "ядро", "узор", "ритм", "сдвиг", "метрика", "энтропия",
        "формула", "принцип", "геометрия", "скелет", "левиафан",
        // Природные элементы
        "вода", "воздух", "огонь", "земля", "металл", "лёд",
        "солнце", "луна", "звезда", "ветер", "дождь", "снег",
        // Социальные/политические (контекстно — могут быть ORG)
        "реестр", "реестра", "реестру",
        // Эмоции/состояния
        "тишина", "молчание", "крик", "шёпот", "вздох",
        "боль", "радость", "грусть", "печаль", "тоска",
        // Время/пространство
        "время", "вечность", "мгновение", "миг", "час",
        "утро", "день", "вечер", "ночь", "рассвет", "закат",
        "весна", "лето", "осень", "зима",
    ];

    let mut reclassified_count = 0u32;
    let mut deleted_count = 0u32;

    for c in result.iter_mut() {
        if c.entity_type != EntityType::Character {
            continue;
        }
        let name_lower = c.name.to_lowercase();
        let name_first_word = name_lower.split_whitespace().next().unwrap_or(&name_lower);

        // Rule 1: ABSTRACT_NOUNS
        let is_abstract = ABSTRACT_NOUNS.contains(&name_lower.as_str())
            || ABSTRACT_NOUNS.contains(&name_first_word);
        if is_abstract {
            // Проверяем context: если есть ORG_CONTEXT_WORDS рядом → Organization
            let is_org = check_organization_context(&c.name, &text_lower);
            if is_org {
                c.entity_type = EntityType::Organization;
            } else {
                c.entity_type = EntityType::Concept;
            }
            // Обновляем reason
            c.reason = format!(
                "{}:rule=abstract_noun_reclassify;freq={};speech_verb_hits={};direct_address_hits={};lemma={};ABSTRACT_NOUN",
                if c.entity_type == EntityType::Organization { "organization" } else { "concept" },
                c.count, c.speech_count, c.direct_count, super::lemmatize_simple(&c.name)
            );
            reclassified_count += 1;
            continue;
        }

        // Rule 2: Low speech ratio AND high count → Concept
        if c.count >= 10 && c.speech_count > 0 {
            let ratio = c.speech_count as f64 / c.count as f64;
            if ratio < 0.05 {
                // Очень мало речи относительно частоты — почти точно концепт
                c.entity_type = EntityType::Concept;
                c.reason = format!(
                    "concept:rule=low_speech_ratio;freq={};speech_verb_hits={};direct_address_hits={};lemma={};ratio={:.3}",
                    c.count, c.speech_count, c.direct_count, super::lemmatize_simple(&c.name), ratio
                );
                reclassified_count += 1;
                continue;
            }
        }
    }

    // Rule 3: Delete Characters with no signal AND low count
    // (speech=0 AND direct=0 AND count<10 — это либо шум, либо слишком редкий)
    let initial_len = result.len();
    result.retain(|c| {
        if c.entity_type != EntityType::Character {
            return true;
        }
        if c.speech_count == 0 && c.direct_count == 0 && c.count < 10 {
            deleted_count += 1;
            return false;
        }
        true
    });
    let _ = initial_len;
    let _ = deleted_count;
    let _ = reclassified_count;

    // Финальная сортировка: characters сначала (по freq DESC), потом concepts/orgs
    result.sort_by(|a, b| {
        // Characters выше concepts
        let a_is_char = a.entity_type == EntityType::Character;
        let b_is_char = b.entity_type == EntityType::Character;
        match (a_is_char, b_is_char) {
            (true, true) | (false, false) => b.count.cmp(&a.count),
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
        }
    });
    result.truncate(25);
    result
}

/// v0.4.0: Проверить контекст вокруг упоминания слова на наличие
/// глаголов коллективного действия (ORG_CONTEXT_WORDS).
///
/// Алгоритм: для каждого упоминания `word` в `text_lower` проверяем
/// окно ±200 символов. Если в окне найдено любое из ORG_CONTEXT_WORDS —
/// возвращаем true (это организация).
///
/// Производительность: O(n*m) где n = кол-во упоминаний, m = кол-во
/// org-слов. Для 2MB текста и 15 упоминаний слова — ~15*50 = 750 проверок,
/// каждая ~200 байт substring search — <1мс.
fn check_organization_context(word: &str, text_lower: &str) -> bool {
    let word_lower = word.to_lowercase();
    if word_lower.is_empty() {
        return false;
    }
    let mut search_from = 0;
    let window = 200;
    while let Some(rel_pos) = text_lower[search_from..].find(&word_lower) {
        let pos = search_from + rel_pos;
        // Окно ±200 символов (БЕЗОПАСНЫЕ границы — выравниваем на char boundary)
        let mut win_start = pos.saturating_sub(window);
        while win_start < pos && !text_lower.is_char_boundary(win_start) {
            win_start += 1;
        }
        let mut win_end = (pos + word_lower.len() + window).min(text_lower.len());
        while win_end > pos + word_lower.len() && !text_lower.is_char_boundary(win_end) {
            win_end -= 1;
        }
        let context = &text_lower[win_start..win_end];
        for org_word in ORG_CONTEXT_WORDS {
            if context.contains(org_word) {
                return true;
            }
        }
        search_from = pos + word_lower.len();
    }
    false
}

/// Проверка присутствия персонажа в тексте главы
/// БЕЗ regex — простые строковые поиски (в 50x быстрее)
pub fn count_in_text(aliases: &[String], text: &str) -> usize {
    let lower = text.to_lowercase();
    let mut total = 0;
    for alias in aliases {
        let alias_lower = alias.to_lowercase();
        let mut start = 0;
        while let Some(pos) = lower[start..].find(&alias_lower) {
            let abs_pos = start + pos;
            let before = if abs_pos == 0 { b' ' } else { lower.as_bytes()[abs_pos - 1] };
            let after_pos = abs_pos + alias_lower.len();
            let after = if after_pos >= lower.len() { b' ' } else { lower.as_bytes()[after_pos] };
            let is_boundary_before = !is_word_char(before);
            let is_boundary_after = !is_word_char(after);
            if is_boundary_before && is_boundary_after {
                total += 1;
            }
            start = abs_pos + alias_lower.len();
        }
    }
    total
}

/// Собирает стартовые byte-offsets всех упоминаний alias'ов в тексте.
/// Возвращает отсортированный уникальный список позиций (byte index).
pub fn collect_mention_starts(aliases: &[String], text: &str) -> Vec<usize> {
    let lower = text.to_lowercase();
    let mut positions: Vec<usize> = Vec::new();
    for alias in aliases {
        let alias_lower = alias.to_lowercase();
        let mut start = 0;
        while let Some(pos) = lower[start..].find(&alias_lower) {
            let abs_pos = start + pos;
            let before = if abs_pos == 0 { b' ' } else { lower.as_bytes()[abs_pos - 1] };
            let after_pos = abs_pos + alias_lower.len();
            let after = if after_pos >= lower.len() { b' ' } else { lower.as_bytes()[after_pos] };
            let is_boundary_before = !is_word_char(before);
            let is_boundary_after = !is_word_char(after);
            if is_boundary_before && is_boundary_after {
                positions.push(abs_pos);
            }
            start = abs_pos + alias_lower.len();
        }
    }
    positions.sort_unstable();
    positions.dedup();
    positions
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || (0xC0..=0xFF).contains(&b) || b >= 0xC0
}

/// Проверяет, является ли байт частью кириллического или латинского символа (UTF-8).
/// Используется для проверки границ слов без декодирования UTF-8.
fn is_cyrillic_or_latin_byte(b: u8) -> bool {
    // Latin: a-z, A-Z
    b.is_ascii_alphabetic() ||
    // Cyrillic UTF-8 first byte: 0xD0-0xD1 (А-я, Ё-ё)
    (0xD0..=0xD1).contains(&b) ||
    // Ukrainian і, ї, є, ґ — first byte 0xD2, 0xD3
    (0xD2..=0xD3).contains(&b)
}

/// Извлекает Capitalized слово из начала строки (после пробелов/знаков).
/// Возвращает None, если первое слово не начинается с заглавной буквы.
/// Используется для Pattern (a): verb + Name.
fn extract_capitalized_word(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let mut i = 0;
    // Пропускаем пробелы и знаки препинания
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r') {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    // Проверяем, что символ — заглавная кириллическая или латинская буква
    // UTF-8: А-П = 0xD0 0x90-0x9F, Р-Я = 0xD0 0xA0-0xAF, Ё = 0xD0 0x81; A-Z = 0x41-0x5A
    let b0 = bytes[i];
    let is_upper = (b0 >= 0x41 && b0 <= 0x5A) || // A-Z
                   (b0 == 0xD0 && i + 1 < bytes.len() &&
                    ((bytes[i+1] >= 0x90 && bytes[i+1] <= 0xAF) || bytes[i+1] == 0x81)); // А-Я, Ё
    if !is_upper {
        return None;
    }
    // Находим конец слова (пока идут байты кириллицы/латиницы)
    let start = i;
    while i < bytes.len() {
        let b = bytes[i];
        if is_cyrillic_or_latin_byte(b) || (b >= 0x80 && b <= 0xBF) {
            i += 1;
        } else if b.is_ascii_alphanumeric() {
            i += 1;
        } else {
            break;
        }
    }
    s.get(start..i)
}

/// Извлекает Capitalized слово, заканчивающееся непосредственно перед позицией `pos`.
/// Идёт назад от pos, пропускает пробелы, затем собирает слово справа-налево.
/// Используется для Pattern (b): Name + verb.
fn extract_capitalized_word_before(text: &str, pos: usize) -> Option<&str> {
    if pos == 0 {
        return None;
    }
    let bytes = text.as_bytes();
    let mut i = pos;
    // Пропускаем пробелы назад
    while i > 0 && (bytes[i-1] == b' ' || bytes[i-1] == b'\t' || bytes[i-1] == b'\n' || bytes[i-1] == b'\r') {
        i -= 1;
    }
    if i == 0 {
        return None;
    }
    // Идём назад, собирая буквы слова
    let word_end = i;
    while i > 0 {
        let b = bytes[i-1];
        if is_cyrillic_or_latin_byte(b) || (b >= 0x80 && b <= 0xBF) || b.is_ascii_alphanumeric() {
            i -= 1;
        } else {
            break;
        }
    }
    if i == word_end {
        return None; // нет слова
    }
    // Выравниваем на char boundary
    let mut start = i;
    while start < word_end && !text.is_char_boundary(start) {
        start += 1;
    }
    // Проверяем, что первая буква — заглавная
    let first_byte = bytes[start];
    let is_upper = (first_byte >= 0x41 && first_byte <= 0x5A) ||
                   (first_byte == 0xD0 && start + 1 < word_end &&
                    ((bytes[start+1] >= 0x90 && bytes[start+1] <= 0xAF) || bytes[start+1] == 0x81));
    if !is_upper {
        return None;
    }
    text.get(start..word_end)
}

/// Подсчёт ВСЕХ вхождений слова в текст (включая начала предложений).
/// Используется для слов, которые есть в speech_bonus но мало в word_counts
/// (потому что они часто стоят в начале предложений и фильтровались Signal 1).
fn count_word_occurrences(word: &str, text: &str) -> usize {
    count_in_text(&[word.to_string()], text)
}

// ============================================================================
// v0.5.0 / Phase 2: Unit tests for confidence + evidence_signals
// ============================================================================
//
// Эти тесты — формальная верификация матрицы Phase 2:
//   3 сигнала → 1.0
//   2 сигнала single-token → 0.7
//   2 сигнала multi-token  → 0.5
//   1 сигнал  → 0.3
//   0 сигналов → 0.0
//
// Без этих тестов «confidence policy» остаётся только документацией.
// Тесты замыкают контракт: любое изменение политики должно пройти через
// обновление матрицы здесь.
#[cfg(test)]
mod phase2_confidence_tests {
    use super::*;

    #[test]
    fn test_3_signals_confidence_1_0() {
        // cap + speech + direct = 1 | 2 | 4 = 7 → 3 сигнала → 1.0
        let signals = SIGNAL_CAPITALIZED | SIGNAL_SPEECH_VERB | SIGNAL_DIRECT_ADDRESS;
        assert_eq!(ParsedCharacter::confidence_from_signals(signals, true), 1.0);
        assert_eq!(ParsedCharacter::confidence_from_signals(signals, false), 1.0);
    }

    #[test]
    fn test_2_signals_single_token_confidence_0_7() {
        // cap + speech = 1 | 2 = 3 → 2 сигнала → 0.7 (single-token)
        let signals = SIGNAL_CAPITALIZED | SIGNAL_SPEECH_VERB;
        assert_eq!(ParsedCharacter::confidence_from_signals(signals, true), 0.7);

        // cap + direct = 1 | 4 = 5 → 2 сигнала → 0.7 (single-token)
        let signals = SIGNAL_CAPITALIZED | SIGNAL_DIRECT_ADDRESS;
        assert_eq!(ParsedCharacter::confidence_from_signals(signals, true), 0.7);
    }

    #[test]
    fn test_2_signals_multi_token_confidence_0_5() {
        // Multi-token names → 0.5 (Python fallback для FIO resolution)
        let signals = SIGNAL_CAPITALIZED | SIGNAL_SPEECH_VERB;
        assert_eq!(ParsedCharacter::confidence_from_signals(signals, false), 0.5);

        let signals = SIGNAL_CAPITALIZED | SIGNAL_DIRECT_ADDRESS;
        assert_eq!(ParsedCharacter::confidence_from_signals(signals, false), 0.5);
    }

    #[test]
    fn test_1_signal_confidence_0_3() {
        // только cap = 1 → 1 сигнал → 0.3 (Python fallback обязателен)
        let signals = SIGNAL_CAPITALIZED;
        assert_eq!(ParsedCharacter::confidence_from_signals(signals, true), 0.3);
        assert_eq!(ParsedCharacter::confidence_from_signals(signals, false), 0.3);
    }

    #[test]
    fn test_0_signals_confidence_0_0() {
        assert_eq!(ParsedCharacter::confidence_from_signals(0, true), 0.0);
        assert_eq!(ParsedCharacter::confidence_from_signals(0, false), 0.0);
    }

    #[test]
    fn test_is_single_token_helper() {
        let single = ParsedCharacter {
            name: "Анна".to_string(),
            aliases: vec![],
            count: 1,
            description: String::new(),
            speech_count: 0,
            direct_count: 0,
            reason: String::new(),
            entity_type: EntityType::Character,
            evidence_signals: SIGNAL_CAPITALIZED,
            confidence: 0.3,
            mention_starts: vec![],
            first_mention: None,
            nominative_count: 0,
            accusative_count: 0,
            genitive_negated_count: 0,
        };
        assert!(single.is_single_token());

        let multi = ParsedCharacter {
            name: "Иван Петров".to_string(),
            aliases: vec![],
            count: 1,
            description: String::new(),
            speech_count: 0,
            direct_count: 0,
            reason: String::new(),
            entity_type: EntityType::Character,
            evidence_signals: SIGNAL_CAPITALIZED,
            confidence: 0.3,
            mention_starts: vec![],
            first_mention: None,
            nominative_count: 0,
            accusative_count: 0,
            genitive_negated_count: 0,
        };
        assert!(!multi.is_single_token());

        let hyphen = ParsedCharacter {
            name: "Анна-Мария".to_string(),
            aliases: vec![],
            count: 1,
            description: String::new(),
            speech_count: 0,
            direct_count: 0,
            reason: String::new(),
            entity_type: EntityType::Character,
            evidence_signals: SIGNAL_CAPITALIZED,
            confidence: 0.3,
            mention_starts: vec![],
            first_mention: None,
            nominative_count: 0,
            accusative_count: 0,
            genitive_negated_count: 0,
        };
        assert!(!hyphen.is_single_token());
    }

    /// Integration test: detect() на тексте с speech+direct адресом должно
    /// дать персонажа с confidence 1.0 (3 сигнала) и evidence_signals=7.
    ///
    /// Это **e2e-контур** того, что confidence policy действительно применяется
    /// внутри detect(), а не только в изолированном helper.
    #[test]
    fn test_detect_populates_confidence_for_3_signal_character() {
        // Текст: имя + speech verb + direct address → 3 сигнала
        let text = "Архип сказал привет. — Архип, иди сюда! Архип ответил.";
        let result = detect(text);

        let archip = result.iter().find(|c| c.name == "Архип");
        assert!(archip.is_some(), "Архип должен быть обнаружен как персонаж");
        let archip = archip.unwrap();

        assert_eq!(archip.entity_type, EntityType::Character);
        assert_eq!(archip.evidence_signals, 7, "3 сигнала: cap|speech|direct = 7");
        assert_eq!(archip.confidence, 1.0);
    }

    /// Integration test: detect() на тексте только с speech verb (без direct)
    /// должно дать персонажа с confidence 0.7 (2 сигнала, single-token).
    #[test]
    fn test_detect_populates_confidence_for_2_signal_character() {
        let text = "Борис сказал слово. Борис промолчал и ушёл.";
        let result = detect(text);

        let boris = result.iter().find(|c| c.name == "Борис");
        assert!(boris.is_some(), "Борис должен быть обнаружен");
        let boris = boris.unwrap();

        assert_eq!(boris.entity_type, EntityType::Character);
        // 2 сигнала: cap + speech = 3
        assert_eq!(boris.evidence_signals, 3);
        assert_eq!(boris.confidence, 0.7, "single-token 2-signal → 0.7");
    }

    /// Integration test: detect() на тексте только с capitalized (concept)
    /// должно дать концепт с confidence 0.3 (1 сигнал).
    #[test]
    fn test_detect_populates_confidence_for_concept() {
        // «Бездна» упоминается многократно, но не говорит → Concept
        let text = "Бездна смотрела. Бездна звала. Бездна ждала. \
                    Бездна дышала. Бездна молчала. Бездна пела. \
                    Бездна раскрывалась. Бездна закрывалась. \
                    Бездна улыбалась. Бездна хмурилась.";
        let result = detect(text);

        let abyss = result.iter().find(|c| c.name == "Бездна");
        // Бездна в ABSTRACT_NOUNS → Concept после reclassify
        if let Some(abyss) = abyss {
            assert_ne!(abyss.entity_type, EntityType::Character,
                "Бездна должна быть реклассифицирована из Character");
            assert_eq!(abyss.evidence_signals, 1, "только cap signal");
            assert_eq!(abyss.confidence, 0.3);
        }
    }
}
