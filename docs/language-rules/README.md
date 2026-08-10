# LitGraph — Довідник правил російської та української мов

> Ця папка містить еталонні правила морфології, синтаксису та лексики
> російської та української мов, зібрані з авторитетних джерел (Gramota.ru,
> Wikipedia, stopwords-iso, Ukrainian Language Institute). Призначена для
> розширення `src-tauri/src/reasoning/semantic_parser.rs` і
> `src-tauri/src/parser/` — поточні реалізують лише ~5% потрібних правил.
>
> Версія: 1.0.0 · Дата: 2026-08-10 · Статус: **reference**.

---

## 0. Чому ця папка існує

У поточному коді LitGraph правила мови розкидані по 5+ файлах у вигляді
хардкожених списків і regex'ів:

- `semantic_parser.rs:421` — `generate_russian_declensions(name)` з 6 правилами
  для чоловічих/жіночих імен. Покриття ~95% типових, але без родового множини,
  без присвійних, без по батькові.
- `semantic_parser.rs:138-179` — `POSITIVE_VERBS` / `NEGATIVE_VERBS` /
  `NEUTRAL_VERBS` зі 130+ російськими лемами. Без українських, без часток виду,
  без часток способу.
- `semantic_parser.rs:558` — `is_russian_stop_word(word)` з ~50 словами.
  Стандартний NLTK-список має 558 слів для російської, 1982 для української.
- `parser/mod.rs:493` — `lemmatize_simple(word)` з 4 спецвипадками.

**Мета цієї папки** — дати канонічну базу знань, у яку розробник (або
алгоритм) може підсунути повні парадигми замість підмножин.

---

## 1. Структура

| Файл | Призначення | Обсяг |
|------|-------------|-------|
| [`01-russian-declension.md`](01-russian-declension.md) | Російські відмінки (6), 3 типи відмінювання, анімативність, винятки | повна парадигма |
| [`02-ukrainian-declension.md`](02-ukrainian-declension.md) | Українські відмінки (7), 4 типи, м'яка/тверда/мішана підгрупи | повна парадигма |
| [`03-proper-names.md`](03-proper-names.md) | Імена власні: імена, по батькові, прізвища — обидві мови | повна парадигма |
| [`04-verbs-and-aspect.md`](04-verbs-and-aspect.md) | Дієслова: вид (доконаний/недоконаний), перехідність, спряження | класифікація |
| [`05-syntax-svo.md`](05-syntax-svo.md) | SVO, залежності, діалоги, пряма мова | синтаксис |
| [`stopwords-ru.txt`](stopwords-ru.txt) | 558 російських стоп-слів (NLTK-список) | список |
| [`stopwords-uk.txt`](stopwords-uk.txt) | 1982 українських стоп-слів (skupriienko/Ukrainian-Stopwords) | список |
| [`raw/`](raw/) | Сирі тексти з Wikipedia, Gramota.ru, slovnyk.ua — для цитування | 11 файлів, ~580KB |

---

## 2. Джерела

| Джерело | URL | Що взято |
|---------|-----|----------|
| Wikipedia: Russian declension | https://en.wikipedia.org/wiki/Russian_declension | Повні таблиці 3 відмінювань, анімативність |
| Wikipedia: Russian grammar | https://en.wikipedia.org/wiki/Russian_grammar | Огляд морфології, дієслів, синтаксису |
| Wikipedia: Ukrainian declension | https://en.wikipedia.org/wiki/Ukrainian_declension | 4 відмінювання, 7 відмінків |
| Wikipedia: Ukrainian grammar | https://en.wikipedia.org/wiki/Ukrainian_grammar | Огляд, фонетика, м'яка/тверда/мішана |
| Gramota.ru | https://gramota.ru/biblioteka/spravochniki/russkij-yazyk-kratkij-teoreticheskij-kurs-dlya-shkolnikov/sklonenie-sushchestvitelnykh | Російські відмінки (академічне джерело) |
| Wikipedia: Andrey Zaliznyak | https://en.wikipedia.org/wiki/Andrey_Zaliznyak | Інформація про канонічний словник Залізняка |
| Polyglottist Language Academy | https://www.polyglottistlanguageacademy.com/blog/basic-russian-sentence-structure-subject-verb-and-object | SVO огляд |
| stopwords-iso (GitHub) | https://github.com/stopwords-iso/stopwords-ru | 558 російських стоп-слів |
| skupriienko/Ukrainian-Stopwords | https://github.com/skupriienko/Ukrainian-Stopwords | 1982 українських стоп-слів |
| Slovnyk.ua (Новий правопис 2019) | https://slovnyk.ua | Українські імена, відмінювання |

---

## 3. Як це використовувати в коді

### 3.1. Канонічний план

Замість хардкодити правила в Rust-коді, перенести їх у **окремі модулі-словники**:

```rust
// Бажана структура після рефакторингу
mod language_rules {
    pub mod russian {
        pub mod declension;
        pub mod verbs;
        pub mod stopwords;
    }
    pub mod ukrainian {
        pub mod declension;
        pub mod verbs;
        pub mod stopwords;
    }
}
```

Кожен модуль завантажує свої дані з JSON-файлу (згенерованого з цієї папки) під
час init, і надає функції:

```rust
pub trait DeclensionRules {
    fn decline(&self, lemma: &str, gender: Gender) -> Vec<CaseForm>;
    fn detect_case(&self, word: &str, lemma: &str) -> Option<Case>;
}
```

### 3.2. Розширення `generate_russian_declensions`

Поточна функція (`semantic_parser.rs:421`) покриває:
- чоловічі на твердий приголосний (Грак, Ревун, Иван)
- чоловічі на -й (Алексей)
- чоловічі на -ь (Игорь)
- жіночі на -а (Марта)
- жіночі на -я (Катя)
- жіночі на -ия (Мария)
- винятки: Пётр, Лев

**Не покриває** (потрібно додати):
- чоловічі на -ей, -ай, -ой (Андрей, Николай, Дмитрий)
- чоловічі на -ий (Василий, Анатолий)
- родовий множини з біглою голосною (писар→писарів, але не завжди)
- по батькові (Іванович, Іванівна — повна парадигма)
- прізвища на -ов/-ев/-ин, -ский/-цкий, -ий (Иванов, Достоевский, Толстой)
- топоніми (Москва, Петербург, Київ, Львів)

### 3.3. Розширення стоп-слів

Поточний список `is_russian_stop_word()` має ~50 слів. Стандартний NLTK-список
має 558 (див. `stopwords-ru.txt`). Це в 11 разів більше покриття.

Особливо важливі додаткові категорії:
- прийменники (під, над, перед, між, через, близько, біля)
- питальні займенники (хто, що, який, чий, скільки)
- вказівні (той, ця, те, ті, цей, ця)
- означальні (весь, всякий, сам, самий, кожен, будь-який)
- форми дієслів "бути" (є, був, була, було, були, буде, будуть)
- частки (б, би, ж, же, мов, ніби, наче, хай, нехай)

### 3.4. Дієслова: додати українські та класифікацію

Поточні `POSITIVE_VERBS` / `NEGATIVE_VERBS` / `NEUTRAL_VERBS` — тільки російські.
Без українських LitGraph не зможе обробляти український текст на рівні російського.

Додатково: для кожної леми треба зберігати **вид** (доконаний / недоконаний),
бо `убить` (док.) ≠ `убивать` (недок.) у часовій логіці:

- `убил` (док.) → подія завершена, факт убиття зафіксовано.
- `убивал` (недок.) → повторювана дія в минулому, без фіксації результату.

Це важливо для Reasoning Engine: `KILL(Ivan, Peter, G12)` виводиться з "убил",
але НЕ з "убивал".

---

## 4. Ліцензія та цитування

- Wikipedia: CC BY-SA 3.0
- Gramota.ru: цитування з академічною метою дозволено
- stopwords-iso: MIT
- skupriienko/Ukrainian-Stopwords: MIT
- Slovnyk.ua: довідкове використання

При використанні в коді — посилання на джерело у коментарях обов'язкове.
