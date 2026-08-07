#!/usr/bin/env python3
"""
NER-извлечение для LitGraph.
Принимает текст через stdin, возвращает JSON с сущностями.

Использование:
    echo "Анна пошла в Москву" | python3 ner_extract.py

Выход (JSON):
{
  "entities": [
    {
      "lemma": "Анна",
      "label": "PER",
      "count": 3,
      "forms": ["Анна", "Анну", "Анной"],
      "mentions": [
        {"text": "Анна", "start": 0, "end": 4, "sentence": "Анна пошла в Москву"}
      ]
    }
  ],
  "stats": {"total": 2, "persons": 1, "locations": 1, "organizations": 0},
  "model": "ru_core_news_sm",
  "version": "0.1.0"
}
"""

import sys
import json
import re
from collections import defaultdict

try:
    import spacy
except ImportError:
    print(json.dumps({"error": "spaCy not installed. Run: pip install spacy && python -m spacy download ru_core_news_sm"}))
    sys.exit(1)

# pymorphy3 — для правильной лемматизации русских имён (ставится с ru_core_news_sm)
try:
    import pymorphy3
    MORPH = pymorphy3.MorphAnalyzer()
except ImportError:
    MORPH = None

# Загружаем модель один раз (медленно, но потом быстро)
try:
    NLP = spacy.load("ru_core_news_sm", disable=["lemmatizer"])
except OSError:
    # Пытаемся загрузить без disable
    try:
        NLP = spacy.load("ru_core_news_sm")
    except OSError:
        print(json.dumps({"error": "ru_core_news_sm model not found. Run: python -m spacy download ru_core_news_sm"}))
        sys.exit(1)

# Слова которые часто промечиваются как PROPN, но не являются именами
STOP_PROPN = {
    # Месяцы
    "январь", "февраль", "март", "апрель", "май", "июнь",
    "июль", "август", "сентябрь", "октябрь", "ноябрь", "декабрь",
    # Дни недели
    "понедельник", "вторник", "среда", "четверг", "пятница", "суббота", "воскресенье",
    # Праздники
    "новый", "год", "рождество", "пасха",
    # Стороны света
    "север", "юг", "восток", "запад",
    # Части суток
    "утро", "день", "вечер", "ночь",
    # Широкие категории которые иногда PROPN
    "бог", "господь", "господи",
}

def get_proper_lemma(text: str, spacy_lemma: str) -> str:
    """Получить каноническую форму (именительный падеж) через pymorphy3.
    Если pymorphy3 недоступен, не нашёл разбор с тегом Name/Surn, или лемма
    слишком отличается от оригинала — вернуть исходное слово."""
    if MORPH is None:
        return spacy_lemma or text
    try:
        parsed = MORPH.parse(text)
        # Ищем разбор с тегом Name (имя) или Surn (фамилия) — это гарантия что это имя
        for p in parsed:
            tags_str = str(p.tag)
            if "Name" in tags_str or "Surn" in tags_str:
                nf = p.normal_form
                if nf:
                    return nf[0].upper() + nf[1:]
        # Нет разбора как имя — возвращаем оригинал (нерусское имя, не склоняется)
        return text
    except Exception:
        return spacy_lemma or text


# Паттерн для определения "это точно имя человека"
# Гласная + согл + гласная в начале, длина 3-15, не содержит странных символов
NAME_PATTERN = re.compile(r"^[А-ЯЁ][а-яё]{2,14}$")


def is_likely_person(text: str, lemma: str) -> bool:
    """Дополнительная эвристика: похоже ли это на имя?"""
    if text.lower() in STOP_PROPN:
        return False
    if not NAME_PATTERN.match(text):
        return False
    return True


def sentence_text(doc, sent_start: int, sent_end: int) -> str:
    """Извлечь текст предложения по границам."""
    return doc.text[sent_start:sent_end].strip()


def extract_entities(text: str) -> dict:
    """Главная функция извлечения."""
    if not text or not text.strip():
        return {"entities": [], "stats": {"total": 0, "persons": 0, "locations": 0, "organizations": 0},
                "model": "ru_core_news_sm", "version": "0.1.0"}

    # Ограничиваем длину для скорости (spaCy ~50ms на 1000 токенов)
    max_chars = 100_000
    truncated = text[:max_chars]
    was_truncated = len(text) > max_chars

    doc = NLP(truncated)

    # Собираем сущности из spaCy NER
    entities_by_lemma = defaultdict(lambda: {
        "lemma": "",
        "label": "",
        "forms": set(),
        "count": 0,
        "mentions": [],
    })

    # 1. Сущности из spaCy.ents
    for ent in doc.ents:
        if not ent.text.strip():
            continue
        # Используем pymorphy3 для правильной лемматизации
        lemma_norm = get_proper_lemma(ent.text, ent.lemma_)
        key = (lemma_norm, ent.label_)
        e = entities_by_lemma[key]
        e["lemma"] = lemma_norm
        e["label"] = ent.label_
        e["forms"].add(ent.text)
        e["count"] += 1
        sent = ent.sent
        e["mentions"].append({
            "text": ent.text,
            "start": ent.start_char,
            "end": ent.end_char,
            "sentence": sent.text.strip()[:200],
        })

    # 2. Fallback: PROPN токены не вошедшие в ents (для русских имён)
    ent_token_ranges = set()
    for ent in doc.ents:
        for i in range(ent.start, ent.end):
            ent_token_ranges.add(i)

    for token in doc:
        if token.i in ent_token_ranges:
            continue
        if token.pos_ != "PROPN":
            continue
        if not is_likely_person(token.text, token.lemma_):
            continue
        lemma_norm = get_proper_lemma(token.text, token.lemma_)
        key = (lemma_norm, "PER")
        e = entities_by_lemma[key]
        e["lemma"] = lemma_norm
        e["label"] = "PER"
        e["forms"].add(token.text)
        e["count"] += 1
        sent = token.sent
        e["mentions"].append({
            "text": token.text,
            "start": token.idx,
            "end": token.idx + len(token.text),
            "sentence": sent.text.strip()[:200],
        })

    # 3. Группируем падежные формы одного имени
    # Если у нас "Анна" и "Анну" как отдельные сущности — это одно имя
    # spaCy должен давать один lemma, но на всякий случай проверим
    merged = {}
    for (lemma, label), data in entities_by_lemma.items():
        # Ключ группировки: lemma + label
        # Если уже есть такая же — объединяем
        if (lemma, label) in merged:
            existing = merged[(lemma, label)]
            existing["forms"].update(data["forms"])
            existing["count"] += data["count"]
            existing["mentions"].extend(data["mentions"])
        else:
            merged[(lemma, label)] = data

    # 4. Группировка падежных форм: "Анна" + "Анну" → "Анна"
    # Если общий префикс lemma ≥ 4 символов, объединяем
    def common_prefix_len(a: str, b: str) -> int:
        n = min(len(a), len(b))
        for i in range(n):
            if a[i].lower() != b[i].lower():
                return i
        return n

    final = {}
    items = list(merged.items())
    # Сортируем: короткие lemma сначала (они — кандидаты на "каноническую" форму)
    items.sort(key=lambda x: (len(x[0][0]), x[0][0]))
    used_keys = set()
    for i, (key_i, data_i) in enumerate(items):
        if key_i in used_keys:
            continue
        lemma_i, label_i = key_i
        canonical = data_i
        for j in range(i + 1, len(items)):
            key_j, data_j = items[j]
            if key_j in used_keys:
                continue
            lemma_j, label_j = key_j
            if label_i != label_j:
                continue
            # Общий префикс ≥ 4 символов
            cp = common_prefix_len(lemma_i, lemma_j)
            if cp >= 4:
                canonical["forms"].update(data_j["forms"])
                canonical["count"] += data_j["count"]
                canonical["mentions"].extend(data_j["mentions"])
                used_keys.add(key_j)
        final[key_i] = canonical

    # 5. Фильтруем: оставляем только сущности с count >= 1
    # Сортируем по убыванию count
    entities = []
    for data in final.values():
        entities.append({
            "lemma": data["lemma"],
            "label": data["label"],
            "count": data["count"],
            "forms": sorted(data["forms"])[:10],
            "firstMention": data["mentions"][0]["start"] if data["mentions"] else 0,
            "mentions": data["mentions"][:50],
        })
    entities.sort(key=lambda x: -x["count"])

    # 5. Статистика
    stats = {
        "total": len(entities),
        "persons": sum(1 for e in entities if e["label"] == "PER"),
        "locations": sum(1 for e in entities if e["label"] in ("LOC", "GPE")),
        "organizations": sum(1 for e in entities if e["label"] == "ORG"),
    }

    return {
        "entities": entities,
        "stats": stats,
        "model": "ru_core_news_sm",
        "version": "0.1.0",
        "truncated": was_truncated,
        "textLength": len(text),
        "processedLength": len(truncated),
    }


def main():
    try:
        text = sys.stdin.read()
        result = extract_entities(text)
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except Exception as e:
        print(json.dumps({"error": str(e), "type": type(e).__name__}, ensure_ascii=False))
        sys.exit(1)


if __name__ == "__main__":
    main()
