# 04. Дієслова: вид, перехідність, спряження

> Канонічний довідник для розширення `POSITIVE_VERBS` / `NEGATIVE_VERBS` /
> `NEUTRAL_VERBS` у `src-tauri/src/reasoning/semantic_parser.rs:138-179`.
> Поточні списки — 130+ російських лем, без українських, без класифікації за
> видом, без спряжень.
>
> Джерело: [Wikipedia: Russian grammar](https://en.wikipedia.org/wiki/Russian_grammar),
> [Wikipedia: Ukrainian grammar](https://en.wikipedia.org/wiki/Ukrainian_grammar).
> Сирі тексти в [`raw/ru-grammar-wiki.txt`](raw/ru-grammar-wiki.txt),
> [`raw/uk-grammar-wiki.txt`](raw/uk-grammar-wiki.txt).

---

## 1. Вид дієслова (аспект)

### 1.1. Два види

| Вид | Російською | Українською | Приклад (рос.) | Приклад (укр.) |
|------|-----------|-------------|----------------|----------------|
| Недоконаний (imperfective) | что делать? | що робити? | убива́ть | вбивати |
| Доконаний (perfective) | что сделать? | що зробити? | уби́ть | вбити |

### 1.2. Значення для Reasoning Engine

**Критично для `KILL(a, p, t)`**: подія фіксується **тільки з доконаного виду**.

| Форма в тексті | Вид | Інтерпретація |
|----------------|-----|----------------|
| "Иван убил Петра" | доконаний | `KILL(Ivan, Peter, G12)` — факт убиття |
| "Иван убивал Петра" | недоконаний | повторювана спроба без фіксації результату |
| "Иван убивает Петра" | недоконаний | триваюча дія, без фіксації завершення |
| "Иван будет убивать Петра" | недоконаний | намір у майбутньому |
| "Иван убьёт Петра" | доконаний | факт у майбутньому — для прогнозу |

Зараз `verb_to_action()` не розрізняє вид. Це серйозний дефект: якщо текст
каже "убивал", код вважає це за `KILL` і додає факт смерті — помилково.

### 1.3. Пары дієслів (видові пари)

Доконай утворюються від недоконай 4 способами:

| Спосіб | Недоконаний | Доконаний | Приклад |
|--------|-------------|-----------|---------|
| Префікс | убива́ть | у-би́ть | убива́ть → уби́ть |
| Суфікс -ну- | стуча́ть | сту́к-ну-ть | стуча́ть → сту́кнуть |
| Чергування | забира́ть | забра́ть (а↔∅) | забира́ть → забра́ть |
| Суплетивізм | говори́ть | сказа́ть | говори́ть → сказа́ть |

Для LitGraph: потрібен **словник видових пар**. Якщо зустрічається недоконаний,
шукаємо пару; якщо є — перевіряємо, чи не зустрічався доконаний раніше. Якщо
зустрічався — недоконаний вважаємо "фоновой дією" без створення нової події.

---

## 2. Перехідність

| Тип | Питання | Приклад | Об'єкт |
|------|---------|---------|--------|
| Перехідний (transitive) | діє над ким/чим? | "Иван убил Петра" | потрібен прямий об'єкт |
| Непереходний (intransitive) | — | "Иван умер" | об'єкта немає |

**Для `KILL(a, p, t)`** перехідний: обов'язковий об'єкт `p`.
**Для `DIE(e, t)`** непереходний: об'єкт відсутній.

Зараз `verb_to_action()` не розрізняє перехідність. Якщо "Иван умер" — код
спробує знайти об'єкт у `object_lemma`, не знайде, залишить `None`. Це працює
випадково, але не формально.

---

## 3. Спряження

### 3.1. Російське I і II спряження

| Спряження | Закінчення (однина) | Закінчення (множина) | Приклад |
|-----------|---------------------|----------------------|---------|
| I | -у, -ешь, -ет, -ем, -ете | -ут, -ут | говори́ть → говорю́, говори́шь |
| II | -у, -ишь, -ит, -им, -ите, -ат/ят | -ат, -ят | ви́деть → ви́жу, ви́дишь |

**Винятки** (II спряження, але на -еть): гнать, держать, смотреть, видеть,
слышать, ненавидеть, обидеть, терпеть, зависеть, вертеть.

**Винятки** (I спряження, але на -ать): брить, стелить, зиждиться, зыбиться.

### 3.2. Для LitGraph

Спряження **не потрібне** для семантичного аналізу — воно визначає лише форми
дієвідмінювання. Але якщо ми хочемо розпізнавати "убьёт" (майбутнє доконане)
як окрему форму, треба знати, що це форма від "убить".

Для цього достатньо лематизації: "убьёт" → "убить". Зараз код покладається на
Python SVO parser для лематизації. Без нього `verb_to_action("убьёт", ...)`
не спрацює (не знайде в таблиці).

---

## 4. Розширена класифікація для `Action`

### 4.1. Поточні 3 класи (рос.)

```rust
const POSITIVE_VERBS: &[&str] = &["любить", "помочь", ...];  // 35
const NEGATIVE_VERBS: &[&str] = &["убить", "ударить", ...];  // 50
const NEUTRAL_VERBS:  &[&str] = &["пойти", "стоять", ...];   // 45
```

### 4.2. Пропоновані 8 класів з підкласами

```rust
pub enum VerbClass {
    Violence,         // убить, ударить, ранить
    ViolencePsych,    // обидеть, оскорбить, угрозать
    Crime,            // украсть, обмануть, предать
    Care,             // помочь, спасти, защитить
    Affection,        // любить, обнять, поцеловать
    Movement,         // пойти, прийти, уехать
    Communication,    // сказать, ответить, спросить
    Cognition,        // думать, знать, забыть
    Perception,       // видеть, слышать, заметить
    State,            // стоять, сидеть, лежать
    Possession,       // дать, взять, получить
    Creation,         // сделать, написать, нарисовать
    Other,
}
```

Для кожного класу — окремий набір правил:
- `Violence` → `Action::Kill` / `Action::Hit` / `Action::Wound`
- `Crime` → `Action::Steal` / `Action::Betray` / `Action::Lie`
- `Care` → `Action::Help` / `Action::Save`
- `Affection` → `Action::FallInLove` / `Action::Hug` / `Action::Kiss`
- `Movement` → `Action::Move` / `Action::Arrive` / `Action::Leave`
- `Communication` → `Action::Speak` (з можливим діалогом)
- `Cognition` → `Action::Know` / `Action::Forget` / `Action::Think`
- `Perception` → `Action::See` / `Action::Hear`

### 4.3. Додати українські еквіваленти

| Російська | Українська | Клас |
|-----------|------------|------|
| убить | вбити | Violence |
| ударить | ударити | Violence |
| обидеть | образити | ViolencePsych |
| украсть | вкрасти | Crime |
| обмануть | обдурити | Crime |
| предать | зрадити | Crime |
| помочь | допомогти | Care |
| спасти | врятувати | Care |
| любить | любити | Affection |
| обнять | обійняти | Affection |
| пойти | піти | Movement |
| прийти | прийти | Movement |
| уехать | поїхати | Movement |
| сказать | сказати | Communication |
| ответить | відповісти | Communication |
| спросить | спитати | Communication |
| думать | думати | Cognition |
| знать | знати | Cognition |
| забыть | забути | Cognition |
| видеть | бачити | Perception |
| слышать | чути | Perception |
| стоять | стояти | State |
| сидеть | сидіти | State |
| дать | дати | Possession |
| взять | узяти | Possession |
| сделать | зробити | Creation |
| написать | написати | Creation |

Це базовий словник 25 пар. Повний словник — 200-300 пар.

---

## 5. Зворотні дієслова

| Російська | Українська | Приклад контексту |
|-----------|------------|-------------------|
| убиться | вбити ся | "Иван убился" — самовбивство |
| удариться | ударитися | "Иван ударился" — травма |
| одеться | одягнутися | — |
| умыться | вмитися | — |

`-ся` / `-сь` змінює семантику: суб'єкт = об'єкт. Це треба окремо розпізнавати
в `verb_to_action()`.

---

## 6. Приклади розширеної таблиці

```rust
struct VerbEntry {
    lemma: &'static str,        // "убить"
    language: Language,          // Ru / Ua
    class: VerbClass,            // Violence
    aspect: Aspect,              // Perfective
    transitivity: Transitivity,  // Transitive
    action: Action,              // Action::Kill
    requires_object: bool,       // true (KILL має об'єкт)
    ru_pair: Option<&'static str>,   // Some("убивать")
    ua_pair: Option<&'static str>,   // Some("вбивати")
}

const VERB_DICTIONARY: &[VerbEntry] = &[
    VerbEntry { lemma: "убить", language: Ru, class: Violence, aspect: Perfective, transitivity: Transitive, action: Action::Kill, requires_object: true, ru_pair: Some("убивать"), ua_pair: Some("вбити") },
    VerbEntry { lemma: "вбити", language: Ua, class: Violence, aspect: Perfective, transitivity: Transitive, action: Action::Kill, requires_object: true, ru_pair: Some("убить"), ua_pair: Some("вбивати") },
    VerbEntry { lemma: "убивать", language: Ru, class: Violence, aspect: Imperfective, transitivity: Transitive, action: Action::KillAttempt, requires_object: true, ru_pair: Some("убить"), ua_pair: Some("вбивати") },
    VerbEntry { lemma: "вбивати", language: Ua, class: Violence, aspect: Imperfective, transitivity: Transitive, action: Action::KillAttempt, requires_object: true, ru_pair: Some("убить"), ua_pair: Some("вбити") },
    // ... 200+ entries
];
```

---

## 7. Тести

```rust
#[test]
fn test_aspect_distinction_kill() {
    // Доконаний → KILL
    let a1 = verb_to_action_v2("убить", "+", false, Ru);
    assert_eq!(a1.action, Action::Kill);

    // Недоконаний → KILL_ATTEMPT (не фіксує смерть)
    let a2 = verb_to_action_v2("убивать", "+", false, Ru);
    assert_eq!(a2.action, Action::KillAttempt);
}

#[test]
fn test_ukrainian_verb_lookup() {
    let a = verb_to_action_v2("вбити", "+", false, Ua);
    assert_eq!(a.action, Action::Kill);
}

#[test]
fn test_reflexive_suicide() {
    let a = verb_to_action_v2("убиться", "+", false, Ru);
    assert_eq!(a.action, Action::Suicide);
    // Subj = Obj
}

#[test]
fn test_verb_pair_lookup() {
    let e = find_verb_entry("убивать", Ru).unwrap();
    assert_eq!(e.ru_pair, Some("убить"));
    assert_eq!(e.aspect, Aspect::Imperfective);
}
```

---

## 8. Посилання

- Граматика (рос.): [`raw/ru-grammar-wiki.txt`](raw/ru-grammar-wiki.txt)
- Граматика (укр.): [`raw/uk-grammar-wiki.txt`](raw/uk-grammar-wiki.txt)
- SVO і синтаксис: [`05-syntax-svo.md`](05-syntax-svo.md)
