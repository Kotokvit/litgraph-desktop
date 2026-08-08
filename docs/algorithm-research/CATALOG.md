# Каталог алгоритмов и инструментов для разработки

Дата поиска: 2026-08-09
Назначение: подбор алгоритмов, полезных для построения новых алгоритмов (в контексте проекта litgraph-desktop — нодового редактора литературных текстов с NER/POLER/SVO анализом), и инструментов/программ для разработки алгоритмов.

---

## Часть A. Алгоритмы, полезные для построения новых алгоритмов

### A1. Графовые алгоритмы и построение графов знаний (Knowledge Graph)

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | Algorithms and methods for automated construction of KG (Russian texts) | https://www.e3s-conferences.org/articles/e3sconf/abs/2024/61/e3sconf_uesf2024_03017/e3sconf_uesf2024_03017.html | Построение графов знаний из русскоязычных текстов; обзор библиотек и методологий. |
| 2 | Methods for Knowledge Graph Construction from Text (arXiv) | https://arxiv.org/abs/2603.25862 | NLP + ML + Generative AI методы для KG construction. |
| 3 | Knowledge graph learning algorithm based on deep convolutional networks | https://www.sciencedirect.com/science/article/pii/S2667305324000619 | Глубокие сверточные сети для повышения точности классификации в KG. |
| 4 | The State of the Art: LLMs for Knowledge Graph Construction (IBM) | https://research.ibm.com/publications/the-state-of-the-art-large-language-models-for-knowledge-graph-construction-from-text-techniques-tools-and-challenges | Современные LLM-методы для построения KG. |
| 5 | Knowledge Graph Construction: Extraction, Learning, and Reasoning (MDPI, 2025) | https://www.mdpi.com/2076-3417/15/7/3727 | Обзор: GNN + extraction методы, 64 цитирования. |
| 6 | Ontology-guided Knowledge Graph Construction from Text (ACL 2024) | https://aclanthology.org/2024.kallm-1.8.pdf | Использование онтологий + LLM для KG. |
| 7 | Knowledge Graph Algorithms (Meegle) | https://www.meegle.com/en_us/topics/knowledge-graphs/knowledge-graph-algorithms | Обзор вычислительных методов обработки KG. |

### A2. Advanced NER (распознавание именованных сущностей)

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | Recent Advances in Named Entity Recognition (arXiv, 2024) | https://arxiv.org/html/2401.10825v3 | Современные достижения NER на базе глубокого обучения. |
| 2 | Transformer models in biomedicine (NIH PMC) | https://pmc.ncbi.nlm.nih.gov/articles/PMC11287876 | BERT-модели, XAI; 150 цитирований. |
| 3 | From Neural Networks to Transformers: The Evolution of ML | https://www.dataversity.net/articles/from-neural-networks-to-transformers-the-evolution-of-machine-learning | Эволюция RNN/LSTM → Transformer. |
| 4 | Easy NER with ML and HuggingFace Transformers | https://github.com/christianversloot/machine-learning-articles/blob/main/easy-named-entity-recognition-with-machine-learning-and-huggingface-transformers.md | Практический туториал BERT NER. |
| 5 | Seeking Advice on NER with AI (HuggingFace discuss) | https://discuss.huggingface.co/t/seeking-advice-on-named-entity-recognition-with-ai/136564 | Использование pre-trained моделей для ускорения NER. |
| 6 | Evaluating Medical Entity Recognition (JMIR, 2024) | https://medinform.jmir.org/2024/1/e59782 | BERT и LLM в медицинском NER. |

### A3. Кластеризация и тематическое моделирование

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | Document Segmentation for Topic Modelling with embeddings (IEEE, 2024) | https://ieeexplore.ieee.org/document/10467643 | Сегментация документов через embeddings и векторные дистанции. |
| 2 | Topic modeling or Text clustering: What approaches are best (Reddit) | https://www.reddit.com/r/LanguageTechnology/comments/iyem6w/topic_modeling_or_text_clustering_what_approaches | Обсуждение подходов к topic modeling. |
| 3 | Integrating document clustering and topic modeling (ACM) | https://dl.acm.org/doi/10.5555/3023638.3023709 | Multi-grain clustering topic model (MGCTM). |
| 4 | Document Clustering — overview (ScienceDirect) | https://www.sciencedirect.com/topics/computer-science/document-clustering | Категории алгоритмов: agglomerative, partitioning, EM. |
| 5 | Integrated clustering and BERT framework (PMC, 2023) | https://pmc.ncbi.nlm.nih.gov/articles/PMC10163298 | 4-компонентная архитектура: feature extraction + BERT + clustering; 220 цитирований. |
| 6 | Clustering algorithms explained (Serokell, 2024) | https://serokell.io/blog/clustering-algorithms-in-ml | Обзор ML-кластеризации. |

### A4. Семантическое сходство и эмбеддинги предложений

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | Semantic Textual Similarity (SBERT docs) | https://sbert.net/docs/sentence_transformer/usage/semantic_textual_similarity.html | Эмбеддинги + косинусное сходство для STS. |
| 2 | SentenceTransformers Documentation | https://sbert.net | Главная Python-библиотека для SOTA эмбеддингов. |
| 3 | Evaluating semantic text similarity using SBERT and NLTK | https://medium.com/@merobi/evaluating-semantic-text-similarity-using-sbert-and-nltk-18f08e51566d | Практическое руководство. |
| 4 | Sentence representations for semantic textual similarity (ScienceDirect, 2026) | https://www.sciencedirect.com/science/article/pii/S0885230826000331 | Сравнение подходов к эмбеддингам. |
| 5 | Evaluating semantic similarity using sentence embeddings (DiVA, 2021) | https://www.diva-portal.org/smash/get/diva2:1536646/FULLTEXT01.pdf | Магистерская работа по BERT-эмбеддингам. |
| 6 | Large Language Models: SBERT - Sentence-BERT | https://towardsdatascience.com/sbert-deb3d4aef8a4 | Введение в SBERT и его применение. |

### A5. Алгоритмы анализа нарратива и сюжета

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | Three Stage Narrative Analysis; Plot-Sentiment Breakdown (arXiv, 2025) | https://arxiv.org/html/2511.11857v1 | Анализ sentiment-дуг сценариев + structure learning + concept detection. |
| 2 | Three Stage Narrative Analysis (ResearchGate) | https://www.researchgate.net/publication/397701586_Three_Stage_Narrative_Analysis_Plot-Sentiment_Breakdown_Structure_Learning_and_Concept_Detection | PDF-версия работы. |
| 3 | Survey on Narrative Structure: Linguistic Theories to Automatic Analysis (ACL, 2022) | https://aclanthology.org/2022.tal-1.3.pdf | Обзор: кластеризация, correspondence analysis, deep learning. |
| 4 | Study on LLMs in Story Generation (DiVA Portal) | https://www.diva-portal.org/smash/get/diva2:1887928/FULLTEXT01.pdf | Plot structure + consistent character development. |
| 5 | Plot extraction and the visualization of narrative flow (Cambridge, 2023) | https://www.cambridge.org/core/journals/natural-language-engineering/article/plot-extraction-and-the-visualization-of-narrative-flow/445A7A36F339A280AA1EA5A6612373A0 | Автоматическое извлечение сюжета + визуализация. |
| 6 | Finding the Narrative with NLP (Medium) | https://scrapfishies.medium.com/finding-the-narrative-with-natural-language-processing-47177d20f743 | Практический подход. |

### A6. Алгоритмы выделения сообществ (Community Detection)

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | Louvain method (Wikipedia) | https://en.wikipedia.org/wiki/Louvain_method | Greedy-оптимизация модулярности для крупных сетей. |
| 2 | What is the Louvain Method? (PuppyGraph) | https://www.puppygraph.com/blog/louvain | Описание метода 2008 г. |
| 3 | Accelerate Community Detection in Python Using GPU (NVIDIA) | https://developer.nvidia.com/blog/how-to-accelerate-community-detection-in-python-using-gpu-powered-leiden | GPU-ускоренная реализация Leiden. |
| 4 | From Louvain to Leiden: guaranteeing well-connected communities (Nature, 2019) | https://www.nature.com/articles/s41598-019-41695-z | Классическая статья — улучшение Louvain. |
| 5 | Louvain (Neo4j Graph Data Science) | https://neo4j.com/docs/graph-data-science/current/algorithms/louvain | Реализация в Neo4j GDS. |
| 6 | Understanding the Leiden Algorithm (Medium) | https://medium.com/@balci.pelin/understanding-the-leiden-algorithm-0b9fc95b277d | Пояснение с примерами. |

### A7. Извлечение отношений (Relation Extraction)

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | A survey on cutting-edge relation extraction techniques (arXiv, 2024) | https://arxiv.org/html/2411.18157v1 | Обзор SOTA методов RE. |
| 2 | Relationship Extraction in NLP (GeeksforGeeks) | https://www.geeksforgeeks.org/nlp/relationship-extraction-in-nlp | Введение в RE. |
| 3 | A survey on Relation Extraction (ScienceDirect, 2023) | https://www.sciencedirect.com/science/article/pii/S2667305323000698 | 107 цитирований, методы RE. |
| 4 | What is Information Extraction? IE for ML (Kili) | https://kili-technology.com/blog/information-extraction-ie-guide | IE как фундамент NLP. |
| 5 | What is Information Extraction? (IBM) | https://www.ibm.com/think/topics/information-extraction | IE-алгоритмы IBM. |
| 6 | NLP for Relation Extraction (AnnotationBox) | https://annotationbox.com/nlp-for-relation-extraction | AI-driven RE для KG. |

### A8. Кореферентное разрешение и привязка сущностей (Coreference & Entity Linking)

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | Entity Linking and Relationship Extraction With Relik (Neo4j) | https://medium.com/neo4j/entity-linking-and-relationship-extraction-with-relik-in-llamaindex-ca18892c169f | Coreference + entity linking + RE без LLM. |
| 2 | Entity Linking & Coreference Resolution (EmergentMind) | https://www.emergentmind.com/topics/entity-linking-and-coreference-resolution | Связывание упоминаний с сущностями KG. |
| 3 | Cross-Document Contextual Coreference Resolution (arXiv, 2025) | https://arxiv.org/html/2504.05767v1 | Coreference между документами в KG. |
| 4 | Coreference Resolution and Entity Linking (Stanford SLP3) | https://web.stanford.edu/~jurafsky/slp3/26.pdf | Учебник Jurafsky & Martin. |
| 5 | Entity generation algorithm based on reference expansion | https://www.sciencedirect.com/science/article/pii/S1674862X23000368 | Алгоритм: NER → candidate generation → entity disambiguation. |
| 6 | Combining entity resolution and knowledge graphs (Linkurious) | https://linkurious.com/blog/entity-resolution-knowledge-graph | Entity resolution + KG. |

### A9. Графовые нейронные сети (GNN / GAT / GCN)

| № | Название / Источник | Ссылка | Краткое описание |
|---|---|---|---|
| 1 | Global-Local Graph Neural Networks for Node-Classification (arXiv, 2024) | https://arxiv.org/html/2406.10863v1 | Глобальная+локальная информация в GNN. |
| 2 | Node classification based on structure migration and graph attention (ScienceDirect, 2025) | https://www.sciencedirect.com/science/article/abs/pii/S0950705124014473 | Structure Migration + Graph Attention. |
| 3 | Comprehensive Guide to GNN, GAT, and GCN (Medium) | https://medium.com/@joycebirkins/comprehensive-guide-to-gnn-gat-and-gcn-a-beginners-introduction-to-graph-neural-networks-after-51d09ac043b5 | Введение: GCN, GAT, GraphSage. |
| 4 | Node classification with GCN (StellarGraph) | https://stellargraph.readthedocs.io/en/stable/demos/node-classification/gcn-node-classification.html | Демо на StellarGraph. |
| 5 | Best Graph Neural Network architectures: GCN, GAT, GraphSAGE, MPNN | https://theaisummer.com/gnn-architectures | SOTA GNN-архитектуры. |
| 6 | ENode-GAT: GNN Node Classification Application (MDPI, 2023) | https://www.mdpi.com/2076-3417/13/12/7150 | Модель для малых выборок. |

---

## Часть B. Программы и инструменты для разработки алгоритмов

### B1. Визуальные/node-based редакторы для построения алгоритмов

| № | Инструмент | Ссылка | Описание |
|---|---|---|---|
| 1 | Designing your own node-based visual programming language (dev.to) | https://dev.to/cosmomyzrailgorynych/designing-your-own-node-based-visual-programming-language-2mpg | Туториал по созданию визуального языка (вдохновлён Unreal Blueprints). |
| 2 | xyflow/awesome-node-based-uis (GitHub) | https://github.com/xyflow/awesome-node-based-uis | Кураторский список node-based UI библиотек и редакторов. |
| 3 | Modern no-code graph editor/visualization tool (Reddit) | https://www.reddit.com/r/compsci/comments/1tcmcya/i_built_a_modern_nocode_graph_editorvisualization | Кастомизация нод и рёбер: формы, цвета, размеры. |
| 4 | Nodes — a new way to create with code | https://nodes.io/story | JavaScript 2D-canvas для вычислительного мышления и визуализации данных. |
| 5 | Tools for building a Graph/Node based UI in webapp (StackOverflow) | https://stackoverflow.com/questions/72164885/tools-for-building-a-graph-node-based-user-interface-in-a-webapp | Обзор инструментов для web-based node UI. |
| 6 | 2026: The Year of the Node-Based Editor (Medium) | https://medium.com/@fadimantium/2026-the-year-of-the-node-based-editor-941f0f15d467 | Weavy и другие visual pipeline редакторы. |

### B2. Визуализация алгоритмов и IDE

| № | Инструмент | Ссылка | Описание |
|---|---|---|---|
| 1 | Visualizing algorithms with Jupyter notebooks | https://pminkov.github.io/blog/visualizing-algorithms-with-jupyter-notebooks.html | Пошаговая визуализация алгоритмов в Jupyter. |
| 2 | mikeroyal/Jupyter-Guide (GitHub) | https://github.com/mikeroyal/Jupyter-Guide | Полный гайд по Jupyter + Image Processing Toolbox. |
| 3 | Interactive Learning with Jupyter Notebooks (публикация) | https://dergipark.org.tr/en/download/article-file/3234628 | Jupyter как инструмент обучения design алгоритмов. |
| 4 | Python GUI Libraries for Sorting Algorithm Visualization (Reddit) | https://www.reddit.com/r/Python/comments/d002rk/python_gui_libraries_for_sorting_algorithm | matplotlib + Jupyter widgets для визуализации. |

### B3. Платформы для практики и соревнований по алгоритмам

| № | Платформа | Ссылка | Описание |
|---|---|---|---|
| 1 | LeetCode — The World's Leading Online Programming Platform | https://leetcode.com | 4250+ задач, контесты, подготовка к интервью. |
| 2 | LeetCode Problemset | https://leetcode.com/problemset | Каталог задач с фильтрами по сложности и темам. |
| 3 | Mastering Competitive Programming (LeetCode discuss) | https://leetcode.com/discuss/general-discussion/5346386/Mastering-Competitive-Programming%3A-Strategies-for-Excelling-Under-Time-Constraints | Стратегии для time-constrained соревнований. |
| 4 | Best LeetCode Alternatives for Coding Practice (AlgoCademy) | https://algocademy.com/blog/top-leetcode-alternatives-for-coding-practice | Обзор 12 платформ-альтернатив. |
| 5 | Which is the best platform for competitive programming (Quora) | https://www.quora.com/Which-is-the-best-platform-for-competitive-programming-LeetCode-or-CodeChef-for-a-beginner-and-why | LeetCode vs CodeChef для новичков. |

---

## Часть C. Рекомендованные пакеты для установки (команды)

Ниже — список конкретных пакетов, которые можно установить локально для построения новых алгоритмов в контексте litgraph-desktop. Все команды рассчитаны на Python 3.10+ / Node 18+.

### C1. Python: NLP + графы + эмбеддинги

```bash
# Базовые NLP
pip install spacy pymorphy3 nltk
python -m spacy download ru_core_news_lg
python -m spacy download en_core_web_lg

# Transformers + BERT
pip install transformers torch sentencepiece accelerate

# Эмбеддинги предложений (SBERT)
pip install sentence-transformers

# Графовые алгоритмы
pip install networkx igraph python-louvain leidenalg cdlib

# GNN
pip install torch-geometric stellargraph dgl

# Кластеризация + topic modeling
pip install scikit-learn hdbscan umap-learn bertopic

# Извлечение отношений и coreference
pip install span_marker relik fastcoref spacy-coref

# Визуализация
pip install matplotlib seaborn plotly pyvis
```

### C2. Node.js / TypeScript (для frontend litgraph-desktop)

```bash
# Node-based UI (уже используется в проекте)
npm install @xyflow/react

# Графовые алгоритмы в JS
npm install graphology graphology-communities-louvain graphology-layout-force
npm install cytoscape cytoscape-fcose

# Визуализация
npm install d3 vis-network
```

### C3. Инструменты разработки алгоритмов

```bash
# Jupyter — интерактивная разработка
pip install jupyterlab notebook ipywidgets
pip install jupyter-contrib-nbextensions

# Algorithm visualization
pip install nbtutor algorithm-x

# Профилирование и бенчмарки
pip install pytest-benchmark py-spy memory-profiler
```

---

## Часть D. Структура файлов исследования

```
/home/z/my-project/download/algorithm-research/
├── README.md                       — этот сводный каталог
├── 01-graph-algorithms.json        — KG construction (7-9 результатов)
├── 02-ner-algorithms.json          — NER + Transformers (9-12)
├── 03-clustering.json              — кластеризация документов (9-12)
├── 04-dev-tools.json               — node-based dev tools (9-12)
├── 05-semantic.json                — SBERT / STS (9-12)
├── 06-narrative.json               — narrative structure analysis (9-12)
├── 07-community.json               — Louvain / Leiden (9-12)
├── 08-visualization.json           — Jupyter + IDE (9-12)
├── 09-relation-extraction.json     — RE techniques (9-12)
├── 10-coreference.json             — coreference + entity linking (9-12)
├── 11-platforms.json               — LeetCode + альтернативы (9-12)
└── 12-gnn.json                     — GNN / GAT / GCN (9-12)
```

Каждый JSON-файл содержит массив объектов с полями: `url`, `name`, `snippet`, `host_name`, `rank`, `date`, `favicon`.

---

## Часть E. Прикладные рекомендации для litgraph-desktop

С учётом стека проекта (Tauri 2 + Rust + React 19 + Python NER/POLER/SVO), приоритетные алгоритмы для интеграции:

1. **SBERT (sentence-transformers)** — для вычисления семантической близости между сценами/героями. Прямая замена эвристик POLER на embeddings даст более точные рёбра в графе персонажей.
2. **Leiden algorithm** (пакет `leidenalg`) — для выделения кластеров сцен и глав. Уже есть `graphology-communities-louvain` на фронтенде, но backend-реализация даст более крупные графы.
3. **Relik** (coreference + RE без LLM) — упростит построение графа сущностей без вызова AI API. Особенно актуально, учитывая что текущие AI-команды в litgraph не передают provider.
4. **BERTopic** — для автоматического выделения тем из глав .md-файла. Дополнит существующий `themes.rs` парсер.
5. **PyTorch Geometric** — если в будущем нужна классификация типов нод (персонаж/локация/тема) через GNN вместо regex+NER.
