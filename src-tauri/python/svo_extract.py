#!/usr/bin/env python3
"""
SVO-извлечение (Subject-Verb-Object) для LitGraph.

Находит в тексте триплеты: кто -> что сделал -> с кем/чем.
Использует dependency parsing spaCy + правила для русских падежей.

V2 (v0.2.0): пять фиксов против типичных потерь в русском тексте:
  1. Pronoun resolution — «его/её/их» (3-е лицо) резолвятся в
     последнего упомянутого PER совпадающего пола/числа.
     Применяется только для INTERACTION_VERBS (увидел, остановил,
     узнал, ударил...), чтобы не путать с анафорой на неодушевлённое.
  2. Pro-drop субъект — для VERB с dep=conj/advcl без nsubj-потомка
     субъект наследуется от головного глагола (Алексей подошёл,
     открыл и увидел Фёдора → у «увидел» субъект = Алексей).
  3. Multi-token PER span — «Владимир Петрович» собирается из
     head + flat:name children (с проверкой совпадения падежа,
     иначе «Алексей Сорокину» ошибочно склеится в одно имя).
  4. Subtree-search объектов — если spaCy attaches PER как nmod
     к obl-имени (увидел в коридоре Фёдора), мы всё равно находим
     его в поддереве глагола.
  5. Negation flag — «не/ни» как advmod-потомок глагола даёт
     поле `negated: true` в триплете (важно для J-матрицы).

Использование:
    python3 svo_extract.py input.txt

Выход (JSON):
{
  "triplets": [
    {
      "subject": "Раскольников",
      "subjectLemma": "Раскольников",
      "subjectGender": "Masc",
      "verb": "ударил",
      "verbLemma": "ударить",
      "object": "Алёну",
      "objectLemma": "Алёна",
      "objectGender": "Fem",
      "sentence": "Раскольников ударил Алёну топором.",
      "position": 123,
      "tense": "past",
      "polarity": "negative",
      "negated": false,
      "pronounResolved": false
    }
  ],
  "stats": {
    "total": 15,
    "uniqueVerbs": 8,
    "uniqueSubjects": 3,
    "uniqueObjects": 5,
    "byPolarity": {"positive": 4, "negative": 6, "neutral": 5},
    "knownPersons": 7
  }
}
"""

import sys
import os
import json
import re
from collections import defaultdict, Counter

# Добавляем свою директорию в path (для импорта ner_extract)
_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)

import spacy

# НЕ отключаем lemmatizer — нужен для глаголов (иначе verb.lemma_ пустой)
try:
    NLP = spacy.load("ru_core_news_sm")
except OSError:
    NLP = spacy.load("ru_core_news_sm", disable=["lemmatizer"])

try:
    import pymorphy3
    MORPH = pymorphy3.MorphAnalyzer()
except ImportError:
    MORPH = None

try:
    from ner_extract import extract_entities, get_proper_lemma, FALSE_POSITIVE_NOUNS
except ImportError:
    extract_entities = None
    get_proper_lemma = lambda text, lemma: text
    FALSE_POSITIVE_NOUNS = set()

# v0.2.0: module-level form→lemma map (заполняется в extract_svo)
# Нужен для нормализации падежных форм к канонической лемме:
# «Марину Игоревну» → «Марина Игоревна», «Владимир Петрович Сорокин» → «Владимир Петрович»
_FORM_TO_LEMMA = {}


# === Классификация глаголов по тональности ===
# Позитивные действия (созидание, помощь, любовь)
POSITIVE_VERBS = {
    "любить", "помочь", "помогать", "спасти", "спасать", "защитить", "защищать",
    "обнять", "обнимать", "поцеловать", "целовать", "подарить", "дарить",
    "утешить", "утешать", "простить", "прощать", "поздравить", "похвалить",
    "наградить", "благословить", "вылечить", "лечить", "кормить", "накормить",
    "одеть", "успокоить", "радовать", "обрадовать", "восхищать", "восхитить",
    "пригласить", "встретить", "проводить", "навестить", "навещать",
    "согласиться", "поддержать", "верить", "доверять", "посочувствовать",
    "сочувствовать", "выслушать", "послушать", "ответить", "сказать", "молвить",
}

# Негативные действия (разрушение, насилие, ложь)
NEGATIVE_VERBS = {
    "убить", "убивать", "ударить", "бить", "избить", "ранить", "ранять",
    "обидеть", "обижать", "оскорбить", "оскорблять", "предать", "предавать",
    "обмануть", "обманывать", "солгать", "лгать", "украсть", "красть",
    "разрушить", "разрушать", "сжечь", "поджечь", "отнять", "отнимать",
    "выгнать", "гонять", "прогнать", "наказать", "казнить", "пытать",
    "ненавидеть", "презирать", "проклясть", "проклинать", "угрожать",
    "напасть", "атаковать", "воевать", "бороться", "запретить", "запрещать",
    "запереть", "запирать", "арестовать", "судить", "осуждать", "осудить",
    "оттолкнуть", "толкать", "ударить", "плакать", "страдать",
    "изменить", "изменять", "соблазнить", "соблазнять", "подкупить",
    "шантажировать", "давить", "подозревать", "обвинить", "обвинять",
}

# Нейтральные глаголы (движение, разговор без оценки)
NEUTRAL_VERBS = {
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
}


def classify_verb_polarity(verb_lemma: str) -> str:
    """Классифицировать глагол: positive/negative/neutral."""
    v = verb_lemma.lower()
    if v in POSITIVE_VERBS:
        return "positive"
    if v in NEGATIVE_VERBS:
        return "negative"
    return "neutral"


def get_verb_lemma(token) -> str:
    """Получить лемму глагола. Сначала spaCy, потом pymorphy3 fallback."""
    if token.lemma_:
        return token.lemma_.lower()
    if MORPH is not None:
        try:
            p = MORPH.parse(token.text)
            if p:
                nf = p[0].normal_form
                if nf:
                    return nf.lower()
        except Exception:
            pass
    return token.text.lower()


def get_verb_tense(token) -> str:
    """Определить время глагола."""
    morph = token.morph
    tense = morph.get("Tense")
    if tense:
        return str(tense[0]).lower()
    return "unknown"


def get_proper_lemma_safe(text: str) -> str:
    """Безопасная лемматизация имени собственного.
    
    v0.2.0: предпочитать Name-разбор Surn-разбору.
    Иначе «Марину» (acc.) лемматизируется как «Марин» (Surn)
    вместо «Марина» (Name).
    """
    if MORPH is None:
        return text
    try:
        parsed = MORPH.parse(text)
        # Первый проход: ищем Name-разбор
        for p in parsed:
            tags_str = str(p.tag)
            if "Name" in tags_str:
                nf = p.normal_form
                if nf:
                    return nf[0].upper() + nf[1:]
        # Второй проход: Surn-разбор
        for p in parsed:
            tags_str = str(p.tag)
            if "Surn" in tags_str:
                nf = p.normal_form
                if nf:
                    return nf[0].upper() + nf[1:]
        return text
    except Exception:
        return text


def is_person_name(text: str, known_persons: set, fallback_to_propn: bool = True) -> bool:
    """Проверить, является ли токен именем известного персонажа.
    
    Если fallback_to_propn=True и known_persons пустой или текст не найден —
    принимаем любой PROPN-подобный токен (с заглавной буквы, кириллица),
    НО с обязательной проверкой pymorphy3 на тег Name/Surn.
    Это отсеивает нарицательные существительные в начале предложения
    ("Перекрёсток", "Прозвище", "Угроза", "Объяснение", "Капли").
    """
    if text in known_persons:
        return True
    lemma = get_proper_lemma_safe(text)
    if lemma in known_persons:
        return True
    if fallback_to_propn:
        # Fallback: похоже на имя (заглавная + кириллица, длина 3-15)
        if re.match(r"^[А-ЯЁ][а-яё]{2,14}$", text):
            if text.lower() not in FALSE_POSITIVE_NOUNS:
                # v0.2.0: обязательная проверка pymorphy3 на Name/Surn тег
                # (фильтрует нарицательные, начинающиеся с большой буквы)
                if MORPH is not None and _has_name_or_surn_tag(text):
                    return True
    return False


def _has_name_or_surn_tag(text: str) -> bool:
    """Проверить, есть ли в pymorphy3-разборах текста тег Name или Surn."""
    try:
        for p in MORPH.parse(text):
            tags_str = str(p.tag)
            if "Name" in tags_str or "Surn" in tags_str:
                return True
    except Exception:
        pass
    return False


# === Глаголы, подразумевающие PER→PER взаимодействие ===
# Только для них активируется pronoun-resolution: это защищает от ложных
# срабатываний вида "открыл её (дверь)" → "её" резолвится в персонажа.
INTERACTION_VERBS = {
    # физическое взаимодействие
    "увидеть", "услышать", "заметить", "найти", "разыскать", "встретить",
    "проводить", "навестить", "наблюдать", "следить",
    # насилие / принуждение
    "остановить", "ударить", "бить", "избить", "ранить", "убить", "поймать",
    "выгнать", "прогнать", "оттолкнуть", "толкнуть", "схватить", "запереть",
    "арестовать", "казнить", "пытать", "наказать",
    # речь / коммуникация
    "ответить", "спросить", "сказать", "позвать", "позвонить", "написать",
    "сообщить", "объяснить", "предупредить", "пообещать", "приказать",
    # эмоциональное / социальное
    "узнать", "обнять", "поцеловать", "простить", "обвинить", "осудить",
    "оправдать", "подозревать", "ненавидеть", "презирать", "любить",
    "предать", "обмануть", "солгать", "угрожать", "напасть", "защитить",
    "спасти", "помочь", "поддержать", "наградить", "поздравить",
    "похвалить", "благословить", "проклясть",
}


def split_text_into_chunks(text: str, chunk_size: int = 50000) -> list:
    """Разбить текст на части по границам предложений."""
    if len(text) <= chunk_size:
        return [text]
    chunks = []
    start = 0
    while start < len(text):
        end = start + chunk_size
        if end >= len(text):
            chunks.append(text[start:])
            break
        for i in range(end, max(end - 2000, start), -1):
            if i < len(text) and text[i - 1] in ".!?":
                chunks.append(text[start:i])
                start = i
                break
        else:
            chunks.append(text[start:end])
            start = end
    return chunks


def get_per_gender(text: str) -> str:
    """Определить пол имени: Masc/Fem/Neut/Unknown."""
    if MORPH is None:
        return "Unknown"
    try:
        for p in MORPH.parse(text):
            tags = str(p.tag).lower()
            if "masc" in tags:
                return "Masc"
            if "femn" in tags:
                return "Fem"
            if "neut" in tags:
                return "Neut"
        return "Unknown"
    except Exception:
        return "Unknown"


def build_person_genders(known_persons: set) -> dict:
    """Для каждого known person lemma вычислить пол (по первому токену)."""
    g = {}
    for p in known_persons:
        first = p.split()[0] if p else p
        g[p] = get_per_gender(first)
    return g


def reconstruct_multi_token_per(token, known_persons: set) -> str:
    """Собрать multi-token PER span: head + flat:name children.
    
    Проверка падежа: если у flat:name-потомка ДРУГОЙ падеж, чем у head,
    это синтаксически отдельная сущность (напр. «Алексей Сорокину» =
    субъект + iobj) — не склеиваем.
    
    v0.2.0: fallback — если spaCy не распознал flat:name, но следующий
    токен в документе — PROPN с тем же падежом, склеиваем (напр.,
    «Марину Игоревну» когда spaCy parses «Игоревну» как obj вместо flat:name).
    """
    span_text = token.text
    head_case = token.morph.get("Case")
    head_number = token.morph.get("Number")
    # 1. Walk flat:name children
    for sub in sorted(token.children, key=lambda t: t.i):
        if sub.dep_ in ("flat:name", "flat"):
            sub_case = sub.morph.get("Case")
            sub_number = sub.morph.get("Number")
            if (head_case and sub_case and
                    str(head_case[0]) != str(sub_case[0])):
                continue
            if (head_number and sub_number and
                    str(head_number[0]) != str(sub_number[0])):
                continue
            span_text += " " + sub.text
    # 2. Fallback: merge adjacent PROPN tokens with same case
    # (исправляет spaCy parse error, где «Игоревну» помечена как obj)
    doc = token.doc
    next_i = token.i + 1
    while next_i < len(doc):
        next_tok = doc[next_i]
        if next_tok.pos_ != "PROPN":
            break
        next_case = next_tok.morph.get("Case")
        next_number = next_tok.morph.get("Number")
        if (head_case and next_case and
                str(head_case[0]) == str(next_case[0])):
            if (not head_number or not next_number or
                    str(head_number[0]) == str(next_number[0])):
                # Проверим, что ещё не включили этот токен
                if next_tok.text not in span_text.split():
                    span_text += " " + next_tok.text
                    next_i += 1
                    continue
        break
    return span_text


def get_per_lemma_from_token(token, known_persons: set) -> str:
    """Получить лемму PER-токена: сначала пробуем multi-token span,
    потом single-token lemma, потом fallback на сам текст.
    
    v0.2.0: если single-token lemma не в known_persons, пробуем найти
    known_persons entry, начинающийся с этой леммы (напр., «Григорий»
    → «Григорий Палыч»). Это помогает фильтровать рефлексивные триплеты
    в разных падежах («Григорий Палыч» nom vs «Григорию Палычу» dat).
    v0.2.0: нормализация форм к леммам через _FORM_TO_LEMMA map
    (напр., «Марину Игоревну» → «Марина Игоревна»).
    """
    full_span = reconstruct_multi_token_per(token, known_persons)
    # v0.2.0: сначала проверяем form→lemma map (нормализация падежей)
    if full_span in _FORM_TO_LEMMA:
        return _FORM_TO_LEMMA[full_span]
    if full_span in known_persons:
        return full_span
    # Если span состоит из нескольких слов, но не в known_persons —
    # возможно, это фамилия+имя которые не были объединены NER.
    # Возьмём лемму первого токена.
    first = full_span.split()[0]
    first_lemma = get_proper_lemma_safe(first)
    if first_lemma in known_persons:
        return first_lemma
    if first in known_persons:
        return first
    # v0.2.0: prefix match — find known_persons entry starting with first_lemma
    for kp in known_persons:
        if kp.split()[0] == first_lemma:
            return kp
    return first_lemma if first_lemma else first


def has_negation(verb) -> bool:
    """Проверить, есть ли при глаголе отрицание «не»/«ни»."""
    for child in verb.children:
        if child.dep_ == "advmod" and child.text.lower() in ("не", "ни"):
            return True
    return False


def find_subject_with_inheritance(verb, known_persons: set,
                                   last_subject_token,
                                   recent_pers_by_gender: dict,
                                   gender_map: dict,
                                   last_subject_lemma: str = None,
                                   sent_pers: list = None):
    """Найти субъект глагола с поддержкой pro-drop наследования.
    
    Возвращает (token, lemma, gender) или (None, None, None).
    
    Стратегия:
      1. Прямой nsubj-потомок (PROPN/NOUN — PER).
         ЕСЛИ есть явный nsubj, но он НЕ PER — возвращаем None
         (у глагола есть субъект, но это не персонаж → не PER→PER).
      2. Если ничего нет и verb.dep in (conj, advcl) — наследуем
         субъекта от головного глагола (pro-drop в цепочке).
      3. last_subject_token из документного потока.
    
    v0.2.0: для PRON-субъекта 3-го лица предпочитаем last_subject_lemma
    (субъект предыдущего предложения).
    v0.2.0: position-based resolution через sent_pers.
    """
    # 1. Прямой nsubj
    explicit_nsubj_found = False
    for child in verb.children:
        if child.dep_ in ("nsubj", "nsubj:pass", "csubj"):
            explicit_nsubj_found = True
            if child.pos_ == "PRON":
                person = child.morph.get("Person")
                if person and str(person[0]) == "Third":
                    resolved = resolve_pronoun_to_per(
                        child, None, recent_pers_by_gender, gender_map,
                        last_subject_lemma=last_subject_lemma,
                        is_subject_pronoun=True,
                        sent_pers=sent_pers
                    )
                    if resolved:
                        return child, resolved, gender_map.get(resolved, "Unknown")
                continue
            if child.pos_ in ("PROPN", "NOUN") and is_person_name(child.text, known_persons):
                lemma = get_per_lemma_from_token(child, known_persons)
                gender = gender_map.get(lemma, get_per_gender(child.text))
                return child, lemma, gender
    
    # v0.2.0: если есть явный nsubj, но он не PER — не наследуем
    if explicit_nsubj_found:
        return None, None, None
    
    # 2. Наследование от головного глагола
    if verb.dep_ in ("conj", "advcl") and verb.head.pos_ == "VERB":
        parent_token, parent_lemma, parent_gender = find_subject_with_inheritance(
            verb.head, known_persons, None, recent_pers_by_gender, gender_map,
            last_subject_lemma=last_subject_lemma,
            sent_pers=sent_pers
        )
        if parent_token is not None:
            return parent_token, parent_lemma, parent_gender
    
    # 3. last_subject_token fallback
    if last_subject_token is not None:
        lemma = get_per_lemma_from_token(last_subject_token, known_persons)
        gender = gender_map.get(lemma, get_per_gender(last_subject_token.text))
        return last_subject_token, lemma, gender
    
    return None, None, None


def resolve_pronoun_to_per(pron_token, subject_lemma: str,
                            recent_pers_by_gender: dict,
                            gender_map: dict,
                            last_subject_lemma: str = None,
                            is_subject_pronoun: bool = False,
                            sent_pers: list = None) -> str:
    """Резолвить 3-е лицо местоимения в последнего упомянутого PER.
    
    Возвращает лемму персонажа или None.
    Исключает subject_lemma (нельзя резолвить «его» в самого субъекта).
    
    v0.2.0: position-based resolution.
    sent_pers = [(lemma, gender, idx), ...] — PER-ы текущего предложения.
    Сначала ищем в sent_pers (с idx < pronoun.idx), потом в recent_pers_by_gender.
    Это исправляет «Сорокин остановил его» → «его» = Фёдор
    (субъект того же предложения), а не «Владимир Петрович»
    (из предыдущего dialogue-тега).
    
    Если is_subject_pronoun=True и last_subject_lemma задан,
    предпочитаем его (субъект предыдущего предложения).
    """
    g = pron_token.morph.get("Gender")
    n = pron_token.morph.get("Number")
    gender = str(g[0]) if g else "Unknown"
    number = str(n[0]) if n else "Sing"
    pron_idx = pron_token.idx
    
    # Subject pronoun preference: prefer last_subject_lemma if gender matches
    if is_subject_pronoun and last_subject_lemma:
        last_subj_gender = gender_map.get(last_subject_lemma, "Unknown")
        if last_subj_gender == gender and last_subject_lemma != subject_lemma:
            return last_subject_lemma
    
    # 1. Same-sentence PERs (position-based, before pronoun)
    if sent_pers:
        if number == "Plur":
            same_sent = [(l, gd, i) for (l, gd, i) in sent_pers
                         if i < pron_idx and l != subject_lemma]
        else:
            same_sent = [(l, gd, i) for (l, gd, i) in sent_pers
                         if i < pron_idx and gd == gender and l != subject_lemma]
        if same_sent:
            # Возвращаем самого недавнего (с наибольшим idx)
            return same_sent[-1][0]
    
    # 2. Cross-sentence cache
    if number == "Plur":
        all_recent = []
        for gend in ("Masc", "Fem"):
            all_recent.extend(recent_pers_by_gender.get(gend, []))
        seen = set()
        unique = []
        for x in all_recent:
            if x not in seen and x != subject_lemma:
                seen.add(x)
                unique.append(x)
        if unique:
            return unique[-1]
        return None
    
    candidates = recent_pers_by_gender.get(gender, [])
    for cand in reversed(candidates):
        if cand != subject_lemma:
            return cand
    return None


# Deps, которые НЕ должны рассматриваться как объекты.
# Напр., conj-PROPN — это parse error spaCy (субъект/дополнение,
# прикреплённое как conjunction к глаголу).
SKIP_DEPS_FOR_OBJECTS = {
    "punct", "parataxis", "conj", "acl", "advcl", "ccomp", "xcomp",
    "advmod", "aux", "cop", "mark", "cc", "det", "case", "discourse",
    "vocative", "csubj", "expl", "fixed", "goeswith", "reparandum",
}


def find_per_objects_in_subtree(verb, known_persons: set,
                                  subject_token,
                                  subject_lemma: str,
                                  recent_pers_by_gender: dict,
                                  gender_map: dict,
                                  interaction_mode: bool,
                                  sent_pers: list = None) -> list:
    """Найти все PER-объекты в поддереве глагола.
    
    Возвращает список словарей:
      {token, text, lemma, gender, is_pronoun, pronoun_resolved_to}
    
    Не заходит в поддеревья вложенных глаголов.
    Не пересекает parataxis-границу (диалог/прямая речь).
    v0.2.0: не принимает токены с dep из SKIP_DEPS_FOR_OBJECTS
    (напр., conj-PROPN — parse error, «Алексей» в диалоге).
    v0.2.0: дедупликация — токены, вошедшие в span предыдущего объекта,
    не рассматриваются повторно.
    Пропускает subject_token и его flat:name-потомков (с тем же падежом).
    """
    objects = []
    consumed_indices = set()  # токены, уже вошедшие в чей-то span
    
    # Соберём flat:name-потомков субъекта (часть его имени) — их не трогаем
    subject_flat_children = set()
    if subject_token is not None:
        head_case = subject_token.morph.get("Case")
        for ch in subject_token.children:
            if ch.dep_ in ("flat:name", "flat"):
                ch_case = ch.morph.get("Case")
                if (not head_case or not ch_case or
                        str(head_case[0]) == str(ch_case[0])):
                    subject_flat_children.add(ch)
        # Также отметим adjacent PROPN-ы субъекта как потреблённые
        subj_span = reconstruct_multi_token_per(subject_token, known_persons)
        # Найдём все токены, вошедшие в span субъекта
        cur_i = subject_token.i
        consumed_indices.add(cur_i)
        for word in subj_span.split()[1:]:  # пропускаем первое слово (это сам субъект)
            cur_i += 1
            if cur_i < len(subject_token.doc):
                consumed_indices.add(cur_i)
    
    def walk(token, depth=0):
        if depth > 0 and token.pos_ == "VERB":
            return
        for child in token.children:
            if child == subject_token or child in subject_flat_children:
                continue
            if child.dep_ == "punct":
                continue
            # v0.2.0: не пересекаем parataxis (диалог)
            if child.dep_ == "parataxis":
                continue
            # v0.2.0: пропускаем «мусорные» dep-ы (conj-PROPN, acl и т.д.)
            if child.dep_ in SKIP_DEPS_FOR_OBJECTS:
                continue
            # v0.2.0: дедупликация — токен уже в чьём-то span
            if child.i in consumed_indices:
                continue
            
            # PRON — 3-е лицо, пробуем резолвить
            if child.pos_ == "PRON" and interaction_mode:
                person = child.morph.get("Person")
                if person and str(person[0]) == "Third":
                    resolved = resolve_pronoun_to_per(
                        child, subject_lemma, recent_pers_by_gender, gender_map,
                        sent_pers=sent_pers
                    )
                    if resolved:
                        objects.append({
                            "token": child,
                            "text": child.text,
                            "lemma": resolved,
                            "gender": gender_map.get(resolved, "Unknown"),
                            "is_pronoun": True,
                            "pronoun_resolved_to": resolved,
                        })
                        consumed_indices.add(child.i)
                    continue
            
            # PROPN/NOUN — PER
            if child.pos_ in ("PROPN", "NOUN") and is_person_name(child.text, known_persons):
                # Особый случай: flat:name-потомок с ДРУГИМ падежом
                # → это отдельная сущность (напр. «Алексей Сорокину»)
                if child.dep_ in ("flat:name", "flat") and child.head != verb:
                    head_case = child.head.morph.get("Case")
                    child_case = child.morph.get("Case")
                    if (head_case and child_case and
                            str(head_case[0]) != str(child_case[0])):
                        lemma = get_per_lemma_from_token(child, known_persons)
                        gender = gender_map.get(lemma, get_per_gender(child.text))
                        objects.append({
                            "token": child,
                            "text": child.text,
                            "lemma": lemma,
                            "gender": gender,
                            "is_pronoun": False,
                            "pronoun_resolved_to": None,
                        })
                        consumed_indices.add(child.i)
                        continue
                    else:
                        continue
                
                # Обычный PER-объект
                span = reconstruct_multi_token_per(child, known_persons)
                lemma = get_per_lemma_from_token(child, known_persons)
                gender = gender_map.get(lemma, get_per_gender(child.text))
                objects.append({
                    "token": child,
                    "text": span,
                    "lemma": lemma,
                    "gender": gender,
                    "is_pronoun": False,
                    "pronoun_resolved_to": None,
                })
                # Отметим все токены span'а как потреблённые
                consumed_indices.add(child.i)
                cur_i = child.i + 1
                for _ in range(len(span.split()) - 1):
                    if cur_i < len(child.doc):
                        consumed_indices.add(cur_i)
                        cur_i += 1
                continue
            
            # Рекурсия в не-PER существительные и предлоги
            # (там может быть PER как nmod: «в коридоре Фёдора»)
            if child.pos_ in ("NOUN", "ADP") and not is_person_name(child.text, known_persons):
                walk(child, depth + 1)
    
    walk(verb)
    return objects


def extract_svo_from_sentence(sent, known_persons: set,
                                recent_pers_by_gender: dict,
                                gender_map: dict,
                                last_subject_token=None,
                                last_subject_lemma: str = None,
                                sent_pers: list = None) -> tuple:
    """Извлечь SVO-триплеты из одного предложения.
    
    Возвращает (triplets, last_subject_token, last_subject_lemma).
    
    sent_pers = [(lemma, gender, idx), ...] — PER-ы текущего предложения
    (для position-based pronoun resolution).
    """
    triplets = []
    current_last_subject = last_subject_token
    current_last_subject_lemma = last_subject_lemma
    
    verbs = [t for t in sent if t.pos_ == "VERB" and t.dep_ in ("ROOT", "conj", "advcl")]
    
    for verb in verbs:
        verb_lemma = get_verb_lemma(verb)
        is_interaction = verb_lemma in INTERACTION_VERBS
        polarity = classify_verb_polarity(verb_lemma)
        tense = get_verb_tense(verb)
        negated = has_negation(verb)
        
        # 1. Субъект
        subj_token, subj_lemma, subj_gender = find_subject_with_inheritance(
            verb, known_persons, current_last_subject,
            recent_pers_by_gender, gender_map,
            last_subject_lemma=current_last_subject_lemma,
            sent_pers=sent_pers
        )
        if subj_token is None or subj_lemma is None:
            continue
        
        # Обновляем last_subject для следующих глаголов
        if subj_token is not None and subj_token.pos_ in ("PROPN", "NOUN"):
            current_last_subject = subj_token
            current_last_subject_lemma = subj_lemma
        
        # 2. Объекты
        objects = find_per_objects_in_subtree(
            verb, known_persons, subj_token, subj_lemma,
            recent_pers_by_gender, gender_map, is_interaction,
            sent_pers=sent_pers
        )
        
        if not objects:
            continue
        
        subj_span = reconstruct_multi_token_per(subj_token, known_persons) \
            if subj_token.pos_ in ("PROPN", "NOUN") else subj_lemma
        
        for obj in objects:
            # v0.2.0: фильтр рефлексивных триплетов (subject == object)
            if subj_lemma == obj["lemma"]:
                continue
            triplets.append({
                "subject": subj_span if subj_token.pos_ in ("PROPN", "NOUN") else subj_token.text,
                "subjectLemma": subj_lemma,
                "subjectGender": subj_gender,
                "verb": verb.text,
                "verbLemma": verb_lemma,
                "object": obj["text"],
                "objectLemma": obj["lemma"],
                "objectGender": obj["gender"],
                "sentence": sent.text.strip()[:300],
                "position": subj_token.idx if subj_token else 0,
                "tense": tense,
                "polarity": polarity,
                "negated": negated,
                "pronounResolved": obj["is_pronoun"],
                "pronounResolvedTo": obj["pronoun_resolved_to"],
            })
    
    return triplets, current_last_subject, current_last_subject_lemma


def scan_pers_in_sentence(sent, known_persons: set, gender_map: dict) -> list:
    """Досканировать предложение: вернуть список PER-токенов в порядке
    появления (для обновления recent_pers_by_gender).
    
    Пропускает flat:name-потомков (они часть head'а).
    """
    pers = []
    for t in sent:
        if t.dep_ in ("flat:name", "flat"):
            continue
        if t.pos_ == "PRON":
            continue
        if t.pos_ in ("PROPN", "NOUN") and is_person_name(t.text, known_persons):
            lemma = get_per_lemma_from_token(t, known_persons)
            gender = gender_map.get(lemma, get_per_gender(t.text))
            pers.append((lemma, gender, t))
    return pers


def extract_svo(text: str, use_ner: bool = True) -> dict:
    """Главная функция извлечения SVO.
    
    Если use_ner=True — сначала извлекает персонажей через NER,
    потом ищет SVO только между ними (высокая точность).
    Если use_ner=False — ищет SVO между всеми PROPN токенами
    (быстрее, но больше шума).
    
    v0.2.0: стрипаем HTML-комментарии (<!-- ... -->) перед обработкой —
    они используются в тестовом корпусе для ожидаемых метрик и
    не должны учитываться как текст.
    """
    if not text or not text.strip():
        return {"triplets": [], "stats": {"total": 0, "uniqueVerbs": 0,
                "uniqueSubjects": 0, "uniqueObjects": 0,
                "byPolarity": {"positive": 0, "negative": 0, "neutral": 0},
                "knownPersons": 0}}
    
    # 0. Strip HTML-комментариев (как в ner_extract.py v0.2.1)
    text = re.sub(r"<!--.*?-->", "", text, flags=re.DOTALL)
    
    # 1. Если включён NER — извлекаем список персонажей
    known_persons = set()
    ner_result = None
    global _FORM_TO_LEMMA
    _FORM_TO_LEMMA = {}
    if use_ner and extract_entities is not None:
        ner_result = extract_entities(text)
        for ent in ner_result.get("entities", []):
            if ent["label"] == "PER":
                known_persons.add(ent["lemma"])
                # v0.2.0: заполняем form→lemma map
                _FORM_TO_LEMMA[ent["lemma"]] = ent["lemma"]
                for form in ent["forms"]:
                    known_persons.add(form)
                    _FORM_TO_LEMMA[form] = ent["lemma"]
    
    # 2. Предвычисляем пол каждого known person
    gender_map = build_person_genders(known_persons)
    
    # 3. Разбиваем на чанки
    chunks = split_text_into_chunks(text, 50000)
    
    # 4. Извлекаем SVO из каждого чанка
    all_triplets = []
    recent_pers_by_gender = {"Masc": [], "Fem": [], "Neut": []}
    last_subject_token = None
    last_subject_lemma = None
    
    for chunk in chunks:
        doc = NLP(chunk)
        for sent in doc.sents:
            # v0.2.0: pre-scan PERs in this sentence (for position-based resolution)
            # но НЕ обновляем cross-sentence cache до завершения обработки.
            sent_pers = scan_pers_in_sentence(sent, known_persons, gender_map)
            # Преобразуем в [(lemma, gender, idx), ...]
            sent_pers_tuples = [(l, g, t.idx) for (l, g, t) in sent_pers]
            
            triplets, last_subject_token, last_subject_lemma = extract_svo_from_sentence(
                sent, known_persons, recent_pers_by_gender, gender_map,
                last_subject_token, last_subject_lemma,
                sent_pers=sent_pers_tuples
            )
            all_triplets.extend(triplets)
            
            # После обработки — обновляем cross-sentence cache
            for lemma, gender, _ in sent_pers:
                if gender not in recent_pers_by_gender:
                    recent_pers_by_gender[gender] = []
                # Не дублируем подряд один и тот же персонаж
                if (not recent_pers_by_gender[gender] or
                        recent_pers_by_gender[gender][-1] != lemma):
                    recent_pers_by_gender[gender].append(lemma)
                # Ограничиваем кэш последними N
                if len(recent_pers_by_gender[gender]) > 20:
                    recent_pers_by_gender[gender] = recent_pers_by_gender[gender][-20:]
    
    # 5. Статистика
    unique_verbs = set(t["verbLemma"] for t in all_triplets)
    unique_subjects = set(t["subjectLemma"] for t in all_triplets)
    unique_objects = set(t["objectLemma"] for t in all_triplets)
    
    by_polarity = {
        "positive": sum(1 for t in all_triplets if t["polarity"] == "positive"),
        "negative": sum(1 for t in all_triplets if t["polarity"] == "negative"),
        "neutral": sum(1 for t in all_triplets if t["polarity"] == "neutral"),
    }
    
    return {
        "triplets": all_triplets,
        "stats": {
            "total": len(all_triplets),
            "uniqueVerbs": len(unique_verbs),
            "uniqueSubjects": len(unique_subjects),
            "uniqueObjects": len(unique_objects),
            "byPolarity": by_polarity,
            "knownPersons": len(known_persons),
            "pronounResolved": sum(1 for t in all_triplets if t.get("pronounResolved")),
            "negated": sum(1 for t in all_triplets if t.get("negated")),
        },
        "nerResult": ner_result,
        "model": "ru_core_news_sm",
        "version": "0.2.0",
    }


def main():
    try:
        # V2: читаем текст из файла (argv[1]) или stdin
        if len(sys.argv) > 1:
            with open(sys.argv[1], "r", encoding="utf-8") as f:
                text = f.read()
        else:
            text = sys.stdin.read()
        
        result = extract_svo(text, use_ner=True)
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except Exception as e:
        import traceback
        print(json.dumps({
            "error": str(e),
            "type": type(e).__name__,
            "traceback": traceback.format_exc(),
        }, ensure_ascii=False))
        sys.exit(1)


if __name__ == "__main__":
    main()
