"""
Извлечение персонажей (PER) из русского текста через Natasha.

Замена чёрным спискам и regex-ам в ner_extract.py.

Natasha использует:
  - Segmenter — разбиение на предложения и токены
  - NewsNERTagger — NER на Slovnet-модели (без pymorphy2)
  - NewsMorphTagger — POS, падеж, род, число
  - MorphVocab — лемматизация

Дополнительная валидация через pymorphy3 (Name/Surn теги).

Поддерживаемые шаблоны:
  1. Одно имя:            Алексей
  2. Имя + Фамилия:       Алексей Сорокин
  3. Полное ФИО:          Владимир Петрович Сорокин
  4. Только фамилия:      Сорокин (если встречается с другими PER)
  5. Уменьшительное:      Лёша, Маша
"""
from __future__ import annotations

import warnings
from typing import Iterable

# Natasha (без NamesExtractor, который зависит от Yargy/pymorphy2)
from natasha import (
    Segmenter,
    NewsEmbedding,
    NewsNERTagger,
    NewsMorphTagger,
    NewsSyntaxParser,
    MorphVocab,
    Doc,
)

# pymorphy3 — для валидации Name/Surn тегов и gender
try:
    import pymorphy3
    MORPH = pymorphy3.MorphAnalyzer()
except ImportError:
    MORPH = None

warnings.filterwarnings('ignore', category=UserWarning, module='pymorphy2')


# =============================================================================
# СЛОВАРИ ИЗВЕСТНЫХ ИМЁН (для разрешения уменьшительных и нерусских)
# =============================================================================

KNOWN_FIRST_NAMES_MASC = {
    'алексей', 'александр', 'андрей', 'антон', 'арсений', 'артём', 'артем',
    'борис', 'вадим', 'валентин', 'валерий', 'василий', 'виктор', 'виталий',
    'владимир', 'владислав', 'вячеслав', 'геннадий', 'георгий', 'григорий',
    'денис', 'дмитрий', 'евгений', 'игорь', 'илья', 'иван', 'кирилл',
    'константин', 'леонид', 'максим', 'матвей', 'михаил', 'никита', 'николай',
    'олег', 'павел', 'пётр', 'петр', 'роман', 'руслан', 'сергей', 'степан',
    'тимофей', 'тимур', 'фёдор', 'федор', 'юрий', 'ярослав',
    # Уменьшительные мужские
    'лёша', 'лёха', 'сёма', 'паша', 'веня', 'жора', 'костя', 'дима', 'женя',
    'саша', 'витя', 'гена', 'юра', 'толя', 'вова', 'слава',
}

KNOWN_FIRST_NAMES_FEM = {
    'анна', 'алёна', 'алена', 'алиса', 'алла', 'анастасия', 'ангелина',
    'валентина', 'валерия', 'варвара', 'вера', 'вероника', 'виктория',
    'галина', 'дарья', 'дария', 'евгения', 'екатерина', 'елена', 'елизавета',
    'зинаида', 'инна', 'ирина', 'кира', 'клавдия', 'лариса', 'людмила',
    'любовь', 'маргарита', 'мария', 'надежда', 'наталья', 'наталия', 'нина',
    'оксана', 'олёна', 'олена', 'ольга', 'полина', 'раиса', 'регина',
    'светлана', 'софья', 'софия', 'таисия', 'тамара', 'татьяна', 'ульяна',
    'юлия', 'яна',
    # Уменьшительные женские
    'маша', 'катя', 'настя', 'лена', 'таня', 'аня', 'оля', 'надя', 'люда',
    'нина', 'вика', 'даша', 'соня', 'валя',
}

KNOWN_FIRST_NAMES = KNOWN_FIRST_NAMES_MASC | KNOWN_FIRST_NAMES_FEM


# =============================================================================
# ЧЁРНЫЙ СПИСОК (по-прежнему нужен — Natasha иногда ошибается на нарицательных,
# особенно в начале предложения или в фэнтези/фантастике)
# =============================================================================

FALSE_POSITIVE_NOUNS = {
    # Время (часто в начале предложения)
    'утро', 'день', 'вечер', 'ночь', 'рассвет', 'закат', 'полдень', 'полночь',
    'вчера', 'сегодня', 'завтра', 'послезавтра', 'весна', 'лето', 'осень', 'зима',
    'понедельник', 'вторник', 'среда', 'четверг', 'пятница', 'суббота', 'воскресенье',
    # Стороны света
    'север', 'юг', 'восток', 'запад',
    # Природа/элементы
    'вода', 'воздух', 'огонь', 'земля', 'свет', 'тьма', 'тепло', 'холод',
    'море', 'океан', 'река', 'озеро', 'гора', 'лес', 'поле', 'небо',
    'солнце', 'луна', 'звезда', 'ветер', 'дождь', 'снег', 'гроза',
    # Абстрактные понятия
    'мир', 'жизнь', 'смерть', 'любовь', 'надежда', 'страх', 'гнев',
    'правда', 'ложь', 'свобода', 'судьба', 'удача',
    'боль', 'радость', 'грусть', 'печаль', 'тоска',
    # Социальные понятия (могут быть в начале предложения)
    'город', 'деревня', 'совет', 'центр', 'королевство', 'империя',
}


# =============================================================================
# ЭКСТРАКТОР
# =============================================================================

class PersonExtractor:
    """Извлекатель персонажей через Natasha NER + pymorphy3 валидация.

    Возвращает список словарей в формате совместимом с ner_extract.py:
    [
        {
            "text": "Владимир Петрович Сорокин",
            "lemma": "Владимир Петрович Сорокин",
            "start": 123,
            "end": 145,
            "label": "PER",
            "gender": "Masc",
        },
        ...
    ]
    """

    _instance = None  # Singleton: Natasha модели дорогие

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._instance._init_natasha()
        return cls._instance

    def _init_natasha(self):
        self.segmenter = Segmenter()
        self.emb = NewsEmbedding()
        self.ner_tagger = NewsNERTagger(self.emb)
        self.morph_tagger = NewsMorphTagger(self.emb)
        self.syntax_parser = NewsSyntaxParser(self.emb)
        self.morph_vocab = MorphVocab()

    def extract(self, text: str, min_freq: int = 1) -> list[dict]:
        """Извлечь всех персонажей из текста.

        Args:
            text: исходный текст
            min_freq: минимальная частота (фильтр шума, по умолчанию 1 = без фильтра)

        Returns:
            список вхождений PER, отсортированных по позиции
        """
        doc = Doc(text)
        doc.segment(self.segmenter)
        doc.tag_morph(self.morph_tagger)
        doc.tag_ner(self.ner_tagger)

        results = []
        for span in doc.spans:
            if span.type != 'PER':
                continue

            span_text = span.text
            lower = span_text.lower()

            # Чёрный список (нарицательные, которые Natasha ошибочно пометила)
            if lower in FALSE_POSITIVE_NOUNS:
                continue

            # Лемматизация через MorphVocab
            try:
                span.normalize(self.morph_vocab)
                lemma = span.normal or span_text
            except Exception:
                lemma = self._fallback_lemmatize(span_text)

            # Пол через pymorphy3 (более точный, чем Natasha)
            gender = self._detect_gender(span_text)

            results.append({
                'text': span_text,
                'lemma': lemma,
                'start': span.start,
                'end': span.stop,
                'label': 'PER',
                'gender': gender,
                'first': self._extract_first_name(span_text, lemma),
                'last': self._extract_last_name(span_text, lemma),
                'middle': self._extract_middle_name(span_text, lemma),
            })

        # Дополнительная валидация через pymorphy3: ищем PROPN, которые Natasha пропустила
        # (Natasha пропускает некоторые имена в начале предложения, в диалогах и т.д.)
        results.extend(self._find_missed_persons(doc, text, results))

        # Применяем min_freq
        if min_freq > 1:
            from collections import Counter
            lemma_counts = Counter(r['lemma'].lower() for r in results)
            results = [r for r in results if lemma_counts[r['lemma'].lower()] >= min_freq]

        # Сортируем по позиции и убираем дубликаты по span
        results = self._deduplicate(results)
        results.sort(key=lambda x: x['start'])
        return results

    def _fallback_lemmatize(self, text: str) -> str:
        """Лемматизация через pymorphy3 если MorphVocab не справился."""
        if MORPH is None:
            return text
        parts = []
        for word in text.split():
            p = MORPH.parse(word)
            if p:
                nf = p[0].normal_form
                parts.append(nf[0].upper() + nf[1:] if nf else word)
            else:
                parts.append(word)
        return ' '.join(parts)

    def _detect_gender(self, span_text: str) -> str:
        """Определить пол персонажа через pymorphy3."""
        if MORPH is None:
            return 'Unknown'
        first_word = span_text.split()[0]
        try:
            p = MORPH.parse(first_word)[0]
            tag_str = str(p.tag)
            if 'masc' in tag_str:
                return 'Masc'
            if 'femn' in tag_str:
                return 'Fem'
            if 'neut' in tag_str:
                return 'Neut'
        except Exception:
            pass
        return 'Unknown'

    def _extract_first_name(self, span_text: str, lemma: str) -> str | None:
        """Извлечь имя (первое слово если это Name по pymorphy3)."""
        if MORPH is None:
            return span_text.split()[0] if span_text.split() else None
        for word in span_text.split():
            p = MORPH.parse(word)
            if p and ('Name' in str(p[0].tag) or word.lower() in KNOWN_FIRST_NAMES):
                # Лемматизируем
                nf = p[0].normal_form
                return nf[0].upper() + nf[1:] if nf else word
        return None

    def _extract_last_name(self, span_text: str, lemma: str) -> str | None:
        """Извлечь фамилию (Surn по pymorphy3)."""
        if MORPH is None:
            return None
        for word in span_text.split():
            p = MORPH.parse(word)
            if p and 'Surn' in str(p[0].tag):
                nf = p[0].normal_form
                return nf[0].upper() + nf[1:] if nf else word
        return None

    def _extract_middle_name(self, span_text: str, lemma: str) -> str | None:
        """Извлечь отчество (Patr по pymorphy3)."""
        if MORPH is None:
            return None
        for word in span_text.split():
            p = MORPH.parse(word)
            if p and 'Patr' in str(p[0].tag):
                nf = p[0].normal_form
                return nf[0].upper() + nf[1:] if nf else word
        return None

    def _find_missed_persons(self, doc: Doc, text: str, found: list[dict]) -> list[dict]:
        """Найти PROPN, которые Natasha пропустила.

        Срабатывает когда:
          - слово помечено как PROPN в morph
          - лемма не в чёрном списке
          - pymorphy3 подтверждает Name/Surn ИЛИ слово в KNOWN_FIRST_NAMES
          - не входит в уже найденный span
        """
        results = []
        found_spans = [(r['start'], r['end']) for r in found]

        def in_found_span(pos):
            for s, e in found_spans:
                if s <= pos < e:
                    return True
            return False

        # Используем top-level doc.tokens — у них есть start/stop
        for tok in doc.tokens:
            if tok.pos != 'PROPN':
                continue
            if in_found_span(tok.start):
                continue

            word = tok.text
            lower = word.lower()

            # Чёрный список
            if lower in FALSE_POSITIVE_NOUNS:
                continue

            # Проверка pymorphy3
            is_person = False
            if MORPH is not None:
                try:
                    p = MORPH.parse(word)
                    if p and ('Name' in str(p[0].tag) or 'Surn' in str(p[0].tag)):
                        is_person = True
                except Exception:
                    pass

            # Проверка по словарю известных имён
            if not is_person and lower in KNOWN_FIRST_NAMES:
                is_person = True

            if not is_person:
                continue

            # Лемматизация
            lemma = self._fallback_lemmatize(word)
            gender = self._detect_gender(word)

            results.append({
                'text': word,
                'lemma': lemma,
                'start': tok.start,
                'end': tok.stop,
                'label': 'PER',
                'gender': gender,
                'first': self._extract_first_name(word, lemma),
                'last': self._extract_last_name(word, lemma),
                'middle': self._extract_middle_name(word, lemma),
            })

        return results

    def _deduplicate(self, results: list[dict]) -> list[dict]:
        """Убрать дубликаты по span (start, end)."""
        seen = set()
        out = []
        for r in results:
            key = (r['start'], r['end'])
            if key in seen:
                continue
            seen.add(key)
            out.append(r)
        return out


# =============================================================================
# УДОБНЫЙ ВХОД
# =============================================================================

_default_extractor = None


def extract_persons(text: str, min_freq: int = 1) -> list[dict]:
    """Извлечь всех персонажей из текста.

    Usage:
        from scripts.dev.grammar.person import extract_persons
        persons = extract_persons("Алексей Сорокин вошёл в кабинет.")
        # [{'text': 'Алексей Сорокин', 'lemma': 'Алексей Сорокин', 'gender': 'Masc', ...}]
    """
    global _default_extractor
    if _default_extractor is None:
        _default_extractor = PersonExtractor()
    return _default_extractor.extract(text, min_freq=min_freq)


if __name__ == '__main__':
    import sys
    import json

    if len(sys.argv) < 2:
        print("Usage: python -m scripts.dev.grammar.person <text-or-file>")
        sys.exit(1)

    arg = sys.argv[1]
    if arg.endswith('.md') or arg.endswith('.txt'):
        with open(arg, encoding='utf-8') as f:
            text = f.read()
    else:
        text = arg

    persons = extract_persons(text, min_freq=2)
    print(json.dumps(persons, ensure_ascii=False, indent=2))
