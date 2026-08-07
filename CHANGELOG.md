# Changelog

## [0.2.0] — 2026-08-07

### Добавлено
- **Полная реализация Rust-парсера .md** (`src-tauri/src/parser/`):
  - `chapters.rs` — детекция глав по 9 паттернам (Глава/Розділ/Частина/Chapter/Part/Часть/#/##/###)
  - `characters.rs` — детекция персонажей (capitalized слова с частотой 5+, стоп-слово на 3 языках, группировка падежей по 4-символьному префиксу)
  - `locations.rs` — детекция локаций (capitalized слова после предлогов места)
  - `themes.rs` — детекция тем/мотивов (словарь ~80 слов на 3 языках)
  - `mod.rs` — сборка графа + раскладка (главы в центре, темы слева, персонажи/локации справа)
- **Полная реализация storage** (`src-tauri/src/storage/`):
  - `list_projects()` — список файлов в `~/.local/share/litgraph/projects/*.litgraph`
  - `load_project(id)` — чтение JSON
  - `save_project(id, project)` — запись с автосозданием директорий
  - `delete_project(id)` — удаление
  - Защита от path traversal
- **Полная реализация версионирования** (`src-tauri/src/commands/versions.rs`):
  - `save_version` — сохранение текущего fullText как версии (макс. 50 на ноду)
  - `restore_version` — восстановление с автосохранением текущего состояния как "перед откатом"
  - `delete_version` — удаление конкретной версии
  - `list_versions` — список версий ноды
- **Полная реализация AI-функций** (`src-tauri/src/ai/`):
  - `prompts.rs` — промпты для 3 функций (помощник, дописать главу, анализ сюжета)
  - `ollama.rs` — провайдер Ollama (локальные модели, бесплатно)
  - `openai_compat.rs` — провайдер OpenAI-совместимый (OpenAI, Groq, OpenRouter, LiteLLM, vLLM, Z.ai)
  - `types.rs` — общие типы (ChatMessage, AiResponse, AiProvider)
- **Полная реализация экспорта** (`src-tauri/src/commands/export.rs`):
  - JSON, текст, Markdown
- **Перенос фронтенда из прототипа** (`src/`):
  - 11 компонентов из `src/components/litgraph/` (LitApp, LitCanvas, LitNodeView, LitEdgeView, Toolbar, Sidebar, Inspector, NodePalette, NodeEditor, AIDialog, AssistantDialog)
  - 47 shadcn/ui компонентов
  - lib/litgraph (types, store, export) — с адаптацией под Tauri
  - Замена `fetch('/api/...')` на `invoke('...')` во всех AI-функциях
  - `downloadFile` теперь использует Tauri save dialog
- **Компонент настроек AI** (`AiSettingsDialog.tsx`):
  - Выбор провайдера: Ollama / OpenAI-compat / Z.ai
  - Для Ollama: URL + модель (с авто-загрузкой списка моделей)
  - Для OpenAI-compat: endpoint + API key + model
  - Для Z.ai: API key + model
  - Проверка соединения
  - Сохранение в Tauri store (`config.json`)
- **Кнопка "Настройки AI"** в меню AI тулбара
- **TypeScript-компиляция проходит без ошибок**
- **Vite build успешен** (1969 модулей, 1.4 МБ JS / 352 КБ gzip)

### Изменено
- `lib.rs` — убран неиспользуемый `app.store()`, добавлена директория `backups/`
- `models/edge.rs` — `EdgeKind` теперь `type EdgeKind = String` (для совместимости с TS-форматом)
- `store.ts` — `createJSONStorage` явно указывает localStorage (для SSR-safety)
- `export.ts` — `downloadFile` теперь async, использует Tauri save dialog с fallback на браузерный метод

### Удалено
- `src/components/ui/calendar.tsx` — не используется, вызывал конфликт типов с react-day-picker

## [0.1.0] — 2026-08-07

### Добавлено
- Скелет Tauri 2 проекта (React + TypeScript + Rust)
- Структура директорий: `src-tauri/{commands,parser,models,storage,ai}`
- Заглушки Tauri commands (15 шт)
- Типы данных на Rust (LitNode, LitEdge, Project, ChapterVersion, AiProvider, ChatMessage)
- Промпт-план разработки (docs/PROMPT_PLAN.md)
- Скриншоты прототипа (docs/screenshots/)
- GitHub Actions workflow для сборки .deb/.AppImage при релизе
- README.md с инструкциями по сборке из исходников
