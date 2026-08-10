#!/usr/bin/env python3
"""
NER-извлечение v2 — на базе Natasha (Slovnet) + pymorphy3.

Это обёртка над scripts.dev.grammar.person, возвращающая JSON-формат,
совместимый со старым ner_extract.py (агрегированный: один персонаж → много упоминаний).

Преимущества над v1:
  - Natasha NER точнее, чем spaCy ru_core_news_sm
  - Нет огромного чёрного списка (~100 слов) — заменён на маленький список часто ошибочных
  - Корректно определяется пол через pymorphy3
  - Multi-token PER (ФИО) работают из коробки
  - Поддержка уменьшительных имён

Использование:
    python3 ner_extract_v2.py path/to/file.md
    echo "Анна пошла в Москву" | python3 ner_extract_v2.py

Выход (JSON):
{
  "entities": [
    {
      "lemma": "Алексей",
      "label": "PER",
      "count": 15,
      "forms": ["Алексей", "Алексея"],
      "firstMention": 609,
      "gender": "Masc",
      "mentions": [
        {"text": "Алексей", "start": 609, "end": 616, "sentence": "..."}
      ]
    }
  ],
  "stats": {"total": 5, "byLabel": {"PER": 4, "LOC": 1}}
}
"""
from __future__ import annotations

import sys
import os
import json
import re
from collections import defaultdict

# Добавляем корень проекта в path, чтобы можно было импортировать scripts.dev.*
# ner_extract_v2.py лежит в src-tauri/python/, корень = 3 уровня вверх
_HERE = os.path.dirname(os.path.abspath(__file__))
_PROJECT_ROOT = os.path.abspath(os.path.join(_HERE, '..', '..'))
if _PROJECT_ROOT not in sys.path:
    sys.path.insert(0, _PROJECT_ROOT)

from scripts.dev.grammar.person import extract_persons, PersonExtractor


# =============================================================================
# УТИЛИТЫ
# =============================================================================

def split_into_sentences(text: str) -> list[tuple[int, int, str]]:
    """Разбить текст на предложения с сохранением позиций.
    Возвращает [(start, end, sentence_text), ...]
    """
    sentences = []
    # Простой сплиттер: по .!? с сохранением позиции
    pattern = re.compile(r'[^.!?]*[.!?]+|\S[^.!?]*$', re.MULTILINE)
    for m in pattern.finditer(text):
        s = m.group().strip()
        if s:
            sentences.append((m.start(), m.start() + len(m.group()), s))
    return sentences


def find_sentence_for_position(pos: int, sentences: list[tuple[int, int, str]]) -> str:
    """Найти предложение, в которое попадает позиция."""
    for start, end, s in sentences:
        if start <= pos < end:
            return s
    return ''


# =============================================================================
# АГРЕГАЦИЯ
# =============================================================================

def aggregate_persons(persons: list[dict], text: str) -> list[dict]:
    """Агрегировать плоский список вхождений в формат как у ner_extract.py.

    Один персонаж = одна запись со списком mentions.
    """
    if not persons:
        return []

    sentences = split_into_sentences(text)

    # Группируем по лемме
    by_lemma = defaultdict(list)
    for p in persons:
        by_lemma[p['lemma']].append(p)

    entities = []
    for lemma, mentions_list in by_lemma.items():
        # Собираем все формы
        forms = sorted(set(m['text'] for m in mentions_list))

        # Сортируем mentions по позиции
        mentions_list.sort(key=lambda x: x['start'])

        # Формируем mentions с предложениями
        mentions = []
        for m in mentions_list:
            sent = find_sentence_for_position(m['start'], sentences)
            mentions.append({
                'text': m['text'],
                'start': m['start'],
                'end': m['end'],
                'sentence': sent,
            })

        # Пол из первого вхождения
        gender = mentions_list[0].get('gender', 'Unknown')

        entities.append({
            'lemma': lemma,
            'label': 'PER',
            'count': len(mentions_list),
            'forms': forms,
            'firstMention': mentions_list[0]['start'],
            'gender': gender,
            'first': mentions_list[0].get('first'),
            'last': mentions_list[0].get('last'),
            'middle': mentions_list[0].get('middle'),
            'mentions': mentions,
        })

    # Сортируем: сначала по count (убывание), потом по firstMention
    entities.sort(key=lambda x: (-x['count'], x['firstMention']))
    return entities


# =============================================================================
# ГЛАВНАЯ ФУНКЦИЯ
# =============================================================================

def extract_entities(text: str, min_freq: int = 2) -> dict:
    """Извлечь все сущности из текста.

    Args:
        text: исходный текст
        min_freq: минимальная частота (по умолчанию 2 — одиночные упоминания шум)

    Returns:
        dict с ключами 'entities' и 'stats'
    """
    # Извлекаем персонажей через Natasha
    persons = extract_persons(text, min_freq=min_freq)
    entities = aggregate_persons(persons, text)

    # Статистика
    stats = {
        'total': len(entities),
        'byLabel': {'PER': len(entities)},
        'totalMentions': sum(len(e['mentions']) for e in entities),
    }

    return {
        'entities': entities,
        'stats': stats,
    }


# =============================================================================
# CLI
# =============================================================================

def main():
    if len(sys.argv) > 1:
        path = sys.argv[1]
        if not os.path.exists(path):
            print(json.dumps({'error': f'File not found: {path}'}, ensure_ascii=False))
            sys.exit(1)
        with open(path, encoding='utf-8') as f:
            text = f.read()
    else:
        text = sys.stdin.read()

    result = extract_entities(text)
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
