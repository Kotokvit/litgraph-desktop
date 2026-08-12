
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

