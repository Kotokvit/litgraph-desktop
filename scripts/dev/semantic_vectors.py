"""
Семантические векторы без трансформеров.

Три источника признаков:
  1. RuWordNet — синонимы, гиперонимы (для группировки «сказал/произнёс/ответил»)
  2. pymorphy3 features — POS, падеж, род, число, одушевлённость, аспект
  3. fastText (опционально) — 300-dim семантические векторы (если установлена модель)

Использование:
    from scripts.dev.semantic_vectors import (
        get_synonyms, get_hypernyms,
        build_linguistic_vector, similarity
    )
"""
from __future__ import annotations

from typing import Optional
import numpy as np

# pymorphy3 для морфологических признаков
try:
    import pymorphy3
    MORPH = pymorphy3.MorphAnalyzer()
except ImportError:
    MORPH = None

# RuWordNet для синонимов и гиперонимов
try:
    from ruwordnet import RuWordNet
    _wn = RuWordNet()  # загружает БД при инициализации
except Exception:
    _wn = None

# fastText (опционально)
try:
    import fasttext
    _ft = None  # ленивая инициализация
except ImportError:
    _ft = None


# =============================================================================
# RUWORDNET — синонимы и гиперонимы
# =============================================================================

def get_synonyms(word: str) -> list[str]:
    """Получить синонимы слова через RuWordNet.

    Args:
        word: лемма слова

    Returns:
        Список синонимов (включая само слово)
    """
    if _wn is None:
        return [word]

    try:
        synsets = _wn.get_synsets(word)
        synonyms = set()
        for synset in synsets:
            for sense in synset.senses:
                synonyms.add(sense.name.lower())
        synonyms.add(word.lower())
        return sorted(synonyms)
    except Exception:
        return [word]


def get_hypernyms(word: str) -> list[str]:
    """Получить гиперонимы (более общие понятия).

    Пример: человек → живое существо, личность
    """
    if _wn is None:
        return []

    try:
        synsets = _wn.get_synsets(word)
        hypernyms = set()
        for synset in synsets:
            for hyp in synset.hypernyms:
                for sense in hyp.senses:
                    hypernyms.add(sense.name.lower())
        return sorted(hypernyms)
    except Exception:
        return []


def are_synonyms(word1: str, word2: str) -> bool:
    """Проверить, являются ли слова синонимами.

    Полезно для SVO: сказал/произнёс/ответил = одно действие.
    """
    if word1.lower() == word2.lower():
        return True
    return word2.lower() in get_synonyms(word1)


# =============================================================================
# PYMORPHY3 FEATURES — лингвистические векторы
# =============================================================================

# Размер лингвистического вектора:
#   POS (15) + Case (8) + Gender (4) + Number (3) + Animacy (2) +
#   Aspect (3) + Tense (4) + Mood (3) + Person (4) + Transitivity (2) + Involvement (2)
# = ~50 features

POS_TAGS = ['NOUN', 'VERB', 'ADJF', 'ADJS', 'PRTF', 'PRTS', 'GRND', 'NUMR',
            'ADVB', 'NPRO', 'PREP', 'CONJ', 'PRCL', 'INTJ', 'PNCT']
CASE_TAGS = ['nomn', 'gent', 'datv', 'accs', 'ablt', 'loct', 'voct', 'gen2']
GENDER_TAGS = ['masc', 'femn', 'neut', 'ms-f']
NUMBER_TAGS = ['sing', 'plur', 'SGTV']
ANIMACY_TAGS = ['anim', 'inan']
ASPECT_TAGS = ['perf', 'impf', 'Sfpf']
TENSE_TAGS = ['pres', 'past', 'futr', 'PRTF']
MOOD_TAGS = ['indc', 'impr', 'INDC']
PERSON_TAGS = ['1per', '2per', '3per', 'PER1']
TRANS_TAGS = ['tran', 'intr']
INVOLV_TAGS = ['incl', 'excl']

FEATURE_NAMES = (
    [f'pos_{t}' for t in POS_TAGS] +
    [f'case_{t}' for t in CASE_TAGS] +
    [f'gender_{t}' for t in GENDER_TAGS] +
    [f'number_{t}' for t in NUMBER_TAGS] +
    [f'animacy_{t}' for t in ANIMACY_TAGS] +
    [f'aspect_{t}' for t in ASPECT_TAGS] +
    [f'tense_{t}' for t in TENSE_TAGS] +
    [f'mood_{t}' for t in MOOD_TAGS] +
    [f'person_{t}' for t in PERSON_TAGS] +
    [f'trans_{t}' for t in TRANS_TAGS] +
    [f'involv_{t}' for t in INVOLV_TAGS]
)


def build_linguistic_vector(word: str) -> np.ndarray:
    """Построить лингвистический вектор слова (one-hot из pymorphy3 тегов).

    Размер вектора: ~50. Никакой нейросети, чистая грамматика.

    Args:
        word: слово в любой форме (pymorphy3 сам лемматизирует)

    Returns:
        np.ndarray размером len(FEATURE_NAMES)
    """
    vec = np.zeros(len(FEATURE_NAMES), dtype=np.float32)

    if MORPH is None:
        return vec

    try:
        p = MORPH.parse(word)[0]
        tag_str = str(p.tag)

        for i, t in enumerate(POS_TAGS):
            if t in tag_str:
                vec[i] = 1.0
                break

        offset = len(POS_TAGS)
        for i, t in enumerate(CASE_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(CASE_TAGS)
        for i, t in enumerate(GENDER_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(GENDER_TAGS)
        for i, t in enumerate(NUMBER_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(NUMBER_TAGS)
        for i, t in enumerate(ANIMACY_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(ANIMACY_TAGS)
        for i, t in enumerate(ASPECT_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(ASPECT_TAGS)
        for i, t in enumerate(TENSE_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(TENSE_TAGS)
        for i, t in enumerate(MOOD_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(MOOD_TAGS)
        for i, t in enumerate(PERSON_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(PERSON_TAGS)
        for i, t in enumerate(TRANS_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break

        offset += len(TRANS_TAGS)
        for i, t in enumerate(INVOLV_TAGS):
            if t in tag_str:
                vec[offset + i] = 1.0
                break
    except Exception:
        pass

    return vec


# =============================================================================
# FASTTEXT (опционально)
# =============================================================================

_ft_model = None

def load_fasttext_model(path: str) -> bool:
    """Загрузить fastText модель.

    Args:
        path: путь к .bin файлу (cc.ru.300.bin или подобный)

    Returns:
        True если загрузка успешна
    """
    global _ft_model
    try:
        _ft_model = fasttext.load_model(path)
        return True
    except Exception as e:
        print(f"Failed to load fastText model: {e}")
        return False


def get_fasttext_vector(word: str) -> Optional[np.ndarray]:
    """Получить 300-dim семантический вектор через fastText.

    Returns:
        np.ndarray размером 300, или None если модель не загружена
    """
    if _ft_model is None:
        return None
    try:
        return _ft_model.get_word_vector(word.lower())
    except Exception:
        return None


# =============================================================================
# SIMILARITY
# =============================================================================

def similarity(vec1: np.ndarray, vec2: np.ndarray) -> float:
    """Косинусная близость двух векторов."""
    norm1 = np.linalg.norm(vec1)
    norm2 = np.linalg.norm(vec2)
    if norm1 == 0 or norm2 == 0:
        return 0.0
    return float(np.dot(vec1, vec2) / (norm1 * norm2))


def word_similarity(word1: str, word2: str, use_fasttext: bool = False) -> float:
    """Семантическая близость двух слов.

    Args:
        use_fasttext: если True и модель загружена — использовать fastText (300 dim).
                     Иначе — лингвистический вектор (~50 dim).
    """
    if use_fasttext and _ft_model is not None:
        v1 = get_fasttext_vector(word1)
        v2 = get_fasttext_vector(word2)
        if v1 is not None and v2 is not None:
            return similarity(v1, v2)

    v1 = build_linguistic_vector(word1)
    v2 = build_linguistic_vector(word2)
    return similarity(v1, v2)


# =============================================================================
# CLI
# =============================================================================

if __name__ == '__main__':
    import sys
    import json

    if len(sys.argv) == 1:
        # Демо
        print("=== Synonyms (RuWordNet) ===")
        for w in ['человек', 'сказать', 'идти', 'бить']:
            syns = get_synonyms(w)
            print(f"  {w}: {syns[:5]}")

        print("\n=== Hypernyms ===")
        for w in ['человек', 'собака', 'стол']:
            hyps = get_hypernyms(w)
            print(f"  {w}: {hyps[:5]}")

        print("\n=== Linguistic vectors ===")
        for w in ['человек', 'человека', 'люди', 'сказал', 'скажет']:
            v = build_linguistic_vector(w)
            nonzero = [(FEATURE_NAMES[i], v[i]) for i in range(len(v)) if v[i] > 0]
            print(f"  {w:12s} → {nonzero}")

        print("\n=== Similarity ===")
        pairs = [('человек', 'человека'), ('человек', 'люди'),
                 ('сказал', 'скажет'), ('бить', 'ударить')]
        for w1, w2 in pairs:
            sim_ling = word_similarity(w1, w2)
            are_syn = are_synonyms(w1, w2)
            print(f"  sim({w1}, {w2}) = {sim_ling:.3f}  synonyms={are_syn}")
    elif len(sys.argv) >= 3:
        w1, w2 = sys.argv[1], sys.argv[2]
        result = {
            'word1': w1,
            'word2': w2,
            'similarity_linguistic': word_similarity(w1, w2),
            'are_synonyms': are_synonyms(w1, w2),
            'synonyms_w1': get_synonyms(w1)[:10],
            'synonyms_w2': get_synonyms(w2)[:10],
        }
        print(json.dumps(result, ensure_ascii=False, indent=2))
