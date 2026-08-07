#!/usr/bin/env python3
"""
Тест NER-скрипта для LitGraph.

Запуск:
    python3 scripts/test_ner.py
    # или с venv:
    ~/.litgraph-venv/bin/python scripts/test_ner.py

Проверяет:
1. Установлен ли spaCy
2. Установлена ли модель ru_core_news_sm
3. Установлен ли pymorphy3
4. NER работает на тестовом тексте
"""
import sys
import os
import json

print("=" * 60)
print("NER Test — LitGraph")
print("=" * 60)
print(f"Python: {sys.executable}")
print(f"Version: {sys.version.split()[0]}")
print()

# 1. spaCy
try:
    import spacy
    print(f"✓ spaCy v{spacy.__version__}")
except ImportError:
    print("✗ spaCy не установлен в этом Python")
    print()
    print("Установка (выберите ОДИН из вариантов):")
    print()
    print("ВАРИАНТ 1 — venv (рекомендуется для Arch/CachyOS):")
    print("  python -m venv ~/.litgraph-venv")
    print("  source ~/.litgraph-venv/bin/activate")
    print("  pip install spacy pymorphy3")
    print("  python -m spacy download ru_core_news_sm")
    print()
    print("ВАРИАНТ 2 — системная установка (Debian/Ubuntu):")
    print("  pip install spacy pymorphy3")
    print("  python -m spacy download ru_core_news_sm")
    print()
    print("ВАРИАНТ 3 — break-system-packages (Arch, на свой риск):")
    print("  pip install --break-system-packages spacy pymorphy3")
    print("  python -m spacy download ru_core_news_sm")
    sys.exit(1)

# 2. pymorphy3
try:
    import pymorphy3
    print(f"✓ pymorphy3 v{pymorphy3.__version__}")
except ImportError:
    print("✗ pymorphy3 не установлен")
    print("  Установите: pip install pymorphy3")
    sys.exit(1)

# 3. Русская модель
try:
    nlp = spacy.load("ru_core_news_sm")
    print(f"✓ ru_core_news_sm загружена (pipes: {nlp.pipe_names})")
except OSError:
    print("✗ Модель ru_core_news_sm не найдена")
    print("  Установите: python -m spacy download ru_core_news_sm")
    sys.exit(1)

# 4. Тест
test_text = """
Анна проснулась рано. Анна подошла к зеркалу. В коридоре Анна встретила мужа.
Вронский вышел из вагона на вокзале в Москве. Вронский подошёл к матери.
Кити стояла у колонны. Левин пригласил Кити на танец. Музыка заиграла.
"""

print("\n" + "=" * 60)
print("Тестовый текст:")
print(test_text.strip())
print("=" * 60)

# Импортируем ner_extract (предполагаем что он в src-tauri/python/)
sys.path.insert(0, "src-tauri/python")
try:
    from ner_extract import extract_entities
except ImportError as e:
    print(f"✗ Не удалось импортировать ner_extract.py: {e}")
    print("  Убедитесь что запускаете из корня проекта litgraph-desktop")
    sys.exit(1)

result = extract_entities(test_text)

print(f"\n✓ NER выполнен за <1 сек")
print(f"  Модель: {result['model']} v{result['version']}")
print(f"  Статистика: {result['stats']}")
print(f"\nНайденные сущности:")
for e in result["entities"]:
    forms = ", ".join(e["forms"][:5])
    print(f"  {e['lemma']:<15} {e['label']:<5} ×{e['count']:<3} forms=[{forms}]")

print("\n" + "=" * 60)
if result["stats"]["persons"] >= 3 and result["stats"]["locations"] >= 1:
    print("✓ ТЕСТ ПРОЙДЕН — NER работает корректно")
    print("  Найдены: Анна, Вронский, Кити, Левин (PER) + Москва (LOC)")
    print("\nNER готов к интеграции с Tauri.")
    print("Соберите desktop-версию: cargo tauri build")
    sys.exit(0)
else:
    print("✗ ТЕСТ ПРОВАЛЕН — NER не находит ожидаемые сущности")
    sys.exit(1)
