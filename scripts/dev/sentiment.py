"""
Тональность русских слов через RuSentiLex.

RuSentiLex — словарь из ~12 000 русских слов с разметкой:
  - positive / negative / neutral / positive/negative
  - source: opinion / feeling / fact

Используется в build_j_matrix.py для определения полярности действий
(агрессия vs помощь), но может применяться к любым словам.

Файл словаря: scripts/dev/resources/rusentilex_2017.txt
Источник: http://www.labinform.ru/pub/rusentilex/rusentilex_2017.txt
"""
from __future__ import annotations

import os
from pathlib import Path
from typing import Optional


# =============================================================================
# ЗАГРУЗКА СЛОВАРЯ
# =============================================================================

_RESOURCE_PATH = Path(__file__).parent / 'resources' / 'rusentilex_2017.txt'

_lexicon: dict[str, dict] | None = None


def load_lexicon(path: Path | str | None = None) -> dict[str, dict]:
    """Загрузить RuSentiLex.

    Returns:
        {lemma: {'pos': 'A|V|N', 'sentiment': 'positive|negative|neutral|positive/negative',
                 'source': 'opinion|feeling|fact'}}
    """
    global _lexicon
    if _lexicon is not None and path is None:
        return _lexicon

    if path is None:
        path = _RESOURCE_PATH
    path = Path(path)

    if not path.exists():
        raise FileNotFoundError(
            f"RuSentiLex not found at {path}. "
            f"Download from http://www.labinform.ru/pub/rusentilex/rusentilex_2017.txt"
        )

    lexicon = {}
    with open(path, encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('!'):
                continue

            # Формат: слово, POS, лемма, тональность, источник, (опц. понятие)
            parts = line.split(',')
            if len(parts) < 5:
                continue

            word = parts[0].strip().lower()
            pos = parts[1].strip()
            lemma = parts[2].strip().lower()
            sentiment = parts[3].strip().lower()
            source = parts[4].strip().lower()

            lexicon[lemma] = {
                'pos': pos,
                'sentiment': sentiment,
                'source': source,
            }

    if path is None:
        _lexicon = lexicon
    return lexicon


# =============================================================================
# API
# =============================================================================

def get_sentiment(word: str) -> str:
    """Получить тональность слова.

    Returns:
        'positive', 'negative', 'neutral', 'positive/negative', или 'unknown'
    """
    lex = load_lexicon()
    entry = lex.get(word.lower())
    if entry:
        return entry['sentiment']
    return 'unknown'


def get_polarity_weight(word: str, negated: bool = False) -> float:
    """Получить вес полярности для POLER J-матрицы.

    Args:
        word: лемматизированный глагол
        negated: True если глагол стоит с отрицанием («не сделал»)

    Returns:
        Вес: 2.0 для агрессии, 1.0 для помощи, 0.5 для нейтрального.
        Если отрицание — вес умножается на 0.3.
    """
    sentiment = get_sentiment(word)

    if sentiment == 'negative':
        w = 2.0  # агрессия
    elif sentiment == 'positive':
        w = 1.0  # помощь
    elif sentiment == 'positive/negative':
        w = 1.5  # неоднозначное
    else:
        w = 1.0  # нейтральное или unknown

    if negated:
        # Отрицание: действие не совершено, но намерение было.
        # Ослабляем вес, но сохраняем знак.
        w *= 0.3

    return w


def is_aggression_word(word: str) -> bool:
    """Проверить, выражает ли слово агрессию."""
    return get_sentiment(word) == 'negative'


def is_assistance_word(word: str) -> bool:
    """Проверить, выражает ли слово помощь."""
    return get_sentiment(word) == 'positive'


def get_aggression_words() -> set[str]:
    """Все слова с негативной тональностью (для расширения словаря SVO)."""
    lex = load_lexicon()
    return {w for w, e in lex.items() if e['sentiment'] == 'negative'}


def get_assistance_words() -> set[str]:
    """Все слова с позитивной тональностью."""
    lex = load_lexicon()
    return {w for w, e in lex.items() if e['sentiment'] == 'positive'}


# =============================================================================
# CLI
# =============================================================================

if __name__ == '__main__':
    import sys
    import json

    if len(sys.argv) < 2:
        # Демо
        test_words = ['гнев', 'любовь', 'ударил', 'помог', 'равнодушие', 'радость', 'страх']
        print("=== Demo ===")
        for w in test_words:
            s = get_sentiment(w)
            w_polarity = get_polarity_weight(w)
            print(f"  {w:15s} sentiment={s:20s} weight={w_polarity}")
        print(f"\nTotal entries: {len(load_lexicon())}")
    else:
        word = sys.argv[1]
        result = {
            'word': word,
            'sentiment': get_sentiment(word),
            'polarity_weight': get_polarity_weight(word),
            'is_aggression': is_aggression_word(word),
            'is_assistance': is_assistance_word(word),
        }
        print(json.dumps(result, ensure_ascii=False, indent=2))
