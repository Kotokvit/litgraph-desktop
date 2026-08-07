#!/usr/bin/env python3
"""
SVO-извлечение (Subject-Verb-Object) для LitGraph.

Находит в тексте триплеты: кто -> что сделал -> с кем/чем.
Использует dependency parsing spaCy + правила для русских падежей.

V2: интеграция с NER — субъект и объект должны быть известными персонажами.

Использование:
    python3 svo_extract.py input.txt

Выход (JSON):
{
  "triplets": [
    {
      "subject": "Раскольников",
      "verb": "ударил",
      "verbLemma": "ударить",
      "object": "Алёну",
      "objectLemma": "Алёна",
      "sentence": "Раскольников ударил Алёну топором.",
      "position": 123,
      "tense": "past",
      "polarity": "negative"   // positive/negative/neutral — тон действия
    }
  ],
  "stats": {
    "total": 15,
    "uniqueVerbs": 8,
    "uniqueSubjects": 3,
    "uniqueObjects": 5
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
    """Безопасная лемматизация имени собственного."""
    if MORPH is None:
        return text
    try:
        parsed = MORPH.parse(text)
        for p in parsed:
            tags_str = str(p.tag)
            if "Name" in tags_str or "Surn" in tags_str:
                nf = p.normal_form
                if nf:
                    return nf[0].upper() + nf[1:]
        return text
    except Exception:
        return text


def is_person_name(text: str, known_persons: set, fallback_to_propn: bool = True) -> bool:
    """Проверить, является ли токен именем известного персонажа.
    
    Если fallback_to_propn=True и known_persons пустой или текст не найден —
    принимаем любой PROPN-подобный токен (с заглавной буквы, кириллица).
    """
    if text in known_persons:
        return True
    lemma = get_proper_lemma_safe(text)
    if lemma in known_persons:
        return True
    if fallback_to_propn:
        # Fallback: похоже на имя (заглавная + кириллица, длина 3-15)
        if re.match(r"^[А-ЯЁ][а-яё]{2,14}$", text):
            # Но не из чёрного списка
            if text.lower() not in FALSE_POSITIVE_NOUNS:
                return True
    return False


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


def extract_svo_from_sentence(sent, known_persons: set) -> list:
    """Извлечь SVO-триплеты из одного предложения.
    
    Ищем структуру:
      [nsubj] -> [ROOT verb] -> [obj/iobj/obl]
    
    Где nsubj и obj должны быть именами известных персонажей.
    """
    triplets = []
    
    # Находим все глаголы (ROOT или conj)
    verbs = [t for t in sent if t.pos_ == "VERB" and t.dep_ in ("ROOT", "conj", "advcl")]
    
    for verb in verbs:
        # Ищем субъект (nsubj или nsubj:pass)
        subject = None
        for child in verb.children:
            if child.dep_ in ("nsubj", "nsubj:pass", "csubj"):
                if child.pos_ in ("PROPN", "NOUN") and is_person_name(child.text, known_persons):
                    subject = child
                    break
        
        if subject is None:
            continue
        
        # Ищем объекты (obj, iobj, obl)
        objects = []
        for child in verb.children:
            if child.dep_ in ("obj", "iobj", "obl", "obl:agent"):
                if child.pos_ in ("PROPN", "NOUN") and is_person_name(child.text, known_persons):
                    objects.append(child)
        
        if not objects:
            continue
        
        # Создаём триплет для каждого объекта
        verb_lemma = get_verb_lemma(verb)
        polarity = classify_verb_polarity(verb_lemma)
        tense = get_verb_tense(verb)
        
        for obj in objects:
            triplets.append({
                "subject": subject.text,
                "subjectLemma": get_proper_lemma_safe(subject.text),
                "verb": verb.text,
                "verbLemma": verb_lemma,
                "object": obj.text,
                "objectLemma": get_proper_lemma_safe(obj.text),
                "sentence": sent.text.strip()[:300],
                "position": subject.idx,
                "tense": tense,
                "polarity": polarity,
            })
    
    return triplets


def extract_svo(text: str, use_ner: bool = True) -> dict:
    """Главная функция извлечения SVO.
    
    Если use_ner=True — сначала извлекает персонажей через NER,
    потом ищет SVO только между ними (высокая точность).
    Если use_ner=False — ищет SVO между всеми PROPN токенами
    (быстрее, но больше шума).
    """
    if not text or not text.strip():
        return {"triplets": [], "stats": {"total": 0, "uniqueVerbs": 0, "uniqueSubjects": 0, "uniqueObjects": 0}}
    
    # 1. Если включён NER — извлекаем список персонажей
    known_persons = set()
    ner_result = None
    if use_ner and extract_entities is not None:
        ner_result = extract_entities(text)
        for ent in ner_result.get("entities", []):
            if ent["label"] == "PER":
                known_persons.add(ent["lemma"])
                for form in ent["forms"]:
                    known_persons.add(form)
    
    # 2. Разбиваем на чанки
    chunks = split_text_into_chunks(text, 50000)
    
    # 3. Извлекаем SVO из каждого чанка
    all_triplets = []
    for chunk in chunks:
        doc = NLP(chunk)
        for sent in doc.sents:
            triplets = extract_svo_from_sentence(sent, known_persons)
            all_triplets.extend(triplets)
    
    # 4. Статистика
    unique_verbs = set(t["verbLemma"] for t in all_triplets)
    unique_subjects = set(t["subjectLemma"] for t in all_triplets)
    unique_objects = set(t["objectLemma"] for t in all_triplets)
    
    # 5. Группировка по полярности
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
        },
        "nerResult": ner_result,
        "model": "ru_core_news_sm",
        "version": "0.1.0",
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
