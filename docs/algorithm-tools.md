# Algorithm Development Tools — LitGraph POLER

**Дата:** 2026-08-11
**Назначение:** Полный каталог инструментов и ресурсов для разработки алгоритмов понимания текста в LitGraph.

Это **не** ML/transformer-стек. Это rule-based + graph + лингвистический стек для построения алгоритмов, которые **программируются понимать текст**, а не угадывают следующий токен.

---

## 1. Извлечение сущностей и фактов (Rule-based)

### spaCy + ru_core_news_sm
- **URL:** https://spacy.io/models/ru
- **GitHub:** https://github.com/explosion/spaCy
- **Установка:** `pip install spacy && python -m spacy download ru_core_news_sm`
- **В LitGraph:** уже используется в `src-tauri/python/ner_extract.py`, `svo_extract.py`
- **Что даёт:** Tokenization, POS-tagging, dependency parsing, NER для русского языка. Базовый слой для всех остальных алгоритмов.
- **Лицензия:** MIT (spaCy), CC BY-SA 3.0 (модель ru_core_news_sm)

### pymorphy3
- **GitHub:** https://github.com/no-plagiarism/pymorphy3
- **Установка:** `pip install pymorphy3`
- **В LitGraph:** используется для лемматизации и морфоанализа
- **Что даёт:** Полный морфологический анализ русских слов (падеж, род, число, одушевлённость, аспект). Превосходит spaCy для русского.
- **Лицензия:** MIT

### Yargy (Томита-парсер для Python)
- **GitHub:** https://github.com/naturalanguage/yargy
- **Docs:** https://yargy.readthedocs.io/
- **Установка:** `pip install yargy`
- **В LitGraph:** NEW — будет использоваться для замены чёрных списков и regex-ов в `ner_extract.py`
- **Что даёт:** Контекстно-свободные грамматики для извлечения сущностей и фактов. Позволяет писать правила вида `PER -> (Name) (Surn)?` со словарями падежей.
- **Лицензия:** MIT

### Natasha
- **GitHub:** https://github.com/natasha/natasha
- **Docs:** https://github.com/natasha/natasha#readme
- **Установка:** `pip install natasha`
- **В LitGraph:** NEW — запасной парсер для русского, Slovnet-модели
- **Что даёт:** Slovnet NER, syntax, morphology. Меньше и быстрее spaCy для русского.
- **Лицензия:** MIT

### ipymarkup (для визуализации Yargy/Natasha)
- **GitHub:** https://github.com/natasha/ipymarkup
- **Установка:** `pip install ipymarkup`
- **Что даёт:** HTML-визуализация NER-разметки и dependency parse
- **Лицензия:** MIT

---

## 2. Графовые алгоритмы

### NetworkX
- **URL:** https://networkx.org/
- **GitHub:** https://github.com/networkx/networkx
- **Docs:** https://networkx.org/documentation/stable/
- **Установка:** `pip install networkx` (уже установлен)
- **Что даёт:**
  - **Community detection:** Louvain, Leiden, label propagation — для сегментации сцен
  - **Centrality:** betweenness, eigenvector, closeness — для определения протагониста
  - **Paths:** shortest paths, all-pairs — для эволюции отношений между персонажами
  - **Subgraphs:** для выделения кластеров сцен
- **Лицензия:** BSD-3-Clause

### igraph (C backend)
- **URL:** https://igraph.org/
- **GitHub:** https://github.com/igraph/python-igraph
- **Docs:** https://python.igraph.org/
- **Установка:** `pip install igraph` (уже установлен)
- **Что даёт:** То же что NetworkX, но в 10-100× быстрее на больших графах. Использовать когда текст > 100K токенов.
- **Лицензия:** GPL-2.0+

### pyvis (интерактивная визуализация)
- **GitHub:** https://github.com/WestHealth/pyvis
- **Docs:** https://pyvis.readthedocs.io/
- **Установка:** `pip install pyvis` (уже установлен)
- **Что даёт:** Интерактивные HTML-графы с drag&drop узлов. Использовать для отладки J-матрицы и graph of characters.
- **Лицензия:** BSD-3-Clause

### displaCy (визуализация dependency parse)
- **URL:** https://spacy.io/usage/visualizers
- **Установка:** встроено в spaCy
- **Что даёт:** SVG-деревья зависимостей. Без неё отладка SVO — ад.
- **Использование:** `spacy.displacy.serve(doc, style='dep')` или `spacy.displacy.render(doc, style='dep', jupyter=True)`

---

## 3. Семантические векторы (без трансформеров)

### fastText (Facebook)
- **URL:** https://fasttext.cc/
- **GitHub:** https://github.com/facebookresearch/fastText
- **Docs:** https://fasttext.cc/docs/en/crawl-vectors.html
- **Pre-trained модели:** https://dl.fbaipublicfiles.com/fasttext/vectors-crawl/cc.ru.300.bin.gz
- **Установка:** `pip install fasttext-wheel` (уже установлен)
- **Размер:** 2.0 GB (compressed), 3 GB (uncompressed)
- **Внимание:** модель cc.ru.300.bin большая. Для разработки можно использовать .vec формат (текстовый, можно загрузить частично).
- **Что даёт:** Skip-gram модель. Вектор слова = 300 чисел. Семантическая близость через косинус.
- **Лицензия:** MIT

### gensim
- **URL:** https://radimrehurek.com/gensim/
- **GitHub:** https://github.com/RaRe-Technologies/gensim
- **Установка:** `pip install gensim` (уже установлен)
- **Что даёт:** Word2Vec, Doc2Vec, FastText API (медленнее native fasttext, но удобнее). Можно обучать свою модель на корпусе.
- **Лицензия:** LGPL-2.1+

### Кастомные лингвистические векторы (без внешних моделей)
Из pymorphy3 можно построить features:
- POS (часть речи) — one-hot
- Падеж — one-hot
- Род — one-hot
- Число — one-hot
- Одушевлённость — one-hot
- Аспект (глагол) — one-hot
- Переходность (глагол) — bool

Размер: ~20-30 features на слово. Никакой нейросети. Использовать для грамматических правил.

---

## 4. Русские лингвистические ресурсы

### RuWordNet (русский WordNet)
- **URL:** http://ruwordnet.ru/ru/
- **GitHub (Python wrapper):** https://github.com/avidale/python-ruwordnet
- **Установка:** `pip install ruwordnet && python -m ruwordnet download`
- **В LitGraph:** NEW — для группировки синонимов в SVO (сказал/произнёс/ответил = одно действие)
- **Что даёт:**
  - 59 905 синсетов, 154 111 смыслов
  - Синонимы (сказал/произнёс/ответил)
  - Гиперонимы (человек → живое существо)
  - Гипонимы
- **Лицензия:** MIT (wrapper), CC BY-NC-SA (данные RuWordNet)

### RuSentiLex (словарь тональности)
- **URL:** http://www.labinform.ru/pub/rusentilex/index.htm
- **Прямой файл:** http://www.labinform.ru/pub/rusentilex/rusentilex_2017.txt
- **В LitGraph:** NEW — для полярности слов в J-матрице (агрессия vs помощь)
- **Размер:** 1.3 MB, ~12 000 записей
- **Что даёт:**
  - Тональность: positive / negative / neutral / positive/negative
  - Источник: opinion / feeling / fact
  - Лемматизированная форма
- **Лицензия:** бесплатно для академического использования

### OpenCorpora
- **URL:** http://opencorpora.org/
- **Использование:** pymorphy3 использует их словарь
- **Что даёт:** Полный морфологический словарь русского языка

### Национальный корпус русского языка
- **URL:** https://ruscorpora.ru/
- **Что даёт:** Частотности, примеры употребления, грамматическая разметка
- **Использование:** для калибровки алгоритмов на реальных текстах

---

## 5. Тестирование и среда разработки

### JupyterLab
- **URL:** https://jupyter.org/
- **GitHub:** https://github.com/jupyterlab/jupyterlab
- **Установка:** `pip install jupyterlab` (уже установлен)
- **В LitGraph:** NEW — основная среда для интерактивной разработки алгоритмов
- **Что даёт:**
  - Вставляешь абзац текста → видишь parse tree → NER → SVO → J-матрицу → граф
  - Меняешь правило — сразу видишь diff
  - Markdown + код + визуализация в одном документе
- **Запуск:** `jupyter lab --notebook-dir=scripts/dev`

### Hypothesis (property-based testing)
- **URL:** https://hypothesis.readthedocs.io/
- **GitHub:** https://github.com/HypothesisWorks/hypothesis
- **Установка:** `pip install hypothesis` (уже установлен)
- **Что даёт:**
  - Тестирование инвариантов: «для любого русского предложения с явным субъектом и переходным глаголом SVO находит триплет»
  - Автоматическая генерация тестовых случаев
  - Поиск edge cases
- **Лицензия:** MPL-2.0

### pytest
- **URL:** https://docs.pytest.org/
- **Установка:** `pip install pytest`
- **Что даёт:** Стандартный фреймворк тестирования. Использовать вместе с Hypothesis.
- **Лицензия:** MIT

---

## 6. Дополнительные инструменты

### Lark (generic parser)
- **GitHub:** https://github.com/lark-parser/lark
- **Установка:** `pip install lark`
- **Что даёт:** Если нужна своя грамматика для шаблонов предложений (например, «кто кого ударил» как формальная конструкция). EBNF-синтаксис.
- **Лицензия:** MIT

### stanza (Stanford NLP)
- **URL:** https://stanfordnlp.github.io/stanza/
- **GitHub:** https://github.com/stanfordnlp/stanza
- **Установка:** `pip install stanza`
- **Что даёт:** Альтернатива spaCy с поддержкой русского. Лучше для многоязычных задач.
- **Лицензия:** Apache-2.0

### summa (TextRank)
- **GitHub:** https://github.com/summanlp/textract
- **Установка:** `pip install summa`
- **Что даёт:** TextRank для извлечения ключевых слов и суммаризации
- **Лицензия:** Apache-2.0

### rapidfuzz (fuzzy string matching)
- **GitHub:** https://github.com/maxbachmann/RapidFuzz
- **Установка:** `pip install rapidfuzz`
- **Что даёт:** Быстрый fuzzy matching. Для разрешения алиасов («Лёша» = «Алексей» = «Алексей Петрович»).
- **Лицензия:** MIT

---

## 7. Установленные в LitGraph пакеты

После `pip install -r src-tauri/python/requirements.txt`:

```
# Базовый слой (уже был)
spacy>=3.8.0,<4.0.0
pymorphy3>=2.0.0
numpy>=1.24.0
scipy>=1.10.0
scikit-learn>=1.3.0

# NEW: Rule-based extraction
yargy>=0.16.0
natasha>=1.4.0
ipymarkup>=0.0.5

# NEW: Графовые алгоритмы
networkx>=3.4
igraph>=0.11
pyvis>=0.3

# NEW: Семантические векторы
fasttext-wheel>=0.2.5
gensim>=4.3

# NEW: Русские лингвистические ресурсы
ruwordnet>=0.0.6
# RuSentiLex скачивается как файл, не через pip

# NEW: Тестирование и разработка
hypothesis>=6.115
pytest>=8.0
jupyterlab>=4.2
rapidfuzz>=3.0
```

---

## 8. Структура разработки

```
scripts/dev/
├── notebook.ipynb              # Jupyter: интерактивная разработка
├── grammar/                    # Yargy-правила для русских конструкций
│   ├── __init__.py
│   ├── person.py               # ФИО, прозвища, occupational names
│   ├── action.py               # глаголы действия с падежами объектов
│   ├── location.py             # топонимы + interior locations
│   └── templates.py            # SVO-шаблоны предложений
├── graph_analysis.py           # NetworkX: communities, centrality, paths
├── semantic_vectors.py         # fastText + RuWordNet + custom features
├── sentiment.py                # RuSentiLex интеграция
├── property_tests.py           # Hypothesis-инварианты
├── visualize/
│   ├── displacy_render.py      # parse trees
│   ├── pyvis_graph.py          # interactive graph
│   └── j_matrix_heatmap.py     # тепловая карта J-матрицы
└── resources/
    ├── rusentilex_2017.txt     # 1.3 MB — словарь тональности
    └── (ruwordnet.db           # устанавливается через python -m ruwordnet download)
```

---

## 9. Использование

### Быстрый старт
```bash
cd /home/z/my-project/litgraph-desktop
pip install -r src-tauri/python/requirements.txt
python -m spacy download ru_core_news_sm
python -m ruwordnet download

# Запуск Jupyter для разработки алгоритмов
jupyter lab --notebook-dir=scripts/dev
```

### Тесты на корпусе
```bash
# Старый пайплайн (после рефакторинга сохраняет CLI)
python src-tauri/python/ner_extract.py tests/corpus/01_conflict_scene.md
python src-tauri/python/svo_extract.py tests/corpus/01_conflict_scene.md

# Регрессионный прогон
for f in tests/corpus/*.md; do
  python src-tauri/python/poler_entities.py "$f" --json > "tests/corpus/results/$(basename $f .md).json"
done

# Property-based тесты
pytest scripts/dev/property_tests.py -v
```

---

## 10. Что НЕ используется (намеренно)

- **Transformers (BERT, GPT, T5):** мы программируем алгоритм, а не используем чужой LLM
- **Tokenizers (HuggingFace):** нам не нужна BPE-токенизация
- **PyTorch / TensorFlow:** нейросети не нужны для rule-based extraction
- **LangChain / LlamaIndex:** это фреймворки поверх LLM, не для нас
- **OpenAI API / Anthropic API:** внешний AI не используется в ядре
- **SBERT (sentence-transformers):** только в опциональном модуле similarity для сценария 05_duplicate_scenes, не в основном пайплайне
