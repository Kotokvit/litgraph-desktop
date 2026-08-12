# Phase 2 Step 4: Teaching Loop

Цикл "генерация → проверка → обучение → улучшение" для тренування
`BurnScorer` (8→16→1 MLP) на реальних текстах.

## Структура

```
experiments/teaching_loop/
├── README.md              ← цей файл
├── ingest_corpus.py       ← приймає .md/.txt → rust_nodes.json
├── proposer.py            ← LLM через z-ai-web-dev-sdk → candidate_nodes.json
├── comparator.py          ← diff rust vs llm → missing/extra/matched
├── auto_reviewer.py       ← heuristic approve/reject/partial → dataset.jsonl
├── train.rs               ← Burn training loop на dataset.jsonl → weights.json
├── run_pipeline.py        ← оркестратор: ingest → propose → compare → review
├── corpus/                ← вхідні тексти (симлінки/копії)
├── out/                   ← проміжні результати (rust_nodes.json, etc.)
├── dataset.jsonl          ← накопичувальний лог (append-only)
└── weights.json           ← поточний стан моделі
```

## Потік

```
corpus/*.md
   ↓ ingest_corpus.py
out/rust_nodes.json   (Rust fast path результат + 8 features per entity)
   ↓ proposer.py (паралельно)
out/candidate_nodes.json   (LLM розділяє на ноди)
   ↓ comparator.py
out/diff.json   (missing/extra/matched з features)
   ↓ auto_reviewer.py
dataset.jsonl   (append: {text_hash, rust, llm, diff, decision, features, label})
   ↓ train.rs (коли 50+ прикладів)
weights.json   ← нові weights від Burn
   ↓ (production: litgraph-core/src/scorer/ підвантажує)
refined confidence замінює hardcoded 0.3/0.7/1.0
```

## Використання

```bash
# 1. Покласти тексти в corpus/
ln -s /path/to/book1.md experiments/teaching_loop/corpus/
ln -s /path/to/book2.md experiments/teaching_loop/corpus/

# 2. Запустити повний цикл
cd experiments/teaching_loop
python3 run_pipeline.py corpus/ out/

# 3. Перевірити dataset.jsonl
wc -l dataset.jsonl
head -1 dataset.jsonl | python3 -m json.tool

# 4. Якщо 50+ прикладів — тренувати
cargo run --release --bin train_scorer -- \
    --dataset experiments/teaching_loop/dataset.jsonl \
    --weights experiments/teaching_loop/weights.json \
    --epochs 200

# 5. Скопіювати weights.json в production
cp experiments/teaching_loop/weights.json litgraph-core/data/scorer_weights.json
```

## Ручні правки

Після кожного циклу користувач може:

1. Переглянути `dataset.jsonl` — знайти помилкові рішення reviewer'а
2. Вручну змінити `decision` поле (approve → reject або навпаки)
3. Додати `comment` з поясненням
4. Перезапустити `train.rs` — Burn перетренується з оновленим dataset

## Provenance

- `rust_nodes.json` помічений `model: "rust-fast-path"`, `version: "2.3-step4"`
- `candidate_nodes.json` помічений `model: "llm-proposer"`, `version: "0.1"`
- `weights.json` помічений `architecture: "mlp_8_16_1_sigmoid"`
