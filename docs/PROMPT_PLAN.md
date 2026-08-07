# Промпт-план: Десктопная версия ЛитоГрафа (LitGraph Desktop)

> Этот документ — готовый промпт для AI-помощника (Claude, GPT, Gemini, Z.ai, Cursor, и т.д.).
> Скопируй его целиком и вставь в новую сессию разработки. AI получит весь контекст.

---

## 0. TL;DR для AI

Создать **десктопное приложение для Linux** на основе существующего веб-прототипа «ЛитоГраф» — нодового редактора для работы с литературным текстом. Стек: **Tauri 2 + React + TypeScript + Rust**. Фронтенд переносится из прототипа почти без изменений, бэкенд переписывается с Next.js API routes на Rust-команды Tauri. AI-функции реализуются через **подключаемый провайдер** (Ollama по умолчанию, Z.ai SDK или пользовательский OpenAI-совместимый ключ — на выбор). Финальная сборка — `.deb` + `.AppImage` через `tauri build`.

---

## 1. Контекст: что уже есть

### 1.1. Превью-версия (веб)
Рабочий прототип на Next.js 16 + React 19 + TypeScript + React Flow + Zustand + Tailwind CSS + shadcn/ui. ~5000 строк кода, 19 исходных файлов. Полностью функционален: нодовый холст, 9 типов нод, 9 типов связей, focus-режим, версионирование, 3 AI-функции, автопарсер .md.

Превью остаётся как инструмент для совместной работы с AI (в песочнице) — десктопная версия отдельная, не зависит от песочницы.

### 1.2. Структура файлов прототипа (что переносим)

```
src/
├── lib/litgraph/
│   ├── types.ts          (344 строки) — типы нод, связей, версий. ПЕРЕНОСИМ БЕЗ ИЗМЕНЕНИЙ
│   ├── store.ts          (556 строк) — Zustand стор с persist. ПЕРЕНОСИМ, заменяем localStorage на Tauri store
│   └── export.ts         (214 строк) — экспорт в текст/Markdown. ПЕРЕНОСИМ БЕЗ ИЗМЕНЕНИЙ
├── components/litgraph/
│   ├── LitApp.tsx        (19 строк) — корневой layout. ПЕРЕНОСИМ
│   ├── LitCanvas.tsx     (241 строк) — React Flow холст + focus-режим. ПЕРЕНОСИМ
│   ├── LitNodeView.tsx   (138 строк) — кастомная нода. ПЕРЕНОСИМ
│   ├── LitEdgeView.tsx   (82 строки) — кастомное ребро. ПЕРЕНОСИМ
│   ├── Toolbar.tsx       (553 строки) — верхняя панель. ПЕРЕНОСИМ, заменяем fetch→invoke
│   ├── Sidebar.tsx       (219 строк) — правый сайдбар. ПЕРЕНОСИМ
│   ├── Inspector.tsx     (325 строк) — инспектор ноды. ПЕРЕНОСИМ
│   ├── NodePalette.tsx   (73 строки) — палитра нод. ПЕРЕНОСИМ
│   ├── NodeEditor.tsx    (416 строк) — модалка редактора + версии. ПЕРЕНОСИМ
│   ├── AIDialog.tsx      (301 строк) — модалка AI-генерации. ПЕРЕНОСИМ, заменяем fetch→invoke
│   └── AssistantDialog.tsx (257 строк) — чат с AI. ПЕРЕНОСИМ, заменяем fetch→invoke
└── app/api/              (4 файла, ~1300 строк) — API routes. ПЕРЕПИСЫВАЕМ НА RUST
    ├── parse-md/route.ts        (705 строк) — автопарсер .md → граф
    ├── ai/assistant/route.ts    (216 строк) — AI-помощник (чат)
    ├── ai/continue-chapter/route.ts (177 строк) — дописать главу
    └── ai/analyze-plot/route.ts (210 строк) — анализ сюжета
```

### 1.3. Функции прототипа (что должно работать в десктопе)

**Нодовый редактор:**
- 9 типов нод: Глава, Сцена, Сюжетная точка, Конфликт, Персонаж, Диалог, Локация, Тема, Идея
- 9 типов связей: поток сюжета, причина-следствие, участие персонажа, место действия, упоминание, конфликт, предзнаменование, альтернативная ветка, тема
- Создание/редактирование/удаление нод и связей
- Focus-режим: при клике на ноду остальные затемняются (видна только выбранная + соседи)
- Поиск по нодам, фильтр по тегам
- Мини-карта, контролы зума, авто-раскладка
- Горячие клавиши: Del (удалить), Ctrl+D (дублировать)

**Контент:**
- Полный текст главы в редакторе (для нод «глава» и «сцена»)
- Версионирование: сохранение/восстановление/удаление версий fullText (макс. 50 на ноду)
- Теги у нод
- Метаданные: POV, настроение, время суток, дуга персонажа, важность, и т.д.

**Импорт/экспорт:**
- Автопарсер .md → граф (детекция глав по 9 паттернам, персонажей по частоте, локаций по предлогам, тем по словарю ~80 слов на 3 языках)
- Импорт/экспорт JSON (формат .litgraph.json)
- Экспорт в текст и Markdown

**AI-функции (3 шт.):**
1. AI-помощник — чат с контекстом графа, видит все ноды и связи, учитывает выбранную ноду
2. Дописать главу — берёт последние 2-3 главы как контекст, генерирует продолжение
3. Анализ сюжета — находит слабые места: недоразвитые персонажи, одинокие главы, выбросы по объёму

**Хранение:**
- Автосохранение в localStorage (в прототипе) → в Tauri-версии заменить на файлы в `~/.local/share/litgraph/projects/`

---

## 2. Целевая архитектура

### 2.1. Стек

| Слой | Технология | Почему |
|------|-----------|--------|
| Desktop-обёртка | **Tauri 2** | Rust backend + веб-фронтенд в одном бинарнике. Нативные пакеты .deb/.AppImage из коробки. Маленький размер (~10 МБ против ~150 МБ у Electron) |
| Frontend | **React 19 + TypeScript + Vite** | Переносим из прототипа без изменений. Vite вместо Next.js — Tauri не нуждается в SSR |
| Нодовый холст | **@xyflow/react** (React Flow) | Тот же, что в прототипе |
| State | **Zustand + persist** | Тот же, что в прототипе. Меняем только storage engine |
| Стили | **Tailwind CSS 4 + shadcn/ui** | Переносим из прототипа |
| Backend | **Rust** | Парсер .md, версионирование, работа с файлами, AI-провайдеры |
| AI | **Подключаемый провайдер** | См. раздел 4 |
| Сборка | `tauri build` | Генерирует .deb, .AppImage, .rpm |

### 2.2. Структура десктоп-проекта

```
litgraph-desktop/
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json           # конфиг сборки, иконки, bundle
│   ├── build.rs
│   ├── icons/                    # иконки приложения (512x512, .icns, .ico)
│   └── src/
│       ├── main.rs               # точка входа Tauri
│       ├── lib.rs
│       ├── commands/             # Tauri commands (вызываются из JS через invoke)
│       │   ├── mod.rs
│       │   ├── parse_md.rs       # автопарсер .md → граф (переписать с TS)
│       │   ├── project.rs        # загрузка/сохранение проектов
│       │   ├── versions.rs       # версионирование глав
│       │   └── export.rs         # экспорт в текст/Markdown
│       ├── ai/                   # AI-провайдеры
│       │   ├── mod.rs
│       │   ├── ollama.rs         # локальные модели через Ollama
│       │   ├── openai_compat.rs  # OpenAI-совместимый API (любой провайдер)
│       │   ├── zai.rs            # Z.ai SDK (опционально)
│       │   └── types.rs          # общие типы (ChatMessage, ChatResponse)
│       ├── parser/               # парсер .md (ядро логики)
│       │   ├── mod.rs
│       │   ├── chapters.rs       # детекция глав
│       │   ├── characters.rs     # детекция персонажей
│       │   ├── locations.rs      # детекция локаций
│       │   └── themes.rs         # детекция тем
│       ├── models/               # типы данных (соответствие TS types.ts)
│       │   ├── mod.rs
│       │   ├── node.rs
│       │   ├── edge.rs
│       │   └── project.rs
│       └── storage/              # работа с файловой системой
│           ├── mod.rs
│           └── projects.rs       # CRUD проектов в ~/.local/share/litgraph/
├── src/                          # React frontend (переносим из прототипа)
│   ├── main.tsx                  # точка входа Vite
│   ├── App.tsx                   # корневой компонент
│   ├── lib/litgraph/             # ПЕРЕНОСИМ БЕЗ ИЗМЕНЕНИЙ из прототипа
│   │   ├── types.ts
│   │   ├── store.ts              # меняем persist storage на Tauri store plugin
│   │   └── export.ts
│   ├── components/litgraph/      # ПЕРЕНОСИМ, заменяем fetch→invoke
│   │   └── (все 11 компонентов)
│   └── components/ui/            # shadcn/ui компоненты
├── package.json
├── vite.config.ts
├── tailwind.config.ts
├── tsconfig.json
└── README.md
```

### 2.3. Поток данных

```
Пользователь кликает "Импорт .md"
  ↓
React (Toolbar.tsx) вызывает invoke('parse_md', { markdown, title, author })
  ↓
Tauri роутит вызов в Rust-функцию parse_md в commands/parse_md.rs
  ↓
Rust парсит .md (модули parser/*), возвращает GraphJSON
  ↓
React получает GraphJSON, загружает в Zustand store
  ↓
Zustand persist пишет в Tauri store plugin (→ ~/.local/share/litgraph/)
```

---

## 3. Что переписать на Rust

### 3.1. Парсер .md (`src-tauri/src/parser/`)

Переписать логику из `src/app/api/parse-md/route.ts` (705 строк TS). Сохранить все алгоритмы:

**Детекция глав** (`chapters.rs`):
- 9 паттернов регэкспов: `Глава N`, `Розділ N`, `Частина N`, `Chapter N`, `Part N`, `Часть N`, `# N`, `## N`, `### N`
- Выбрать паттерн с максимальным числом совпадений
- Уникальные по номеру (первое вхождение)
- Пролог = текст до первой главы
- Извлечение заголовка: после "Глава N" берём текст до `\n` или до точки, чистим от меток "(Робоча назва)", "(Виправлена версія)", "Місце дії:" и т.д.

**Детекция персонажей** (`characters.rs`):
- Регэксп для capitalized слов: `(?<![a-zA-Z\u0400-\u04FF])([А-ЯЁA-Z][а-яёa-z\u0400-\u04FF]{2,})(?![a-zA-Z\u0400-\u04FF])`
- ВАЖНО: `\b` не работает с кириллицей в JS, используем lookbehind/lookahead. В Rust через `regex` или `fancy-regex` crate (fancy-regex поддерживает lookaround)
- Фильтр стоп-слов (~200 слов на 3 языках: укр, рус, англ) — перенести список из TS
- Группировка падежей по 4-символьному префиксу (Ліана + Ліани + Ліану → одна группа)
- Минимум 5 упоминаний, топ-25

**Детекция локаций** (`locations.rs`):
- Регэксп: `(?<![...])(?:у|в|на|біля|під|над|за|до|із|від|через|крізь|около|под|возле|перед|in|at|on|near|under|over|behind|from|through)\s+([Capitalized]{3,})(?![...])`
- Группировка по префиксу, минимум 3 упоминания, топ-15

**Детекция тем** (`themes.rs`):
- Словарь ~80 тематических существительных на 3 языках (тиша, память, тень, голос, страх, надежда, любовь, предательство, одиночество, судьба, свобода, выбор, правда, ложь, война, смерть, жизнь, кровь, огонь, вода, время, вечность, мгновение, боль, печаль, радость, гнев, нежность, детство, взросление, вина, искупление, шёпот, слово, бездна, мрак + английские аналоги)
- Минимум 5 упоминаний в тексте → создаётся нода-тема
- Тема связывается с главой, если ключевое слово встречается в ней ≥2 раз

**Сборка графа** (`mod.rs`):
- Создать ноды: пролог, главы (с fullText), персонажи, локации, темы
- Связи: поток глав (flow), персонаж→глава (character, если ≥3 упоминаний), локация→глава (location, если ≥2), тема→глава (theme, если ≥2)
- Раскладка: главы в центральной колонке (x=600, y=60+i*130), темы слева (x=200), персонажи справа (x=1100), локации ещё правее (x=1500)

**Крейты:** `regex` или `fancy-regex` (для lookaround), `serde` + `serde_json` (типы), `unicode-segmentation` (корректная работа с юникодом).

### 3.2. Хранение проектов (`src-tauri/src/storage/projects.rs`)

Заменить localStorage на файлы:
```
~/.local/share/litgraph/
├── config.json              # настройки (AI-провайдер, тема оформления, и т.д.)
├── projects/
│   ├── kasiopia.litgraph    # JSON-файл проекта
│   ├── my-novel.litgraph
│   └── ...
└── backups/                 # автобэкапы
```

Tauri commands:
- `list_projects() → Vec<ProjectMeta>` — список проектов (имя, дата изменения, размер)
- `load_project(id: String) → Project` — загрузить проект
- `save_project(id: String, project: Project) → ()` — сохранить
- `delete_project(id: String) → ()` — удалить
- `export_project(id: String, format: "json"|"text"|"markdown", path: String) → ()` — экспорт в файл

### 3.3. Версионирование (`src-tauri/src/commands/versions.rs`)

Можно оставить в Zustand (как в прототипе) — версии хранятся в `data.versions` ноды. Но для больших проектов лучше вынести в Rust:
- `save_version(node_id: String, label: String, source: String) → Version`
- `restore_version(node_id: String, version_id: String) → ()` (с автосохранением текущего состояния как "перед откатом")
- `delete_version(node_id: String, version_id: String) → ()`
- `list_versions(node_id: String) → Vec<Version>`

Лимит: 50 версий на ноду (как в прототипе).

### 3.4. AI-провайдеры (`src-tauri/src/ai/`)

См. раздел 4 ниже.

---

## 4. AI-интеграция (главное требование)

**Цель:** сохранить AI-функции (помощник, дописать главу, анализ сюжета) без оплаты за API.

### 4.1. Три варианта провайдера (пользователь выбирает в настройках)

#### Вариант A: Ollama (локально, бесплатно, по умолчанию)

Пользователь ставит [Ollama](https://ollama.com) на свой Linux, тянет модель (например `ollama pull llama3.1` или `qwen2.5`), приложение общается с локальным сервером `http://localhost:11434`.

Плюсы: бесплатно, приватно, работает офлайн.
Минусы: нужно ставить Ollama, качество моделей зависит от железа (нужно 8+ ГБ RAM для 7B моделей).

Rust-реализация (`ai/ollama.rs`):
```rust
use reqwest::Client;

pub async fn chat(model: String, messages: Vec<ChatMessage>) -> Result<String, AiError> {
    let client = Client::new();
    let resp = client
        .post("http://localhost:11434/api/chat")
        .json(&serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false
        }))
        .send().await?
        .json::<serde_json::Value>().await?;
    Ok(resp["message"]["content"].as_str().unwrap_or("").to_string())
}
```

#### Вариант B: Z.ai SDK (бесплатно через песочницу, опционально)

Если пользователь хочет интеграцию с Z.ai (как в превью), используем `z-ai-web-dev-sdk`. Но это Node.js SDK — в Tauri его можно подключить через sidecar (отдельный Node-процесс) или через HTTP-вызовы к API Z.ai.

**Рекомендация:** реализовать как OpenAI-совместимый провайдер (вариант C) с предустановкой endpoint Z.ai. Так проще.

#### Вариант C: OpenAI-совместимый API (пользовательский ключ)

Пользователь указывает endpoint + API-ключ в настройках. Работает с OpenAI, Anthropic (через прокси), Together AI, Groq, OpenRouter, Z.ai (если есть совместимый endpoint), любым self-hosted LLM сервером (LiteLLM, vLLM, и т.д.).

Rust-реализация (`ai/openai_compat.rs`):
```rust
pub struct OpenAiCompat {
    endpoint: String,    // https://api.openai.com/v1
    api_key: String,
    model: String,       // gpt-4o-mini, llama-3.1-70b, и т.д.
}

impl OpenAiCompat {
    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, AiError> {
        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/chat/completions", self.endpoint))
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "model": self.model,
                "messages": messages
            }))
            .send().await?
            .json::<serde_json::Value>().await?;
        Ok(resp["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string())
    }
}
```

### 4.2. Настройки AI (UI)

В тулбаре добавить кнопку **«AI Настройки»** → модалка:
- Радио: Ollama (локально) / OpenAI-совместимый API / Z.ai
- Для Ollama: поле "URL сервера" (по умолчанию `http://localhost:11434`), выпадающий список моделей (запрашивается через `/api/tags`), кнопка "Проверить соединение"
- Для OpenAI-совместимого: поля Endpoint, API Key, Model. Кнопка "Проверить"
- Для Z.ai: инструкция как получить доступ, поле токена
- Сохранение в `~/.local/share/litgraph/config.json`

### 4.3. Промпты AI-функций (перенести из прототипа)

Все промпты уже написаны в `src/app/api/ai/*/route.ts` — переносим в Rust без изменений:

1. **AI-помощник** (`ai/assistant.rs`):
   - Системный промпт: «литературный редактор, соавтор и аналитик»
   - В контексте: структура графа (главы, персонажи топ-10, локации, темы, конфликты, сюжетные точки, конспект глав)
   - Если выбрана нода — добавляется её fullText (до 4000 символов)
   - История диалога: последние 6 сообщений

2. **Дописать главу** (`ai/continue_chapter.rs`):
   - Берёт последние 2-3 главы (fullText, обрезанные до 3000 символов)
   - Извлекает активных персонажей и локации по рёбрам графа
   - Извлекает сюжетные точки с cause-связями
   - Опционально: customPrompt от пользователя
   - Возвращает текст новой главы

3. **Анализ сюжета** (`ai/analyze_plot.rs`):
   - Метрики: средний/мин/макс объём глав, недоразвитые персонажи (топ-5 по min связей), одинокие главы без персонажей, слишком короткие/длинные (выбросы от среднего)
   - 4 фокуса: полный / сюжет / персонажи / темп
   - Структурированный отчёт: 🟢 Сильные / 🔴 Слабые / 💡 Рекомендации / ⚠️ Нестыковки

### 4.4. Tauri commands для AI

```rust
#[tauri::command]
async fn ai_assistant(project: Project, message: String, history: Vec<ChatMessage>, selected_node_id: Option<String>) -> Result<AiResponse, String>;

#[tauri::command]
async fn ai_continue_chapter(project: Project, from_chapter_id: Option<String>, custom_prompt: Option<String>) -> Result<AiResponse, String>;

#[tauri::command]
async fn ai_analyze_plot(project: Project, focus: String) -> Result<AiResponse, String>;

#[tauri::command]
async fn ai_test_connection(provider: AiProvider) -> Result<bool, String>;

#[tauri::command]
async fn ai_list_ollama_models(url: String) -> Result<Vec<String>, String>;
```

---

## 5. Что перенести из прототипа без изменений

### 5.1. Frontend (React)

Все 11 компонентов из `src/components/litgraph/` переносятся с минимальными правками:

| Файл | Что менять |
|------|-----------|
| `types.ts` | Ничего. Переносим 1:1 |
| `store.ts` | Заменить `persist` storage с localStorage на Tauri Store plugin (`@tauri-apps/plugin-store`). Методы те же |
| `export.ts` | Ничего. Переносим 1:1 (функция `downloadFile` работает в Tauri через `@tauri-apps/plugin-dialog`) |
| `LitApp.tsx`, `LitCanvas.tsx`, `LitNodeView.tsx`, `LitEdgeView.tsx`, `Sidebar.tsx`, `Inspector.tsx`, `NodePalette.tsx`, `NodeEditor.tsx` | Переносим 1:1 |
| `Toolbar.tsx` | Заменить `fetch('/api/parse-md', ...)` → `invoke('parse_md', { markdown, projectTitle, author })`. Заменить `fetch('/api/ai/...')` → `invoke('ai_...')`. Добавить кнопку "AI Настройки" |
| `AIDialog.tsx`, `AssistantDialog.tsx` | Заменить `fetch` → `invoke`. Остальное без изменений |

### 5.2. shadcn/ui компоненты

Все 50+ компонентов из `src/components/ui/` (button, dialog, dropdown-menu, textarea, и т.д.) переносятся 1:1. Это просто обёртки над Radix UI — работают в любом React-окружении.

### 5.3. Стили

`globals.css`, `tailwind.config.ts` — переносим без изменений. Tailwind 4 работает в Vite через `@tailwindcss/vite` plugin.

### 5.4. Иконки

`lucide-react` — работает в любом React-окружении.

---

## 6. Пошаговый план разработки

### Этап 0: Подготовка (1 день)

- [ ] Установить Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- [ ] Установить Tauri CLI: `cargo install tauri-cli --version "^2"`
- [ ] Установить системные зависимости для Linux: `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`
- [ ] Создать проект: `npm create tauri-app@latest litgraph-desktop -- --template react-ts`
- [ ] Проверить что `cargo tauri dev` запускает пустое окно

### Этап 1: Перенос фронтенда (2-3 дня)

- [ ] Скопировать `src/lib/litgraph/` (3 файла) из прототипа
- [ ] Скопировать `src/components/litgraph/` (11 файлов) из прототипа
- [ ] Скопировать `src/components/ui/` (50+ shadcn компонентов) из прототипа
- [ ] Скопировать `globals.css`, `tailwind.config.ts`
- [ ] Установить npm-зависимости: `@xyflow/react`, `zustand`, `lucide-react`, `class-variance-authority`, `clsx`, `tailwind-merge`, `cmdk`, `react-hook-form`, `zod`, `react-markdown`, и т.д. (см. package.json прототипа)
- [ ] В `App.tsx` рендерить `<LitApp />`
- [ ] **Проверка:** запустить `cargo tauri dev` — должно открыться окно с нодовым редактором, работают создание/перемещение/удаление нод. Демо-данные загружаются (из дефолтного проекта в store.ts)

### Этап 2: Замена persist storage (1 день)

- [ ] Установить `@tauri-apps/plugin-store`
- [ ] В `store.ts` заменить:
  ```ts
  // Было (localStorage):
  persist(storeCreator, { name: "litgraph-store-v1", ... })
  
  // Стало (Tauri store):
  persist(storeCreator, {
    name: "litgraph-store-v1",
    storage: createJSONStorage(() => tauriStore),  // tauriStore из plugin-store
    ...
  })
  ```
- [ ] **Проверка:** создать ноду, закрыть приложение, открыть — нода на месте

### Этап 3: Rust — типы и модели (1 день)

- [ ] Создать `src-tauri/src/models/` с типами, зеркалящими `types.ts`:
  ```rust
  #[derive(Serialize, Deserialize, Clone)]
  pub struct LitNode {
      pub id: String,
      pub r#type: String,  // "chapter" | "scene" | ...
      pub position: Position,
      pub data: LitNodeData,
  }
  
  #[derive(Serialize, Deserialize, Clone)]
  pub struct LitNodeData {
      pub title: String,
      pub body: String,
      pub r#type: String,
      pub tags: Vec<String>,
      pub meta: Option<serde_json::Value>,
      pub full_text: Option<String>,
      pub versions: Option<Vec<ChapterVersion>>,
  }
  // ... и т.д. для всех типов из types.ts
  ```
- [ ] **Проверка:** `cargo check` проходит

### Этап 4: Rust — парсер .md (3-4 дня, самый сложный этап)

- [ ] Установить крейты: `fancy-regex` (для lookaround), `unicode-segmentation`
- [ ] Реализовать `parser/chapters.rs` — детекция глав по 9 паттернам
- [ ] Реализовать `parser/characters.rs` — capitalized слова + стоп-слово + группировка
- [ ] Реализовать `parser/locations.rs` — предлоги места
- [ ] Реализовать `parser/themes.rs` — словарь ~80 слов
- [ ] Реализовать `parser/mod.rs` — сборка графа + раскладка
- [ ] Создать Tauri command `parse_md` в `commands/parse_md.rs`
- [ ] В `Toolbar.tsx` заменить `fetch('/api/parse-md')` → `invoke('parse_md', ...)`
- [ ] **Проверка:** загрузить `1-Касіопея Редактированое канон.md` (1.1 МБ), должно получиться ~110 нод, ~730 связей, 10 тем — как в прототипе

### Этап 5: Rust — хранение проектов (2 дня)

- [ ] Реализовать `storage/projects.rs`:
  - `list_projects()` — список файлов в `~/.local/share/litgraph/projects/`
  - `load_project(id)` — чтение JSON
  - `save_project(id, project)` — запись JSON
  - `delete_project(id)` — удаление
- [ ] Добавить Tauri commands для каждого метода
- [ ] В UI добавить диалог "Открыть проект" (список сохранённых проектов)
- [ ] В UI добавить "Сохранить как..." (создать новый проект)
- [ ] **Проверка:** создать 3 проекта, переключаться между ними, перезапустить приложение — проекты на месте

### Этап 6: Rust — версионирование (1 день)

- [ ] Реализовать `commands/versions.rs` (save_version, restore_version, delete_version, list_versions)
- [ ] В `NodeEditor.tsx` заменить вызовы store на invoke (или оставить в store — см. решение в этапе 3)
- [ ] **Проверка:** сохранить 3 версии, откатиться к первой, проверить что появилась 4-я "перед откатом"

### Этап 7: AI — Ollama провайдер (2 дня)

- [ ] Реализовать `ai/ollama.rs` (chat, list_models, test_connection)
- [ ] Реализовать `ai/types.rs` (ChatMessage, AiResponse, AiProvider)
- [ ] Реализовать `ai/assistant.rs` (промпт из `assistant/route.ts`)
- [ ] Реализовать `ai/continue_chapter.rs` (промпт из `continue-chapter/route.ts`)
- [ ] Реализовать `ai/analyze_plot.rs` (промпт из `analyze-plot/route.ts`)
- [ ] Создать Tauri commands: `ai_assistant`, `ai_continue_chapter`, `ai_analyze_plot`, `ai_test_connection`, `ai_list_ollama_models`
- [ ] В `AIDialog.tsx` и `AssistantDialog.tsx` заменить `fetch` → `invoke`
- [ ] Добавить модалку "AI Настройки" с выбором провайдера
- [ ] **Проверка:** запустить Ollama локально (`ollama pull llama3.1 && ollama serve`), в настройках выбрать Ollama, нажать "Проверить" → должно показать список моделей. Открыть AI-помощник, задать вопрос → получить ответ.

### Этап 8: AI — OpenAI-совместимый провайдер (1 день)

- [ ] Реализовать `ai/openai_compat.rs` (chat с произвольным endpoint + api_key)
- [ ] Добавить UI для ввода endpoint/api_key/model
- [ ] **Проверка:** ввести ключ OpenAI (или Groq, или OpenRouter), проверить соединение, задать вопрос помощнику

### Этап 9: Экспорт/импорт (1 день)

- [ ] Реализовать `commands/export.rs` (экспорт в JSON/текст/Markdown через `tauri-plugin-dialog` для выбора файла)
- [ ] В `export.ts` заменить `downloadFile` (создание `<a>` в DOM) на `save` из `@tauri-apps/plugin-dialog`
- [ ] **Проверка:** экспортировать проект в .txt, открыть в редакторе — корректный формат

### Этап 10: Сборка и упаковка (1 день)

- [ ] Настроить `tauri.conf.json`:
  ```json
  {
    "productName": "LitGraph",
    "version": "0.1.0",
    "identifier": "com.litgraph.desktop",
    "build": {
      "frontendDist": "../dist"
    },
    "bundle": {
      "active": true,
      "targets": ["deb", "appimage", "rpm"],
      "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.icns"]
    }
  }
  ```
- [ ] Создать иконки приложения (512x512 PNG, .icns для Linux, .ico для Windows)
- [ ] Запустить `cargo tauri build`
- [ ] **Проверка:** в `src-tauri/target/release/bundle/` появились `.deb` и `.AppImage`. Установить `.deb` через `sudo dpkg -i litgraph_0.1.0_amd64.deb`, запустить `litgraph` из меню приложений.

### Этап 11: Полировка (2-3 дня)

- [ ] Тёмная тема (next-themes уже в прототипе)
- [ ] Горячие клавиши: Ctrl+O (открыть), Ctrl+S (сохранить), Ctrl+N (новый)
- [ ] Системный трей (опционально, через `tauri-plugin-system-tray`)
- [ ] Автопроверка обновлений через GitHub Releases (опционально)
- [ ] Локализация (укр/рус/англ) — i18next
- [ ] README.md с инструкцией сборки из исходников

### Этап 12: GitHub Actions CI (1 день)

- [ ] Создать `.github/workflows/release.yml`:
  ```yaml
  on:
    push:
      tags: ['v*']
  jobs:
    build-linux:
      runs-on: ubuntu-22.04
      steps:
        - uses: actions/checkout@v4
        - uses: actions/setup-node@v4
          with: { node-version: 20 }
        - uses: dtolnay/rust-toolchain@stable
        - run: sudo apt update && sudo apt install -y libwebkit2gtk-4.1-dev libssl-dev librsvg2-dev
        - run: npm ci
        - run: cargo tauri build
        - uses: softprops/action-gh-release@v1
          with:
            files: src-tauri/target/release/bundle/**/*.deb,src-tauri/target/release/bundle/**/*.AppImage
  ```
- [ ] Создать тег `v0.1.0`, запушить — GitHub Actions соберёт .deb и .AppImage и прикрепит к Release

**Итого: ~15-18 дней разработки** (можно ускорить, если AI генерирует Rust-код по TS-исходникам).

---

## 7. Ключевые технические решения

### 7.1. Почему Tauri, а не чистый Rust GUI

- Фронтенд уже написан на React + React Flow — переписывать UI на egui/iced/slint = потерять всю работу
- Tauri позволяет использовать веб-фронтенд 1:1, добавляя Rust только для тяжёлой логики
- Размер бинарника ~10 МБ (против ~150 МБ у Electron)
- Нативные пакеты .deb/.AppImage из коробки через `tauri build`

### 7.2. Почему не переписывать UI на Rust

Rust не предназначен для DOM-манипуляций. React Flow (нодовый холст) — это 50k+ строк JS-кода с оптимизациями рендеринга. Переписать на egui/iced = месяцы работы и худший результат. UI остаётся на React, Rust берёт только бэкенд.

### 7.3. Почему Ollama по умолчанию

- Бесплатно, работает офлайн, приватно
- На Linux ставится в 1 команду: `curl -fsSL https://ollama.com/install.sh | sh`
- Качество llama3.1-8b / qwen2.5-7b достаточно для AI-помощника и анализа сюжета
- Для дописывания глав лучше qwen2.5-14b или llama3.1-70b (если есть GPU)

### 7.4. Формат проекта

`.litgraph` — это JSON-файл со структурой:
```json
{
  "title": "Касіопея",
  "author": "Автор",
  "description": "...",
  "nodes": [...],
  "edges": [...],
  "createdAt": 1234567890,
  "updatedAt": 1234567890
}
```

Тот же формат что и в прототипе — совместимость 100%.

### 7.5. Хранение

```
~/.local/share/litgraph/
├── config.json              # { aiProvider: "ollama"|"openai"|"zai", ollamaUrl, openaiEndpoint, openaiKey, ... }
├── projects/
│   ├── kasiopia.litgraph
│   ├── my-novel.litgraph
│   └── ...
├── backups/                 # автобэкапы при сохранении
└── cache/                   # кэш AI-ответов (опционально)
```

---

## 8. Критерии готовности v0.1.0

- [ ] Устанавливается через `.deb` на Ubuntu 22.04+ / Mint / Pop!_OS
- [ ] Запускается из меню приложений или терминала
- [ ] Работает нодовый редактор: создание/редактирование/удаление нод и связей, focus-режим, поиск, мини-карта
- [ ] Импорт .md работает (автопарсер детектирует главы/персонажей/локации/тем)
- [ ] Полный текст главы в редакторе, версионирование работает
- [ ] Экспорт в JSON/текст/Markdown работает
- [ ] AI-помощник работает с Ollama (локально)
- [ ] AI-помощник работает с OpenAI-совместимым API (с пользовательским ключом)
- [ ] AI-функции "Дописать главу" и "Анализ сюжета" работают
- [ ] Проекты сохраняются между запусками
- [ ] `.AppImage` работает без установки (просто запустить)
- [ ] README.md с инструкциями: установка, сборка из исходников, настройка AI

---

## 9. Что НЕ делать (антипаттерны)

- ❌ НЕ переписывать React-фронтенд на Rust GUI (egui/iced/slint)
- ❌ НЕ использовать Python для бэкенда (проблемы со сборкой и зависимостями)
- ❌ НЕ использовать Electron (тяжёлый, ~150 МБ)
- ❌ НЕ хардкодить API-ключи в коде (только в config.json пользователя)
- ❌ НЕ требовать интернет для работы (Ollama работает офлайн)
- ❌ НЕ удалять совместимость с форматом .litgraph.json из прототипа (пользователь должен иметь возможность открывать проекты из превью в десктопе и наоборот)
- ❌ НЕ менять промпты AI-функций (они уже отлажены на Касіопее)

---

## 10. Исходные данные для разработки

### 10.1. Доступ к коду прототипа

Все 19 файлов прототипа (~5000 строк) доступны в песочнице Z.ai по ссылке превью. AI-разработчик должен:

1. Открыть превью: `https://preview-chat-3ca7c1b3-4e16-4a64-a46f-0c1e4ef31594.space-z.ai/`
2. Запросить у пользователя архив исходников (или скопировать файлы по списку из раздела 1.2)
3. Перенести фронтенд в Tauri-проект

### 10.2. Тестовый файл

`1-Касіопея Редактированое канон.md` (1.1 МБ, 103k слов, 60 глав) — использовать как основной тест-кейс. После автопарсинга должно получиться:
- 60 нод глав (с прологом)
- 25 нод персонажей (Ліана, Рівен, Скія, Каелус, Юна, Люма, Роан, и т.д.)
- 15 нод локаций
- 10 нод тем (Голос 236, Тень 135, Мгновение 122, Тишина 121, и т.д.)
- ~730 связей
- 60 нод с fullText (полным текстом главы)

### 10.3. Сравнение с прототипом

После завершения каждого этапа сравнивать с прототипом:
- Парсер должен давать тот же результат на том же файле
- AI-промпты должны давать сопоставимый результат
- UI должен выглядеть идентично

---

## 11. Команды для быстрого старта

```bash
# 1. Создать Tauri-проект
npm create tauri-app@latest litgraph-desktop -- --template react-ts
cd litgraph-desktop

# 2. Установить зависимости фронтенда
npm install @xyflow/react zustand lucide-react class-variance-authority clsx tailwind-merge cmdk tailwindcss @tailwindcss/vite tw-animate-css

# 3. Установить shadcn/ui
npx shadcn@latest init

# 4. Установить Tauri плагины
npm install @tauri-apps/plugin-store @tauri-apps/plugin-dialog @tauri-apps/plugin-fs
cargo add tauri-plugin-store tauri-plugin-dialog tauri-plugin-fs --manifest-path src-tauri/Cargo.toml

# 5. Установить Rust-крейты для парсера
cargo add fancy-regex unicode-segmentation serde serde_json reqwest tokio --manifest-path src-tauri/Cargo.toml

# 6. Скопировать фронтенд из прототипа
# (ручками из песочницы)

# 7. Запустить дев-режим
cargo tauri dev

# 8. Собрать релиз
cargo tauri build
```

---

## 12. Контакт и поддержка

- Превью-версия (для работы с AI в песочнице): `https://preview-chat-3ca7c1b3-4e16-4a64-a46f-0c1e4ef31594.space-z.ai/`
- Десктоп-версия: отдельный проект, не зависит от песочницы
- AI-интеграция: Ollama (по умолчанию, бесплатно) / OpenAI-совместимый API (пользовательский ключ) / Z.ai (опционально)
- Лицензия: MIT (открытый исходный код, любой может пересобрать под себя)

---

**Конец промпт-плана. Скопируй этот текст целиком и вставь в AI-помощника для разработки.**
