# LitGraph Desktop — Changelog

Все заметные изменения проекта документируются здесь.
Формат основан на [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/),
версионирование — [SemVer](https://semver.org/lang/ru/).

## [Unreleased]

### Добавлено
- Скелет Tauri 2 проекта (React + TypeScript + Rust)
- Структура директорий: `src-tauri/{commands,parser,models,storage,ai}`
- Заглушки Tauri commands: parse_md, list_projects, load_project, save_project, delete_project, save_version, restore_version, delete_version, list_versions, export_project, ai_assistant, ai_continue_chapter, ai_analyze_plot, ai_test_connection, ai_list_ollama_models
- Типы данных на Rust (LitNode, LitEdge, Project, ChapterVersion, AiProvider, ChatMessage) — зеркало types.ts
- Структура AI-провайдеров: Ollama, OpenAI-compat, Z.ai
- Заглушки парсера .md (chapters, characters, locations, themes)
- Промпт-план разработки (docs/PROMPT_PLAN.md)
- Скриншоты прототипа (docs/screenshots/)
- GitHub Actions workflow для сборки .deb/.AppImage при релизе
- README.md с инструкциями по сборке из исходников

### В разработке
- Перенос 11 React-компонентов из прототипа (этап 1)
- Реализация автопарсера .md на Rust (этап 4)
- Реализация хранения проектов (этап 5)
- Реализация версионирования (этап 6)
- Реализация AI-функций через Ollama (этап 7)
- Реализация OpenAI-compat провайдера (этап 8)
- Реализация экспорта (этап 9)
- Финальная сборка .deb/.AppImage (этап 10)

## [0.1.0] — TODO

Первый релиз. Критерии готовности — см. docs/PROMPT_PLAN.md раздел 8.
