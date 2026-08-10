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

# Поддержка двух режимов запуска:
# 1. Dev mode: python3 ner_extract_v2.py — запущен из src-tauri/python/,
#    тогда scripts.dev.grammar.person доступен через project root.
# 2. Runtime mode: Rust run_python_with_text_file копирует скрипт в
#    /tmp/litgraph_scripts_*/main_script.py и кладёт person.py рядом.
#    Тогда scripts.dev.* недоступен — fallback на плоский import.
try:
    from scripts.dev.grammar.person import extract_persons, PersonExtractor
except ImportError:
    from person import extract_persons, PersonExtractor


# =============================================================================
# COMPATIBILITY SHIM (Phase 1B)
# =============================================================================
# poler_entities.py и svo_extract.py исторически импортируют из v1 ner_extract.py:
#   - NLP                   — spaCy-объект для doc.ents / token.pos_ / token.i / token.idx
#   - extract_entities      — функция, возвращающая JSON-контракт
#   - get_proper_lemma      — нормализация регистра леммы
#   - FALSE_POSITIVE_NOUNS  — frozenset часто-ошибочных нарицательных
#
# v2 экспортирует те же символы, но на базе Natasha + pymorphy3.
# Это позволяет полностью удалить v1 ner_extract.py без изменения потребителей.
# =============================================================================


class _EntSpanCompat:
    """spaCy-совместимая обёртка над Natasha Span.

    Natasha Span имеет только {start, stop, type} (символьные позиции).
    spaCy-потребители (poler_entities.py) ожидают:
      - ent.text       — текст спана
      - ent.label_     — 'PER' / 'LOC' / 'ORG'
      - ent.start_char — позиция начала (как Natasha.start)
      - ent.end_char   — позиция конца (как Natasha.stop)
      - ent.start      — token-индекс начала (для range(ent.start, ent.end))
      - ent.end        — token-индекс конца (exclusve)
    """

    __slots__ = ('text', 'label_', 'start_char', 'end_char', 'start', 'end')

    def __init__(self, text: str, label: str, start_char: int, end_char: int,
                 tok_start: int, tok_end: int):
        self.text = text
        self.label_ = label
        self.start_char = start_char
        self.end_char = end_char
        self.start = tok_start
        self.end = tok_end

    def __repr__(self) -> str:
        return f'Ent({self.text!r}, {self.label_}, char={self.start_char}:{self.end_char})'


class _TokenCompat:
    """spaCy-совместимая обёртка над Natasha Token.

    spaCy-потребители ожидают:
      - token.text   — текст
      - token.pos_   — POS-тег ('PROPN', 'NOUN', 'VERB', ...)
      - token.lemma_ — лемма
      - token.i      — индекс в документе
      - token.idx    — символьная позиция начала
    """

    __slots__ = ('text', 'pos_', 'lemma_', 'i', 'idx')

    def __init__(self, text: str, pos: str, lemma: str, idx: int, char_start: int):
        self.text = text
        self.pos_ = pos
        self.lemma_ = lemma
        self.i = idx
        self.idx = char_start

    def __repr__(self) -> str:
        return f'Token({self.text!r}, pos={self.pos_}, lemma={self.lemma_})'


class _DocCompat:
    """spaCy-совместимая обёртка над Natasha Doc.

    Поддерживает только те атрибуты, которые реально используют потребители:
      - doc.ents   — список _EntSpanCompat
      - doc.tokens — список _TokenCompat (для итерации `for token in doc`)
      - doc.sents  — генератор списков токенов по предложениям (для svo_extract)
    """

    def __init__(self, text: str, natasha_doc):
        self._text = text
        self._natasha = natasha_doc
        self.ents = []
        self._tokens = []
        self._build_compat()

    def _build_compat(self):
        # Tokens
        for i, t in enumerate(self._natasha.tokens):
            # Natasha POS уже использует Universal Dependencies ('PROPN', 'NOUN', ...)
            # что совпадает со spaCy Universal Dependencies scheme
            pos = t.pos or 'X'
            self._tokens.append(_TokenCompat(
                text=t.text,
                pos=pos,
                lemma=t.lemma or t.text,
                idx=i,
                char_start=t.start,
            ))
        # Entities — Natasha Span.start/stop это СИМВОЛЬНЫЕ позиции
        # Нужно найти token-индексы по символьным позициям
        for span in self._natasha.ner.spans:
            ent_text = self._text[span.start:span.stop]
            # Найти token-индексы: первый токен с t.start >= span.start,
            # последний с t.stop <= span.stop
            tok_start = None
            tok_end = None
            for i, t in enumerate(self._tokens):
                if t.idx >= span.start and tok_start is None:
                    # idx в _TokenCompat — это индекс, а не char. Берём по char_start
                    pass
                # Сравниваем по символьным позициям
                nat_t = self._natasha.tokens[i]
                if nat_t.start >= span.start and tok_start is None:
                    tok_start = i
                if nat_t.stop <= span.stop:
                    tok_end = i + 1
            if tok_start is None:
                tok_start = 0
            if tok_end is None:
                tok_end = tok_start + 1
            self.ents.append(_EntSpanCompat(
                text=ent_text,
                label=span.type,
                start_char=span.start,
                end_char=span.stop,
                tok_start=tok_start,
                tok_end=tok_end,
            ))

    def __iter__(self):
        return iter(self._tokens)

    @property
    def sents(self):
        """Генератор предложений как списков токенов.

        spaCy doc.sents возвращает Span-объекты, но svo_extract.py использует
        только token-итерацию внутри предложения. Возвращаем списки _TokenCompat.
        """
        # Natasha не имеет явной сегментации предложений в .tokens по умолчанию,
        # но если segmenter запущен — токены с .rel == 'root' начинают новое предложение
        current = []
        nat_tokens = self._natasha.tokens
        for i, nat_t in enumerate(nat_tokens):
            current.append(self._tokens[i])
            # Конец предложения: токен с punct или перед .rel == 'root'
            if nat_t.pos == 'PUNCT' or nat_t.text in '.!?':
                yield current
                current = []
        if current:
            yield current


class _NLPCompat:
    """spaCy-совместимый NLP-объект на базе Natasha.

    Используется poler_entities.py:
        doc = NLP(chunk)
        for ent in doc.ents:
            if ent.label_ == "PER" and ent.text in form_to_lemma: ...
        for token in doc:
            if token.pos_ == "PROPN" and token.text in form_to_lemma: ...
    """

    def __init__(self):
        from natasha import (
            Segmenter, NewsEmbedding, NewsNERTagger,
            NewsSyntaxParser, NewsMorphTagger, MorphVocab,
        )
        self._segmenter = Segmenter()
        self._emb = NewsEmbedding()
        self._ner_tagger = NewsNERTagger(self._emb)
        self._syntax_parser = NewsSyntaxParser(self._emb)
        self._morph_tagger = NewsMorphTagger(self._emb)
        self._morph = MorphVocab()

    def __call__(self, text: str) -> _DocCompat:
        from natasha import Doc
        doc = Doc(text)
        doc.segment(self._segmenter)
        doc.tag_ner(self._ner_tagger)
        # Syntax нужен для doc.sents (rel/head_id) и для morph tagging
        # (morph tagger требует syntax-разбор)
        try:
            doc.parse_syntax(self._syntax_parser)
            doc.tag_morph(self._morph_tagger)
            # MorphVocab исправляет леммы на основе morph-тегов
            for token in doc.tokens:
                token.lemmatize(self._morph)
        except Exception:
            pass  # syntax/morph не критичны для NER-only путей
        return _DocCompat(text, doc)


# Lazy singleton — инициализируем только при первом обращении (экономия 0.8с
# при использовании extract_entities без NLP-вызовов)
_NLP_INSTANCE = None


def get_nlp():
    global _NLP_INSTANCE
    if _NLP_INSTANCE is None:
        _NLP_INSTANCE = _NLPCompat()
    return _NLP_INSTANCE


# NLP = None на уровне модуля — poler_entities.py делает `from ner_extract import NLP`
# и ожидает callable. Делаем NLP callable proxy через lazy init.
class _NLPLazyProxy:
    """Прокси для lazy-init NLP. При первом вызове NLP(text) инициализирует Natasha."""

    def __call__(self, text: str) -> _DocCompat:
        return get_nlp()(text)

    def __getattr__(self, name):
        # Проксируем доступ к атрибутам реального NLP (pipe, disable, etc.)
        return getattr(get_nlp(), name)


NLP = _NLPLazyProxy()


# -----------------------------------------------------------------------------
# get_proper_lemma — нормализация регистра леммы
# -----------------------------------------------------------------------------
# v1-логика: если слово с большой буквы — это имя собственное, лемма должна
# сохранить регистр. pymorphy3 возвращает lowercase-лемму, мы восстанавливаем.
_MORPH_INSTANCE = None


def _get_morph():
    global _MORPH_INSTANCE
    if _MORPH_INSTANCE is None:
        try:
            from pymorphy3 import MorphAnalyzer
            _MORPH_INSTANCE = MorphAnalyzer()
        except ImportError:
            _MORPH_INSTANCE = False  # sentinel: pymorphy3 недоступен
    return _MORPH_INSTANCE


def get_proper_lemma(text: str, spacy_lemma: str = '') -> str:
    """Нормализовать лемму, сохраняя регистр собственных имён.

    v1-совместимая сигнатура: get_proper_lemma(text, spacy_lemma) -> str
    v2 игнорирует spacy_lemma (использует pymorphy3), но принимает аргумент
    для совместимости с вызовами `get_proper_lemma(token.text, token.lemma_)`.
    """
    if not text:
        return spacy_lemma or ''
    morph = _get_morph()
    if morph is False:
        # pymorphy3 недоступен — возвращаем spacy_lemma или исходный text
        return spacy_lemma or text
    parsed = morph.parse(text)
    if not parsed:
        return spacy_lemma or text
    normal = parsed[0].normal_form
    # Сохраняем регистр первой буквы для собственных имён
    if text and text[0].isupper():
        return normal.capitalize()
    return normal


# -----------------------------------------------------------------------------
# FALSE_POSITIVE_NOUNS — минимальный список часто-ошибочных нарицательных
# -----------------------------------------------------------------------------
# v1 содержал ~100 слов. v2/Natasha сама фильтрует через NER-тег (PER/LOC/ORG),
# но оставляем минимальный список универсальных нарицательных, которые Natasha
# может ошибочно тегировать как PER (особенно одушевлённые нарицательные).
FALSE_POSITIVE_NOUNS = frozenset({
    # Универсальные нарицательные (могут упоминаться с большой буквы в начале предложения)
    # УКР
    'Світло', 'Темрява', 'Тиша', 'Вогонь', 'Вода', 'Повітря', 'Земля', 'Небо',
    'Місто', 'Країна', 'Дім', 'Школа', 'Церква', 'Річка', 'Гора', 'Ліс',
    'День', 'Ніч', 'Ранок', 'Вечір', 'Час', 'Рік', 'Місяць', 'Тиждень',
    # РУС
    'Свет', 'Тьма', 'Тишина', 'Огонь', 'Вода', 'Воздух', 'Земля', 'Небо',
    'Город', 'Страна', 'Дом', 'Школа', 'Церковь', 'Река', 'Гора', 'Лес',
    'День', 'Ночь', 'Утро', 'Вечер', 'Время', 'Год', 'Месяц', 'Неделя',
    'Солнце', 'Луна', 'Звезда', 'Море', 'Океан', 'Поляна', 'Дорога', 'Путь',
    # EN
    'Light', 'Darkness', 'Silence', 'Fire', 'Water', 'Air', 'Earth', 'Sky',
    'City', 'Country', 'House', 'School', 'Church', 'River', 'Mountain', 'Forest',
    'Day', 'Night', 'Morning', 'Evening', 'Time', 'Year', 'Month', 'Week',
    'Sun', 'Moon', 'Star', 'Sea', 'Ocean', 'Road', 'Path',
})


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
        dict с ключами 'entities' и 'stats' + meta-поля для совместимости
        с Rust NerResult struct (commands/ner.rs).

    Контракт совместимости с Rust `NerResult`:
      Обязательные поля (без #[serde(default)] в Rust):
        - entities: Vec<Entity>
        - stats: {total, persons, locations, organizations}
        - model, version, truncated
        - textLength, processedLength
      Опциональные поля (Rust игнорирует unknown, для v1-compat оставлены):
        - stats.byLabel, stats.totalMentions
        - entity.gender, entity.first, entity.last, entity.middle
    """
    # Извлекаем персонажей через Natasha
    persons = extract_persons(text, min_freq=min_freq)
    entities = aggregate_persons(persons, text)

    text_length = len(text)
    persons_count = sum(1 for e in entities if e.get('label') == 'PER')

    # Статистика — обязательные поля v1-контракта + расширенные поля v2
    stats = {
        # v1-совместимые обязательные поля (Rust NerStats struct)
        'total': len(entities),
        'persons': persons_count,
        'locations': 0,   # v2 пока не извлекает LOC — будет добавлено в Phase 2
        'organizations': 0,  # v2 пока не извлекает ORG
        # v2-расширенные поля (Rust игнорирует через unknown fields)
        'byLabel': {'PER': persons_count},
        'totalMentions': sum(len(e['mentions']) for e in entities),
    }

    return {
        'entities': entities,
        'stats': stats,
        # Meta-поля для Rust NerResult struct
        'model': 'natasha-slovnet+pymorphy3',
        'version': '2.0',
        'truncated': False,
        'textLength': text_length,
        'processedLength': text_length,
        'chunksProcessed': 1,
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
