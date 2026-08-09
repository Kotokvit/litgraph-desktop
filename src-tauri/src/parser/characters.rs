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
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
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

pub fn detect(text: &str) -> Vec<ParsedCharacter> {
    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();

    // Регэксп для capitalized слов: кириллица + латиница
    let re = Regex::new(
        r"(?<![a-zA-Z\x{0400}-\x{04FF}])([А-ЯЁA-Z][а-яёa-z\x{0400}-\x{04FF}]{2,})(?![a-zA-Z\x{0400}-\x{04FF}])",
    )
    .expect("invalid regex");

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
            let re_sent_end = Regex::new(
                r#"(?:[.!?…]["'»]?|\xE2\x80\x94|--|«|"|'|\n)\s*$"#,
            )
            .unwrap();
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
            // Проверяем, что после имени идёт знак препинания (, ! . ?)
            let name_end = after_dash + name.len();
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
    // Ключевое изменение v0.3.0: кандидат становится персонажем ТОЛЬКО
    // при наличии лингвистического сигнала:
    //   - speech >= 1 (имя употреблено с глаголом речи)
    //   - OR direct >= 1 (имя в прямом обращении)
    // Это убирает концепты даже при очень высокой частоте (Мнемар freq=80,
    // Секвестр freq=88, Архив-как-здание freq=329) — потому что они никогда
    // не выступают субъектами речи.
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

    let mut groups: HashMap<String, (String, usize, HashSet<String>, usize, usize)> =
        HashMap::new();
    for word in all_words {
        let count_from_wc = word_counts.get(word).copied().unwrap_or(0);
        let speech = speech_bonus.get(word).copied().unwrap_or(0);
        let direct = direct_bonus.get(word).copied().unwrap_or(0);

        // ФИЛЬТР: нужен лингвистический сигнал.
        // Без него — это концепт/топоним/абстракция, не персонаж.
        // Порог speech >= 1 (а не >= 2) потому, что даже одно употребление
        // с глаголом речи — сильный сигнал (концепты не говорят вообще).
        if speech < 1 && direct < 1 {
            continue;
        }

        // Если слово есть в speech_bonus но не в word_counts (или count < 2),
        // пересчитаем его частоту в полном тексте — включая начала предложений.
        let count = if count_from_wc < 2 && (speech >= 1 || direct >= 1) {
            count_word_occurrences(word, text)
        } else {
            count_from_wc
        };

        if count < 1 {
            continue;
        }

        // v0.3.1: Группировка по лемме (вместо 4-символьного префикса).
        // `lemmatize_simple` отсекает типичные русские/украинские окончания
        // (ами/я/у/ю/ы/и/е/ого/ему/...) и возвращает каноническую основу.
        // Это правильно объединяет:
        //   «Алексей» + «Алексея» + «Алексею» → lemma «алексе»
        //   «Марта» + «Марту» + «Мартой» → lemma «март»
        // И правильно НЕ объединяет разные имена с одинаковым 4-char prefix:
        //   «Алексей» + «Александр» + «Алексеев» (раньше все → «алек», теперь
        //   разные лемы: «алексе» / «александр» / «алексеев»).
        //
        // Ограничение: короткие имена (≤4 символов) возвращаются as-is.
        //   «Рэй» и «Рэя» НЕ сольются (нужен pymorphy3 — Варіант C).
        let key = super::lemmatize_simple(word);
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
    }

    let mut sorted: Vec<_> = groups.into_values().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(25);

    sorted
        .into_iter()
        .map(|(rep, count, forms, speech, direct)| {
            let forms_vec: Vec<String> = forms.into_iter().collect();
            let aliases_str = forms_vec.iter().take(6).cloned().collect::<Vec<_>>().join(", ");

            // === X-ray reason string ===
            // Полная трассировка решения парсера — для AI/developer review.
            // Формат вдохновлён оригинальным vision пользователя:
            //   "capitalized_word freq=8 prefix=Секв NOT in stoplist"
            // Теперь расширено до:
            //   "character:rule=linguistic_signal;freq=N;speech_verb_hits=N;
            //    direct_address_hits=N;lemma=XXXX;NOT_IN_STOPLIST;
            //    forms=[a,b,c,d]"
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

            ParsedCharacter {
                name: rep,
                aliases: forms_vec,
                count,
                description,
                speech_count: speech,
                direct_count: direct,
                reason,
            }
        })
        .collect()
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
