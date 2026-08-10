# Subagent 07 — Python Scripts, xtask Build Tools, Build Scripts

- **Task ID**: 07-tauri-python-xtask
- **Agent**: Explore (medium thoroughness)
- **Scope**: `src-tauri/python/*.py`, `xtask/src/*.rs`, `src-tauri/build.rs`, `scripts/test_ner.py`, `scripts/test_poler.ts`, `src-tauri/python/requirements.txt`, plus the Python↔Rust invocation boundary in `src-tauri/src/commands/ner.rs` and `src-tauri/src/commands/poler.rs`.
- **Repo root**: `/home/z/my-project/litgraph-desktop/`

---

## 1. Executive Summary

The LitGraph desktop app has **two parallel linguistic stacks**:

1. **Python (spaCy + pymorphy3 + numpy/scipy/sklearn)** — invoked as an out-of-process subprocess via `std::process::Command` from the Tauri backend. Backs three Tauri commands registered in `src-tauri/src/lib.rs`: `extract_entities`, `analyze_characters`, `extract_svo`. Targets **Russian** text. Source of truth for the legacy NER/POLER pipeline (Layers 1–2 from earlier architecture).

2. **Rust-native (litgraph-core)** — `SvoParser`, `pos_tagger`, `lemmatizer` modules bundled into the binary. Backs three Tauri commands with the `cmd_` prefix: `cmd_extract_svo`, `cmd_compute_epsilon_climax`, `cmd_detect_paradoxes`. Targets **Ukrainian** text. The "POLER Layer F" canonical engine.

The xtask workspace is a **build-time codegen tool** (not part of the runtime binary): it downloads LanguageTool UK resources and UD_Ukrainian-IU treebank, then compiles them into gzipped JSON artifacts under `resources/ua-linguistic/derivatives/` which are consumed by the Rust-native stack at runtime.

**Security posture of the Python boundary**: Low risk but suboptimal hygiene. Python is invoked via `Command::new(python).arg(script).arg(text_file)` — **no `shell=True`**, no shell interpolation. Text is passed via a **temp file** in `std::env::temp_dir()/litgraph_scripts_{pid}_{nanos}/input_text.txt` (avoiding the broken-pipe issue that affects large stdin payloads >100k chars). Temp directory is removed in a `finally`-style block. However: (a) the temp file is created with default permissions (world-readable on multi-user systems), (b) `text.trim().is_empty()` is the only input validation — no size cap, no content sanitization, (c) user text is written verbatim into a `.py`-importable workspace and then loaded by spaCy, which is acceptable but means maliciously crafted UTF-8 could in principle exploit parser edge cases in spaCy/pymorphy3.

---

## 2. Files Inspected

### 2.1 Python scripts (`src-tauri/python/`)

| File | LOC | Purpose |
|---|---|---|
| `ner_extract.py` | 1161 | Russian NER: spaCy `ru_core_news_sm` + pymorphy3, multi-token ФИО, contextual locations, false-positive noun blacklist, role-noun reclassification (CONCEPT/ORG/LOC/PER_KEEP_LAST/REJECT), HTML-comment stripping, chunked processing (50k chars/chunk). |
| `svo_extract.py` | 911 | Russian SVO triplets via spaCy dependency parsing. Pronoun resolution (3rd-person → recent PER by gender), pro-drop subject inheritance (conj/advcl), multi-token PER span reconstruction with case-matching, negation detection, polarity classification (positive/negative/neutral verb sets). |
| `poler_entities.py` | 385 | Integrator: NER + SVO → character co-occurrence graph (window=2000 chars) → POLER operator `H = Π_Λ (L + γJ - B/m) Π_Λ` (γ=0.05) → `scipy.sparse.linalg.eigsh` for 4 smallest eigenvalues → KMeans clustering → silhouette score. Also computes directed J matrix from SVO triplets (aggressor/victim asymmetry). |
| `conflict_graph.py` | (out of scope, not read in detail) | Sibling conflict-graph script. |
| `build_j_matrix.py` | (out of scope) | J-matrix construction helper. |
| `requirements.txt` | 19 | See §4. |

### 2.2 xtask workspace

| File | LOC | Purpose |
|---|---|---|
| `xtask/Cargo.toml` | 16 | Workspace crate with deps: `phf_codegen`, `quick-xml`, `reqwest` (blocking), `anyhow`, `tokio`, `regex`, `serde`, `serde_json`, `flate2`. |
| `xtask/src/main.rs` | 232 | Dispatcher: `build-lemmatizer`, `build-pos-tables`, `build-svo-templates` subcommands. Default action fetches LanguageTool RU+UK `replace.txt` and UK `grammar-barbarism.xml` from GitHub and generates a PHF cognate map into `src-tauri/src/dict/generated_cognates.rs` and `litgraph-core/src/dict/generated_cognates.rs` (duplicate output, byte-identical). |
| `xtask/src/build_pos_tables.rs` | 604 | Parses LanguageTool UK `disambiguation.xml` (486 rules, 6 action types: replace/remove/add/filter/filterall/immunize) + `case_government.txt` (~38,100 verb→case mappings) into `resources/ua-linguistic/derivatives/pos_rules.json.gz`. Streaming `quick_xml` event parser. Includes regex sanity-check + 6 unit tests. |
| `xtask/src/build_svo_templates.rs` | 294 | Parses UniversalDependencies `uk_iu-ud-train.conllu` treebank → SVO verb patterns (subject/object/instrument case sets per verb lemma) → `resources/ua-linguistic/derivatives/svo_templates.json.gz`. Auto-downloads from GitHub if local file missing. |
| `xtask/src/build_lemmatizer.rs` | 615+ (partial) | Builds `lemma_index.json.gz` from dict_uk (ВЕСУМ) `base.lst` + `exceptions.lst` + affix rules. Pure symbolic morphology, no ML. |

### 2.3 Build scripts

| File | LOC | Purpose |
|---|---|---|
| `src-tauri/build.rs` | 3 | Trivial — only calls `tauri_build::build()`. No codegen, no env var consumption, no Python detection. **No repo-root `build.rs` exists.** |

### 2.4 Test scripts

| File | LOC | Purpose |
|---|---|---|
| `scripts/test_ner.py` | 110 | Manual smoke test for NER: verifies spaCy + pymorphy3 + `ru_core_news_sm` model are installed, runs `extract_entities` on a fixed Russian test text (Anna/Vronsky/Kitty/Levin), expects ≥3 PER + ≥1 LOC. Not integrated into CI; user runs manually with `python3 scripts/test_ner.py` (recommends `~/.litgraph-venv/bin/python`). |
| `scripts/test_poler.ts` | 61 | Bun/TS test for the **frontend TS** POLER implementation (`src/lib/poler/analyze.ts`), not the Python one. Reads hardcoded path `/home/z/my-project/poler-prototype/data/sample_text.txt`. Compares against expected `nNodes=40, nEdges=419, silhouette≈0.372`. |

### 2.5 Python↔Rust bridge

| File | LOC | Purpose |
|---|---|---|
| `src-tauri/src/commands/ner.rs` | 238 | Three Tauri commands: `extract_entities`, `analyze_characters`, `extract_svo`. The `run_python_with_text_file` helper (lines 87–164) writes the script + user text into a temp dir, spawns `python3 <script> <text_file>` with piped stdout/stderr, parses stdout JSON. |
| `src-tauri/src/commands/poler.rs` | 706 | Three Tauri commands: `cmd_compute_epsilon_climax`, `cmd_extract_svo`, `cmd_detect_paradoxes`. **Pure Rust-native** — uses `litgraph_core::SvoParser` / `NarrativeGraph` / `ParadoxDetector`. No Python, no I/O, deterministic. |

---

## 3. Python Sandbox Security

### 3.1 Invocation mechanism

`src-tauri/src/commands/ner.rs:87-164` defines `run_python_with_text_file`:

```rust
let python_cmd = find_python();               // 1. venv, 2. $LITGRAPH_PYTHON, 3. "python3"
let temp_dir = std::env::temp_dir();
let pid = std::process::id();
let timestamp = std::time::SystemTime::now()...as_nanos();
let script_dir = temp_dir.join(format!("litgraph_scripts_{}_{}", pid, timestamp));
fs::create_dir_all(&script_dir)?;
fs::write(&script_dir.join("main_script.py"), script)?;
for (filename, content) in extra_files { fs::write(&script_dir.join(filename), content)?; }
fs::write(&script_dir.join("input_text.txt"), text)?;
let output = Command::new(&python_cmd)
    .arg(&main_script_path)
    .arg(&text_file)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()?;
let _ = fs::remove_dir_all(&script_dir);      // cleanup
```

**Verdict:**
- ✅ **No `shell=True`** — direct `Command::new(python).arg(script).arg(text_file)`. No `sh -c`, no shell metacharacter injection vector.
- ✅ **Temp dir name is unpredictable** (PID + nanosecond timestamp), reducing race-condition risk.
- ✅ **Temp dir is removed unconditionally** after the Python run (even on error — the cleanup is outside the `let result = (|| { ... })();` block).
- ⚠️ **Text is passed via file, not stdin** — this is documented as a deliberate fix for the "Канал оборвано (os error 32)" broken-pipe bug on >100k char inputs. Good engineering tradeoff.
- ⚠️ **Temp file default permissions** — `fs::write` uses default `0o644` on Linux. On a multi-user system, another local user could read the user's manuscript while the Python script runs (race window of seconds to minutes for large texts). Should use `fs::OpenOptions::new().mode(0o600)` or `tempfile` crate.
- ⚠️ **No size cap on input text** — only `text.trim().is_empty()` is checked. A user pasting a 50 MB document will trigger a 50 MB temp file write + spaCy processing that may OOM-kill. The error path handles "возможно OOM killed" gracefully but doesn't prevent it.
- ⚠️ **`find_python()` resolves to system `python3` if neither venv nor `$LITGRAPH_PYTHON` is set** — on a hostile system, a malicious `python3` earlier in `PATH` could execute arbitrary code with the user's text. The venv-first lookup mitigates this for users who follow the install instructions.
- ⚠️ **Script files written alongside text** — `main_script.py`, `ner_extract.py`, `svo_extract.py` are all written to the temp dir from `include_str!`. They are byte-identical to the bundled source. No injection vector here, but the pattern of writing executable code to `/tmp` is mildly anti-pattern (some hardened systems mount `/tmp` noexec, which would break this — though Python scripts are not ELF executables so `noexec` doesn't apply).

### 3.2 Python script content review

- ✅ `ner_extract.py` reads only `sys.argv[1]` (file path) or stdin. No `eval`, no `exec`, no `__import__` of untrusted code. Uses `re`, `spacy`, `pymorphy3`, `json`, `collections`.
- ✅ `svo_extract.py` same pattern — file/stdin in, JSON out.
- ✅ `poler_entities.py` imports `ner_extract` and `svo_extract` via `sys.path.insert(0, _SCRIPT_DIR)` (resolved from `__file__`, not cwd). This is the V3 fix for the `ModuleNotFoundError` mentioned in the docstring.
- ⚠️ `poler_entities.py` uses `numpy`, `scipy.sparse.linalg.eigsh`, `sklearn.cluster.KMeans`, `sklearn.metrics.silhouette_score`. The numpy random state for KMeans is fixed (`random_state=42, n_init=10`) → deterministic. No sandboxing concern but heavy dependency tree.
- ✅ None of the Python scripts call `subprocess`, `os.system`, `os.popen`, or open sockets. Pure local text processing.
- ✅ `pymorphy3.MorphAnalyzer()` is instantiated at module load — no per-call init cost.

### 3.3 Data exchange protocol (Python↔Rust)

**Direction:** Rust → Python → Rust.

1. Rust writes script(s) + `input_text.txt` to `/tmp/litgraph_scripts_{pid}_{nanos}/`.
2. Rust spawns `python3 main_script.py input_text.txt`.
3. Python reads `sys.argv[1]` as file path → `open(argv[1], "r", encoding="utf-8").read()`. (V2 fix; previously stdin.read().)
4. Python prints `json.dumps(result, ensure_ascii=False, indent=2)` to stdout.
5. Rust captures stdout via `Stdio::piped()`, parses with `serde_json::from_str(&stdout)`.
6. Rust deletes temp dir.

**Protocol: JSON over stdout, text over file arg.** No stdin used (Stdio::null()).

**Error contract:** Python prints `json.dumps({"error": str(e), "type": ..., "traceback": ...})` and `sys.exit(1)`. Rust checks `output.status.success()` first — if false, surfaces stderr as the error message (does NOT try to parse the JSON error envelope). If exit code is 0 but JSON is malformed, Rust surfaces the first 500 chars of stdout for debugging.

---

## 4. requirements.txt

`src-tauri/python/requirements.txt` (19 lines):

```
spacy>=3.8.0,<4.0.0
pymorphy3>=2.0.0
numpy>=1.24.0
scipy>=1.10.0
scikit-learn>=1.3.0
```

Plus a separately-installed spaCy model: `python -m spacy download ru_core_news_sm` (~10 MB).

**Notes:**
- All five deps are heavy: numpy+scipy+sklearn alone pull ~150 MB of wheels. spaCy adds ~30 MB + the ru_core_news_sm model (~10 MB). pymorphy3 is small but has its own Russian dictionary (~13 MB).
- No version pinning to a lockfile — uses `>=` lower bounds only. Reproducibility depends on whatever PyPI resolves at install time.
- The required `spacy<4.0.0` upper bound is significant: spaCy 4.x (when released) will likely break the `disable=["lemmatizer"]` API and the `token.morph.get(...)` calls.
- `scikit-learn` is pulled in only for `KMeans` + `silhouette_score` in `poler_entities.py`. If POLER-on-characters is replaced by the Rust-native Layer E `NarrativeGraph`, this dependency could be dropped.

---

## 5. xtask Purpose

The xtask workspace is a **build-time codegen tool** — it is **not compiled into the runtime Tauri binary**. It is invoked manually by developers (or CI) to regenerate static linguistic data.

### 5.1 Three subcommands

| Subcommand | Input | Output | Consumer at runtime |
|---|---|---|---|
| `cargo run -p xtask -- build-lemmatizer` | `resources/ua-linguistic/dict_uk/` (ВЕСУМ `base.lst`, `exceptions.lst`, affix rules) | `resources/ua-linguistic/derivatives/lemma_index.json.gz` (~5 MB compressed, ~30 MB raw) | `litgraph-core::linguistic::lemmatizer` |
| `cargo run -p xtask -- build-pos-tables` | `resources/ua-linguistic/languagetool/lt_disambiguation.xml` (486 rules) + `lt_case_government.txt` (~38,100 verb→case entries) | `resources/ua-linguistic/derivatives/pos_rules.json.gz` | `litgraph-core::linguistic::pos_tagger` |
| `cargo run -p xtask -- build-svo-templates` | `resources/ua-linguistic/ud-ukrainian/uk_iu-ud-train.conllu` (UD treebank, auto-downloaded from GitHub if missing) | `resources/ua-linguistic/derivatives/svo_templates.json.gz` (~150 KB) | `litgraph-core::linguistic::svo_parser` |

### 5.2 Default action (no subcommand)

Generates a PHF (perfect hash function) cognate map from:
- 16 hardcoded manual cognate pairs (RU↔UK names: алексей↔олексій, владимир↔володимир, etc.)
- LanguageTool UK `replace.txt` (fetched from GitHub, weight 0.95, source="Barbarism")
- LanguageTool RU `replace.txt` (weight 0.90, source="Spelling")
- LanguageTool UK `grammar-barbarism.xml` (weight 0.95, source="Barbarism")

Output is written to **two locations** (byte-identical duplicates, technical debt — same as the `src-tauri/src` vs `litgraph-core/src` duplication noted in worklog Task 1):
- `src-tauri/src/dict/generated_cognates.rs`
- `litgraph-core/src/dict/generated_cognates.rs`

The output file is a `phf::Map<&'static str, CognateEntry>` — a compile-time perfect hash map, zero runtime cost for lookups.

### 5.3 Network access

- `build-lemmatizer` and `build-pos-tables` read local files only.
- `build-svo-templates` auto-downloads the CoNLL-U treebank from `https://raw.githubusercontent.com/UniversalDependencies/UD_Ukrainian-IU/master/uk_iu-ud-train.conllu` if the local file is missing, then saves it for future runs.
- Default action fetches three files from `raw.githubusercontent.com/languagetool-org/languagetool/master/...`.

**Security note:** These are HTTP→HTTPS GitHub raw URLs fetched via `reqwest::blocking::get(url)?.text()?`. No TLS pinning, no checksum verification. A MITM could substitute malicious linguistic data. Acceptable for dev-time tooling, would be concerning if invoked at runtime (it is not).

### 5.4 Are they codegen for linguistic data?

**Yes, exactly.** The three subcommands compile open-source linguistic resources (dict_uk morphology, LanguageTool UK disambiguation rules, UD_Ukrainian-IU treebank) into compact gzipped JSON artifacts that the Rust-native `litgraph-core` linguistic modules load at runtime. This is the **Layer B/C foundation** for the Rust-native Ukrainian POLER engine — without these artifacts, `SvoParser`, `pos_tagger`, and `lemmatizer` cannot function.

The default action generates a different artifact (PHF cognate map) which is **compile-time** (not runtime) — it produces Rust source code that gets compiled into the binary.

---

## 6. Python vs Rust-native Fallback Decision Matrix

The codebase has **both** a Python SVO extractor and a Rust-native SVO extractor. They are **not fallbacks for each other** — they target **different languages** and serve **different layers**:

| Aspect | Python `extract_svo` (commands::ner) | Rust `cmd_extract_svo` (commands::poler) |
|---|---|---|
| **Language target** | Russian (`ru_core_news_sm`) | Ukrainian (dict_uk + UD_Ukrainian-IU) |
| **Engine** | spaCy dependency parser + pymorphy3 | `litgraph_core::linguistic::svo_parser::SvoParser` (rule-based, symbolic) |
| **Dependencies** | ~200 MB Python wheels + spaCy model | Bundled `.json.gz` artifacts (~5 MB total) + compile-time PHF maps |
| **Process** | Out-of-process subprocess (`std::process::Command`) | In-process, pure function |
| **Determinism** | Mostly deterministic (spaCy is deterministic, KMeans uses `random_state=42`) | Fully deterministic (no randomness, no I/O) |
| **Tauri command name** | `extract_svo` | `cmd_extract_svo` |
| **Frontend invocation** | `src/lib/poler/nerBridge.ts::extractSvo()` | `src/lib/tauri-commands.ts::cmdExtractSvo()` |
| **Layer in POLER spec** | Legacy Layer 1–2 (NER + POLER-on-characters) | Layer C (canonical POLER v7.5-LEM) |
| **Output schema** | Rich: `{triplets: [{subject, subjectLemma, subjectGender, verb, verbLemma, object, objectLemma, objectGender, sentence, position, tense, polarity, negated, pronounResolved, pronounResolvedTo}], stats, nerResult}` | Minimal: `[{actor, verb, target, instrument, location, polarity, confidence}]` |

**Coexistence:** Both are registered in `src-tauri/src/lib.rs:60-75` and both are invokable from the frontend. The docstring in `commands/poler.rs:26-30` explicitly states: *"Both can coexist: the frontend chooses which one to invoke based on whether it wants the Rust-native symbolic engine or the Python spaCy one."*

**Practical reality:** The frontend's primary SVO entry point in `src/lib/tauri-commands.ts` only wraps `cmd_extract_svo` (the Rust one). The Python `extract_svo` is wrapped in `src/lib/poler/nerBridge.ts` which is imported by the older PolerDialog/CharacterGraphDialog components. So the **Python path is still live for the character-graph feature**, while the **Rust-native path is used for ε-climax and paradox detection**.

**True fallback structure:**
- For **Ukrainian** text → Rust-native only (Python has no `uk_core_news_sm` model configured).
- For **Russian** text → Python only (Rust-native `SvoParser` is configured for Ukrainian morphology via dict_uk).
- The two stacks are **language-partitioned**, not **primary/fallback**.

---

## 7. Other Findings

### 7.1 `__pycache__` checked in

`src-tauri/python/__pycache__/ner_extract.cpython-314.pyc` exists in the repo (visible in the directory listing). This is a Python 3.14 bytecode cache — likely accidentally committed. Should be in `.gitignore` and removed from git history.

### 7.2 Two generated_cognates.rs files

`xtask/src/main.rs:110-113` writes the same PHF map to both `src-tauri/src/dict/generated_cognates.rs` and `litgraph-core/src/dict/generated_cognates.rs`. This is byte-identical duplication — same technical-debt pattern noted in worklog Task 1 for the `parser/`, `models/`, `ai/` directories.

### 7.3 Hardcoded absolute path in test

`scripts/test_poler.ts:8` reads `/home/z/my-project/poler-prototype/data/sample_text.txt` — a hardcoded developer-machine path. This script cannot run on any other machine without modification. Indicates the script is a developer-local sanity check, not a CI test.

### 7.4 `src-tauri/build.rs` is trivial

Only 3 lines: `fn main() { tauri_build::build() }`. No `println!("cargo:rerun-if-changed=...")`, no Python detection, no env var consumption. The xtask is invoked manually, not as part of the cargo build pipeline. **There is no `build.rs` in the repo root.**

### 7.5 xtask is a separate cargo workspace

`xtask/Cargo.toml` is at the repo root level (alongside `litgraph-core/Cargo.toml` and `src-tauri/Cargo.toml`), and `xtask/Cargo.lock` exists separately. This is the standard "xtask" pattern popularized by the Rust community — a separate binary crate for project automation. It is **not** part of the Tauri build.

### 7.6 Temp dir cleanup is best-effort

`src-tauri/src/commands/ner.rs:161`: `let _ = fs::remove_dir_all(&script_dir);` ignores cleanup errors. If the process is killed (SIGKILL, OOM, panic) between creation and cleanup, the temp dir leaks. The PID+nanosecond naming prevents collisions but doesn't self-clean. A periodic cleanup of stale `/tmp/litgraph_scripts_*` dirs would be hygienic.

### 7.7 spaCy model loading

`ner_extract.py:42-48` loads `ru_core_news_sm` with `disable=["lemmatizer"]` (for speed) and falls back to loading without disable if that fails. The model is loaded **at module import time** — every Python invocation pays the ~2-3 second model-load cost. For a Tauri desktop app where the user triggers NER interactively, this means every NER call has a 2-3s cold-start. There is **no persistent Python process** — each `extract_entities` call spawns a fresh Python, loads spaCy, processes, exits. This is a significant latency issue for interactive use.

`poler_entities.py` imports `ner_extract` (which loads spaCy) AND imports `svo_extract` (which loads spaCy again with lemmatizer enabled) — so the `analyze_characters` command loads spaCy **twice** per invocation.

### 7.8 Conflict-graph Python script (out of scope but noted)

`src-tauri/python/conflict_graph.py` and `src-tauri/python/build_j_matrix.py` exist in the same directory but were not in the inspection list. They are likely invoked by `src-tauri/src/commands/conflict.rs` (registered as `commands::conflict::get_conflict_graph` in lib.rs). Same invocation pattern presumed.

---

## 8. Risks & Recommendations

### 8.1 Security risks (Low→Medium severity)

1. **Temp file world-readable** (Low/Medium on multi-user systems): Use `fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&text_file)` or the `tempfile` crate's `tempfile_in(dir)` which uses 0o600.
2. **No input size cap** (Low): Add a `text.len() > MAX_LEN` check (e.g., 5 MB) with a friendly error before writing to temp file.
3. **System `python3` fallback** (Low): Document the venv requirement more loudly, or refuse to start if `find_python()` falls through to `"python3"` (require explicit `$LITGRAPH_PYTHON` opt-in for system Python).
4. **Hardcoded GitHub URLs in xtask** (Low): For dev-time only, acceptable. If xtask is ever run in CI behind a corporate proxy, ensure `HTTPS_PROXY` is respected (reqwest respects it by default).

### 8.2 Performance risks

1. **spaCy cold-start per call** (High impact on UX): Each `extract_entities` / `analyze_characters` / `extract_svo` Tauri command spawns Python and loads spaCy (2-3s). For interactive NER on a manuscript, this is painful. Options: (a) persistent Python worker process (JSON-RPC over stdin/stdout), (b) migrate NER to Rust-native (the `litgraph-core` stack already has Ukrainian; Russian would need a Russian morphology dictionary), (c) cache spaCy model in memory via a Tauri sidecar process.
2. **spaCy loaded twice in `analyze_characters`** (Medium): `poler_entities.py` imports both `ner_extract` (spaCy without lemmatizer) and `svo_extract` (spaCy with lemmatizer). Refactor to share a single `NLP` instance.

### 8.3 Correctness risks

1. **`ner_extract.py` line 102 contains a stray CJK comment** `# 生物 / biology` — non-breaking but indicates copy-paste from a multilingual notes file. Should be cleaned up.
2. **`scripts/test_poler.ts` hardcoded path** — will fail on any machine except the original developer's. Either parameterize via `process.argv` or remove from the scripts directory.
3. **`__pycache__/ner_extract.cpython-314.pyc` committed** — remove from git, add to `.gitignore`.

### 8.4 Architectural observation

The codebase has **two parallel linguistic stacks** that will eventually need to be unified:
- **Russian** (Python, spaCy, legacy) — backs the character-graph feature.
- **Ukrainian** (Rust-native, litgraph-core, canonical POLER v7.5-LEM) — backs ε-climax and paradox detection.

The `POLER_UA_LP_MASTER_ROADMAP_V8.md` and `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md` files in the repo root suggest this unification is planned. Until then, the Python stack remains a runtime dependency for the character-graph feature, and the `requirements.txt` deps (spaCy, pymorphy3, numpy, scipy, sklearn) must be installed by every desktop-app user who wants that feature.

---

## 9. Summary for the Orchestrator

**Python invocation is safe-but-not-hardened**: `std::process::Command` with no shell, text passed via temp file (not stdin, deliberately — fixes broken-pipe on large inputs), temp dir cleaned up best-effort. Main gaps: default-perm temp files (world-readable on multi-user Linux), no input size cap, no persistent Python worker (2-3s spaCy cold-start per call).

**xtask is dev-time codegen only** — three subcommands (`build-lemmatizer`, `build-pos-tables`, `build-svo-templates`) compile open-source Ukrainian linguistic resources (dict_uk, LanguageTool UK, UD_Ukrainian-IU treebank) into gzipped JSON artifacts consumed by the Rust-native `litgraph-core` at runtime. A fourth default action generates a compile-time PHF cognate map. Not part of the runtime binary.

**Python vs Rust is language-partitioned, not primary/fallback**: Python `extract_svo` targets Russian (spaCy `ru_core_news_sm`); Rust `cmd_extract_svo` targets Ukrainian (dict_uk + UD_Ukrainian-IU). Both are registered as live Tauri commands. The frontend uses Rust-native for ε-climax/paradox detection and Python for the character-graph feature.
