
---
Task ID: phase2-plan-doc
Agent: Super Z (main)
Task: Сборка архитектурного документа Phase 2 + Teaching Loop + Burn Scorer

Work Log:
- Прочитал /home/z/my-project/skills/docx/SKILL.md + routes/create.md + references/design-system.md + references/common-rules.md
- Создал скрипт /home/z/my-project/scripts/teaching_loop_plan.js (переиспользовал helpers.js из предыдущего репорт-генератора)
- Cover: R1-style, DM-1 палитра (Deep Cyan, tech/AI), тёмный фон, левая композиция
- TOC: TableOfContents с 3 уровнями заголовков
- Body: 10 разделов (контекст, состояние, архитектура, 3 этапа, JSON-схемы, риски, метрики, структура файлов)
- Исправил баг: codeBlock() возвращает массив, забыл spread — добавил ... перед всеми 11 вызовами
- Запустил add_toc_placeholders.py --auto: добавил 42 закладки, outlineLvl, updateFields=true
- postcheck.py: 0 errors, 2 warnings (несущественные — line spacing в code blocks, Consolas font fallback)

Stage Summary:
- Документ: docs/architecture/LitGraph_Phase2_Teaching_Loop_Burn_Plan.docx (39 KB)
- 10 разделов, ~5000 слов, 8 таблиц, 11 code blocks
- Cover + TOC + 3 секции (cover margins 0, TOC roman numerals, body arabic)
- Не запушен — ожидает ревью пользователя и Claude
- Планируемый следующий коммит после ревью: либо "docs: add Phase 2 architectural plan" если всё ок, либо правки по замечаниям

---
Task ID: reasoning-engine-v0.7
Agent: Super Z (main)
Task: Создать Reasoning Engine — движок рассуждений (без LLM), объединяющий NER + POLER + SVO + scorer-веса + лингвистику (падежи), плюс диагностика ошибок алгоритма (underfitting, class imbalance, approve vs reject)

Work Log:
- Спроектировал 7-стадийный pipeline: Rust NER → Burn Scorer (weights.json) → SVO Parser → Case Validation → POLER ε_climax → Narrative Graph (Ω_conf) → Diagnostics
- Создал litgraph-core/src/scorer/inference.rs — pure-Rust MLP inference (потребляет WeightsFile без Burn runtime, 161 f32 = ~644 байт). Numerically stable sigmoid, Decision enum (Approve/Reject/Review с порогами 0.65/0.35)
- Создал litgraph-core/src/linguistic/case_validation.rs — валидация SVO через украинские падежи: Subject→Nominative, Object→Accusative (или Genitive под отрицанием), Instrument→Instrumental, Location→Locative. Invalid case → confidence × 0.3
- Создал litgraph-core/src/reasoning/diagnostics.rs — 5 детекторов ошибок:
  * ClassImbalanceReport: approve:reject ratio > 5:1 = severely imbalanced
  * ScoreDistribution: separation < 0.15 = underfitting detected
  * ScriptAnalysis: latin_fraction > 0.30 = parallel-text pollution
  * FeatureInformativeness: std=0.5 floored = constant feature (zero information)
  * WeightMagnitudeReport: fc1 std < 0.05 = collapse, max > 5.0 = explosion
- Создал litgraph-core/src/reasoning/engine.rs — главный ReasoningEngine + ReasoningReport + ScoredCharacter + ValidatedTriplet + EpsilonSummary. Полностью детерминированный, без LLM, без сети, без стохастики
- Добавил SvoParser::tag_text() публичный метод — возвращает tagged tokens без triplet extraction (для case validation без повторного тегирования)
- Создал litgraph-core/src/bin/reasoning_cli.rs — end-to-end CLI: --weights, --kappa, --json, human-readable output
- Добавил Serialize/Deserialize в ParsedCharacter (нужно для ScoredCharacter)
- Зарегистрировал reasoning_cli в Cargo.toml
- Компиляция: 5 warnings (unused imports в существующих файлах), 0 errors
- Тесты: 180 unit + 6 integration + 6 doc-tests — ВСЕ ПРОШЛИ

End-to-end demo (на реальных weights.json v0.2.0):
  Input: "Петро сказав Марті: йдемо у ліс. Веня відповів: добре."
  - 3 character candidates: Марті, Петро, Веня (все Cyrillic, без parallel-text pollution)
  - Все 3 REJECTED с refined=0.001 — модель выучила "всегда reject" из-за class imbalance
  - 4 SVO triplets: 3 invalid case (Марті в Dative вместо Accusative), 1 valid
  - ε_climax = 7.73 (climax detected)
  - Diagnostics: degraded health, ВСЕ 8 фич low-information (std=0.5 floored)
  - Recommendation: добавить case-aware фичи (Nominative_case_count, Accusative_case_count)

Stage Summary:
- Новые файлы (7):
  * litgraph-core/src/scorer/inference.rs (338 строк)
  * litgraph-core/src/linguistic/case_validation.rs (260 строк)
  * litgraph-core/src/reasoning/engine.rs (487 строк)
  * litgraph-core/src/reasoning/diagnostics.rs (660 строк)
  * litgraph-core/src/bin/reasoning_cli.rs (200 строк)
- Изменённые файлы (5):
  * litgraph-core/src/scorer/mod.rs (экспорт InferenceScorer, Decision)
  * litgraph-core/src/linguistic/mod.rs (экспорт case_validation)
  * litgraph-core/src/linguistic/svo_parser.rs (новый метод tag_text)
  * litgraph-core/src/reasoning/mod.rs (экспорт engine, diagnostics)
  * litgraph-core/src/parser/characters.rs (Serialize/Deserialize для ParsedCharacter)
  * litgraph-core/Cargo.toml (регистрация reasoning_cli binary)
- Архитектура: ReasoningEngine потребляет weights.json через InferenceScorer (pure-Rust, no Burn runtime). Burn остаётся только для обучения (train_scorer binary)
- Готов к Git commit + push


---
Task ID: wire-full-pipeline-to-ui
Agent: Super Z (main)
Task: Подключить новый ReasoningEngine v0.7+ (с Burn weights + case validation + diagnostics) к UI-кнопке Reasoning, чтобы пользователь видел результат работы движка а не только старый символьный цикл

Work Log:
- Прочитал API litgraph-core/src/reasoning/engine.rs: ReasoningEngine::with_weights_file() + analyze(text, kappa) → ReasoningReport
- Прочитал src-tauri/Cargo.toml — уже зависит от litgraph-core (path = "../litgraph-core"), никаких новых зависимостей не нужно
- Добавил новую Tauri-команду `reasoning_run_full_pipeline(text, kappa)` в src-tauri/src/commands/reasoning.rs:
  * include_str!("../../litgraph-core/data/scorer_weights.json") — веса вкомпилированы в бинарник (нет I/O, нет зависимости от CWD)
  * WeightsFile::from_json() → ReasoningEngine::with_weights_file() → engine.analyze(text, kappa)
  * kappa по умолчанию = 1.0, clamped до max(0.1, x) для защиты от деления на 0
  * 4 новых unit-теста: simple text, empty text, kappa=0.0, JSON serializability
- Зарегистрировал команду в src-tauri/src/lib.rs invoke_handler
- Добавил TS-биндинги в src/lib/tauri-commands.ts:
  * ReasoningReport, ScoredCharacter, ValidatedTriplet, EpsilonSummary, ConflictReport, DiagnosticsReport типы
  * reasoningRunFullPipeline(text, kappa?) async функция
- Обновил ReasoningDialog.tsx:
  * Mode switcher: 'Full Pipeline (v0.7+)' (default) vs 'Symbolic Engine (v0.1)'
  * Full Pipeline tab рендерит: scored characters (11 features visible), case-validated triplets, POLER ε_climax, Ω_conf, diagnostics block (class imbalance + score distribution + script analysis + weight magnitude + feature informativeness + recommendations), weights metadata
  * Symbolic tab оставлен без изменений — оба движка сосуществуют
- Создал litgraph-core/examples/verify_full_pipeline.rs — smoke test:
  * include_str! weights.json → ReasoningEngine::with_weights_file → analyze(sample Ukrainian text, 1.0)
  * Печатает полный ReasoningReport в читаемом виде
  * Sanity assertions: total_characters >= 1, decision tallies sum, 11 features, weights version non-empty
- Запустил verify_full_pipeline — PASS:
  * 3 character candidates (Веня, Петро, Марті) — все Cyrillic, parallel-text pollution НЕТ
  * Все 11 features извлекаются (включая nominative_case_norm=0.1)
  * Case validation: 1 valid, 3 invalid (Марті в Dative вместо Accusative)
  * ε_climax = 7.18, Ω_conf = 0.0 (без confirmed characters в graph)
  * Diagnostics: degraded (explosion_detected=true — fc1_max=6.07 > 5.0)
  * 2 low-info features: indices [2, 6] = has_direct_address + direct_count_norm
- Запустил тесты: 180 unit + 6 integration + 5 doc-tests — ВСЕ ПРОШЛИ
- TypeScript: tsc --noEmit — 0 errors
- Коммит 6bb615b запушен в origin/main

Stage Summary:
- Новые файлы (1):
  * litgraph-core/examples/verify_full_pipeline.rs (139 строк)
- Изменённые файлы (4):
  * src-tauri/src/commands/reasoning.rs (+112 строк: новая команда + 4 теста)
  * src-tauri/src/lib.rs (+2 строки: регистрация команды)
  * src/components/litgraph/ReasoningDialog.tsx (+473/-61: mode switcher + full pipeline render)
  * src/lib/tauri-commands.ts (+176 строк: TS типы + биндинг)
- Архитектура подтверждена: один движок, два фасада (CLI + UI). BurnScorer = генератор
  данных для обучения, не интегрируется в продакшн напрямую. Веса → материал для
  ReasoningEngine через include_str! в бинарник.
- Пользователь может теперь запустить `WEBKIT_DISABLE_DMABUF_RENDERER=1 GDK_BACKEND=x11 bun run tauri dev`,
  открыть Reasoning, и по умолчанию увидит вкладку Full Pipeline (v0.7+) с:
  * Scored characters (11 features видны, включая падежные)
  * Case-validated SVO triplets
  * POLER ε_climax и Ω_conf
  * Diagnostics (health = degraded, explosion detected)
  * Recommendations
- Старый символьный движок (Symbolic Engine v0.1) доступен через переключатель
