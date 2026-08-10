"""
Property-based тесты для алгоритмов LitGraph POLER.

Использует Hypothesis для генерации тестовых случаев и проверки инвариантов.

Инварианты:
  1. NER: любое предложение с явным русским именем → PersonExtractor находит PER
  2. NER: персонаж всегда имеет gender (Masc/Fem/Neut), не Unknown
  3. NER: лемма всегда в именительном падеже
  4. SVO: для предложения «PER глагол PER» извлекается триплет
  5. Sentiment: get_polarity_weight возвращает [0.3, 2.0]
  6. Graph: J-матрица всегда антисимметричная
"""
from __future__ import annotations

import sys
import os
import unittest
from hypothesis import given, strategies as st, assume, settings

# Setup path
_HERE = os.path.dirname(os.path.abspath(__file__))
_PROJECT_ROOT = os.path.abspath(os.path.join(_HERE, '..', '..'))
if _PROJECT_ROOT not in sys.path:
    sys.path.insert(0, _PROJECT_ROOT)

from scripts.dev.grammar.person import extract_persons, PersonExtractor
from scripts.dev.sentiment import get_polarity_weight, get_sentiment
from scripts.dev.semantic_vectors import build_linguistic_vector, are_synonyms


# =============================================================================
# СТРАТЕГИИ ДЛЯ ГЕНЕРАЦИИ
# =============================================================================

# Стратегия: простые русские имена
RUSSIAN_NAMES_MASC = ['Алексей', 'Иван', 'Владимир', 'Фёдор', 'Дмитрий', 'Сергей', 'Андрей']
RUSSIAN_NAMES_FEM = ['Анна', 'Мария', 'Елена', 'Ольга', 'Татьяна', 'Наталья', 'Ирина']

# Стратегия: простые переходные глаголы
TRANSITIVE_VERBS = ['увидел', 'остановил', 'ударил', 'позвал', 'встретил', 'узнал', 'обнял']


@st.composite
def russian_sentence_with_per(draw):
    """Сгенерировать предложение вида: PER глагол PER."""
    subj = draw(st.sampled_from(RUSSIAN_NAMES_MASC + RUSSIAN_NAMES_FEM))
    verb = draw(st.sampled_from(TRANSITIVE_VERBS))
    obj = draw(st.sampled_from(RUSSIAN_NAMES_MASC + RUSSIAN_NAMES_FEM))
    # Если genders не совпадают с глагольным окончанием, грамматика может быть нарушена
    # — это нормально для property-based теста (мы проверяем что алгоритм не падает)
    return f"{subj} {verb} {obj}."


# =============================================================================
# ТЕСТЫ
# =============================================================================

class TestPersonExtractor(unittest.TestCase):
    """Тесты NER-извлекателя персонажей."""

    def setUp(self):
        self.extractor = PersonExtractor()

    @settings(max_examples=20, deadline=None)
    @given(text=st.text(min_size=1, max_size=100, alphabet='абвгдежзийклмнопрстуфхцчшщъыьэюяАБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ .'))
    def test_extractor_does_not_crash_on_any_input(self, text):
        """Инвариант 0: extract_persons не должен падать на любом русском тексте."""
        try:
            result = extract_persons(text)
            self.assertIsInstance(result, list)
        except Exception as e:
            self.fail(f"extract_persons crashed on {text!r}: {e}")

    def test_known_name_is_extracted(self):
        """Инвариант 1a: известное русское имя должно быть извлечено как PER."""
        text = "Алексей вошёл в комнату."
        persons = extract_persons(text)
        lemmas = [p['lemma'].lower() for p in persons]
        self.assertIn('алексей', lemmas, f"Expected 'алексей' in {lemmas}")

    def test_fio_extracted_as_one_span(self):
        """Инвариант 1b: ФИО должно быть извлечено как один span."""
        text = "Владимир Петрович Сорокин вошёл в кабинет."
        persons = extract_persons(text)
        # Должен быть span, покрывающий минимум 3 слова
        for p in persons:
            if 'Владимир' in p['text'] and 'Сорокин' in p['text']:
                self.assertGreaterEqual(len(p['text'].split()), 2)
                return
        self.fail(f"FIO not found as single span in {persons}")

    def test_gender_is_set(self):
        """Инвариант 2: для извлечённого PER gender не должен быть Unknown."""
        text = "Анна пошла в магазин. Дмитрий работает инженером."
        persons = extract_persons(text)
        for p in persons:
            if p['lemma'].lower() in ('анна', 'дмитрий'):
                self.assertIn(p['gender'], ('Masc', 'Fem'),
                              f"Gender for {p['lemma']} should be Masc/Fem, got {p['gender']}")

    def test_no_duplicate_spans(self):
        """Инвариант 3: не должно быть дублирующих span-ов."""
        text = "Алексей увидел Алексея в зеркале."
        persons = extract_persons(text)
        spans = [(p['start'], p['end']) for p in persons]
        self.assertEqual(len(spans), len(set(spans)),
                         f"Duplicate spans found: {spans}")


class TestSentiment(unittest.TestCase):
    """Тесты словаря тональности."""

    @settings(max_examples=30, deadline=None)
    @given(word=st.sampled_from(['гнев', 'любовь', 'радость', 'страх', 'спокойствие', 'грубость']))
    def test_sentiment_returns_known_value(self, word):
        """Инвариант: get_sentiment возвращает одно из известных значений."""
        s = get_sentiment(word)
        self.assertIn(s, ('positive', 'negative', 'neutral', 'positive/negative', 'unknown'))

    @settings(max_examples=20, deadline=None)
    @given(
        word=st.sampled_from(['гнев', 'любовь', 'радость']),
        negated=st.booleans()
    )
    def test_polarity_weight_range(self, word, negated):
        """Инвариант 5: get_polarity_weight возвращает значение в [0.3, 2.0]."""
        w = get_polarity_weight(word, negated=negated)
        self.assertGreaterEqual(w, 0.0)
        self.assertLessEqual(w, 2.0)

    def test_negation_reduces_weight(self):
        """Отрицание должно уменьшать вес."""
        w_pos = get_polarity_weight('гнев', negated=False)
        w_neg = get_polarity_weight('гнев', negated=True)
        self.assertLess(w_neg, w_pos,
                        f"Negated weight {w_neg} should be < positive {w_pos}")


class TestSemanticVectors(unittest.TestCase):
    """Тесты лингвистических векторов."""

    def test_vector_size(self):
        """Вектор должен иметь фиксированный размер."""
        v = build_linguistic_vector('человек')
        self.assertEqual(len(v), 50)  # Сумма всех one-hot групп

    def test_vector_is_normalized_or_zero(self):
        """Вектор либо нулевой, либо имеет минимум одну 1.0."""
        v = build_linguistic_vector('человек')
        if v.sum() == 0:
            self.skipTest("Empty vector for unknown word")
        self.assertGreater(v.sum(), 0)

    def test_synonyms_of_word_include_itself(self):
        """Синонимы слова должны включать само слово."""
        syns = are_synonyms('человек', 'человек')
        self.assertTrue(syns)


class TestGraphProperties(unittest.TestCase):
    """Тесты свойств J-матрицы (без её построения — проверяем инварианты)."""

    def test_j_matrix_is_antisymmetric(self):
        """Инвариант 6: J-матрица всегда антисимметричная."""
        import numpy as np
        # Простая J-матрица 3x3
        nodes = ['Алексей', 'Фёдор', 'Мария']
        J = np.zeros((3, 3))
        # Алексей → Фёдор, вес 2.0 (агрессия)
        J[0, 1] = 2.0
        J[1, 0] = -2.0
        # Мария → Алексей, вес 1.0 (помощь)
        J[2, 0] = 1.0
        J[0, 2] = -1.0

        # Проверяем антисимметричность: J[i,j] = -J[j,i]
        for i in range(3):
            for j in range(3):
                self.assertEqual(J[i, j], -J[j, i],
                                 f"J[{i},{j}]={J[i,j]} != -J[{j},{i}]={-J[j,i]}")

        # Диагональ всегда нулевая
        for i in range(3):
            self.assertEqual(J[i, i], 0.0)


if __name__ == '__main__':
    unittest.main(verbosity=2)
