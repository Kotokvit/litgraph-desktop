# POLER Math Toolkit — реестр инструментов

Для разработки математического ядра POLER (Phase 4) — без LLM/PyTorch.
Все инструменты — open-source, устанавливаются через `pip` или системный
пакетный менеджер.

## Python-стек (основной)

| Пакет | Версия | Назначение в POLER |
|---|---|---|
| **sympy** | 1.14 | Символьные вычисления: операторная алгебра, антисимметричные матрицы, Lie-скобки, интегралы действия S = ∫L dt |
| **numpy** | 2.5 | Численная линейная алгебра: матрицы J, A, F; собственные значения H = L + iγJ − B/m |
| **scipy** | 1.18 | Sparse CSR-матрицы (для больших графов), SVD, разложения Шура, eigsh для больших симметричных систем |
| **networkx** | 3.6 | Теория графов: directed graphs, междуness-центральность, поиск циклов (для разрешения кореферентности) |
| **sparse** | 0.19 | N-мерные разреженные тензоры — для представления T[i,j,k] = (subject, verb, object) |
| **tensorly** | 0.9 | Тензорные разложения: CP, Tucker, PARAFAC. Декомпозиция SVO-тензора на латентные факторы |
| **clifford** | 1.5 | Алгебра Клиффорда: A (симметричный) + J (антисимметричный) → Cl(p,q). Это естественный язык для POLER-операторов |
| **gudhi** | 3.13 | Топологический анализ данных: симплициальные комплексы, persistent homology. Для структуры повествования |
| **persim** | 0.3 | Persistence diagrams — расстояние между топологическими сигнатурами двух текстов |
| **scikit-learn** | 1.5 | Кластеризация (DBSCAN для alias-resolution), PCA, метрики |
| **matplotlib** | 3.10 | Визуализация матриц, графов, диаграмм |
| **graphviz** | — | DOT-графы (для синтаксических деревьев и операторных схем) |
| **pandas** | 3.0 | Табличное представление корпусов |
| **jupyter lab** | 4.5 | Интерактивная разработка с LaTeX-формулами |

## Что пока НЕ нужно (отказ от эмпирического подхода)

| Отказ | Почему |
|---|---|
| ❌ PyTorch / TensorFlow | Это frameworks для fitting параметрических моделей через SGD. POLER строится из явных операторов, не из фит-моделей |
| ❌ transformers (BERT/GPT) | Embeddings-модели не дают понимания — они дают сжатие. POLER требует **алгебраических** операторов, а не статистики совместной встречаемости |
| ❌ spaCy (как основа) | Оставим только как токенайзер. Dependency parsing у spaCy — статистический, без семантики |
| ❌ Word2Vec / fastText | Линейные embeddings — слишком бедное пространство |

## Установка на машине разработчика

```bash
# Linux (Ubuntu/Debian)
sudo apt install python3-venv graphviz libgraphviz-dev texlive-latex-extra

# Python-окружение
python3 -m venv ~/.litgraph-math
source ~/.litgraph-math/bin/activate
pip install sympy numpy scipy networkx sparse tensorly clifford \
            gudhi persim scikit-learn matplotlib graphviz pandas jupyterlab

# Запуск Jupyter Lab
jupyter lab
```

## Структура рабочей директории

```
litgraph-desktop/docs/poler_math/
├── TOOLKIT.md            ← этот файл
├── 01_operator_algebra.ipynb     — операторы A, J, H, F, ε, R[n]
├── 02_j_matrix_axioms.ipynb      — аксиомы J (антисимметричность, etc.)
├── 03_clifford_embedding.ipynb   — J ∈ Cl(p,q), алгебра Клиффорда
├── 04_topology_of_text.ipynb     — persistent homology сцен
├── 05_coreference_as_fixed_point.ipynb  — P² = P, проекции
└── 06_poler_hamiltonian.ipynb    — H = L + iγJ − B/m, спектр
```

## Следующий шаг

После ответов на вопросы из `QUESTIONS_FOR_MATHEMATICIAN.md` — выбрать один
из блоков (01-06) и начать проработку.
