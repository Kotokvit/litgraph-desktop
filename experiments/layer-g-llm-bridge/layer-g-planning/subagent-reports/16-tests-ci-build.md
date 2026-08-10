# Subagent 16 — Tests, CI/CD, Build Configuration Audit

**Task ID:** 16-tests-ci-build
**Agent:** Explore (very thorough)
**Scope:** Test suite, CI/CD pipelines, build configuration, Tauri 2 capabilities, security posture of tauri.conf.json

---

## 1. Executive Summary

LitGraph has a healthy **Rust** test surface (≈406–413 tests) but a **completely missing TypeScript test layer** (zero `.test.ts`/`.test.tsx` files, no test runner installed) and an **immature CI/CD pipeline** consisting of a single Linux-only, tag-triggered release workflow with **no test, lint, typecheck, clippy, or audit steps**. The Tauri 2 capabilities directory *does* exist (correcting the Task-1 worklog note), but grants overly permissive `fs:**` access to all paths. The `tauri.conf.json` CSP is set to `null` (insecure). Layer G (LLM Reasoning Bridge) already has a partial scaffold in `src-tauri/src/reasoning/llm_bridge.rs` (1099 LOC, 10 tests) that needs ~6 additional test categories before production.

---

## 2. Test Count Inventory (Verified by Grep)

### 2.1 litgraph-core (Rust library crate)

| File | `#[test]` count |
|---|---|
| `litgraph-core/src/linguistic/pos_tagger.rs` | 32 |
| `litgraph-core/src/parser/epsilon.rs` | 22 |
| `litgraph-core/src/ukrainian_semantic_categories.rs` | 12 |
| `litgraph-core/src/reasoning/narrative_graph.rs` | 11 |
| `litgraph-core/src/reasoning/mod.rs` | 9 |
| `litgraph-core/src/linguistic/lemmatizer.rs` | 9 |
| `litgraph-core/src/languagetool_weights.rs` | 6 |
| `litgraph-core/src/reasoning/paradox.rs` | 6 |
| `litgraph-core/src/reasoning/stub.rs` | 5 |
| `litgraph-core/src/linguistic/svo_parser.rs` | 3 |
| `litgraph-core/src/dict/cognate.rs` | 2 |
| **Subtotal litgraph-core/src** | **117** |

| File | `#[test]` count |
|---|---|
| `litgraph-core/tests/parser_test.rs` | 3 (test_parse_kasiopia, test_simple_text, test_empty_text) |
| `litgraph-core/tests/sfera_test.rs` | 1 (test_parse_sfera_v040) |
| `litgraph-core/tests/chapters_only_test.rs` | 1 (test_chapters_only) |
| `litgraph-core/tests/profile_test.rs` | 1 (test_profile_sfera) |
| `litgraph-core/tests/test_lt.rs` | 1 (test_vozvrashatsja_debug) |
| **Subtotal litgraph-core/tests** | **7** (integration tests, not counted in worklog Task-1's 406) |

### 2.2 src-tauri (Tauri desktop crate)

| File | `#[test]` | `#[tokio::test]` |
|---|---|---|
| `src-tauri/src/reasoning/semantic_parser.rs` | 135 | 0 |
| `src-tauri/src/reasoning/causality.rs` | 13 | 0 |
| `src-tauri/src/reasoning/llm_bridge.rs` | 10 | 0 |
| `src-tauri/src/reasoning/memory.rs` | 11 | 0 |
| `src-tauri/src/reasoning/planner.rs` | 11 | 0 |
| `src-tauri/src/reasoning/rules.rs` | 10 | 0 |
| `src-tauri/src/reasoning/timeline.rs` | 10 | 0 |
| `src-tauri/src/reasoning/integration_tests.rs` | 7 | 0 |
| `src-tauri/src/reasoning/inference.rs` | 8 | 0 |
| `src-tauri/src/reasoning/constraints.rs` | 8 | 0 |
| `src-tauri/src/reasoning/facts.rs` | 7 | 0 |
| `src-tauri/src/reasoning/contradictions.rs` | 6 | 0 |
| `src-tauri/src/reasoning/state.rs` | 6 | 0 |
| `src-tauri/src/reasoning/hypotheses.rs` | 5 | 0 |
| `src-tauri/src/reasoning/cycle.rs` | 5 | 0 |
| `src-tauri/src/ukrainian_semantic_categories.rs` | 12 | 0 |
| `src-tauri/src/commands/poler.rs` | 9 | 0 |
| `src-tauri/src/languagetool_weights.rs` | 6 | 0 |
| `src-tauri/src/dict/cognate.rs` | 3 | 0 |
| `src-tauri/src/commands/reasoning.rs` | 0 | 7 |
| **Subtotal src-tauri/src** | **282** | **7** |
| **src-tauri grand total** | **289** (matches worklog Task-1) | |

### 2.3 Grand totals

| Scope | Count |
|---|---|
| litgraph-core/src + src-tauri/src (in-module) | 117 + 289 = **406** ✓ (matches worklog Task-1) |
| litgraph-core/tests/ (integration) | **7** |
| **Grand total Rust tests** | **413** |
| TS/TSX tests | **0** |

**Notes:**
- `cargo` is not installed in this sandbox (`cargo: command not found`), so `cargo test --no-run --workspace` could not be executed. Counts are derived from `grep '#\[test\]'` and `grep '#\[tokio::test\]'` at source level.
- There is no root `Cargo.toml` workspace file — `litgraph-core/` and `src-tauri/` are independent crates. `cargo test --workspace` would fail; tests must be run per-crate (`cd litgraph-core && cargo test` then `cd src-tauri && cargo test`).
- The single integration test `test_parse_kasiopia` (parser_test.rs:13) asserts `result.stats.words >= 100000` and reads `tests/kasiopia.md`. Verified the fixture file is **1,161,126 bytes** (~1.16 MB, 22 lines with very long single-paragraph content) — assertion should pass on real corpus.

### 2.4 Test fixture inventory

| Fixture | Size | Used by |
|---|---|---|
| `litgraph-core/tests/sfera.md` | 5082 lines | sfera_test, chapters_only_test, profile_test, integration_tests::test_eval_sfera_predela_full |
| `litgraph-core/tests/kasiopia.md` | 1.16 MB, 22 lines | parser_test::test_parse_kasiopia, integration_tests::test_eval_kasiopia_full |
| `litgraph-core/examples/test_sfera.rs` | example, NOT a `#[test]` — uses hardcoded `/home/z/my-project/upload/sfera-predela.md` (env-specific) |
| `tests/corpus/{01..05}_*.md` | 5 small (~3-8 KB) corpus files | TS-side `scripts/test_*` ad-hoc, not Rust `#[test]` |
| `tests/corpus/results/*.json` | expected-output snapshots | ad-hoc only, no test runner enforces them |

---

## 3. Test Coverage Gaps (Critical)

### 3.1 Untested Rust modules (P0)

| Module | LOC | Test count | Risk |
|---|---|---|---|
| `src-tauri/src/commands/parse_md_full.rs` | 348 | 0 | Markdown→graph full pipeline IPC, silently broken if regressed |
| `src-tauri/src/commands/ner.rs` | 237 | 0 | **Python subprocess invocation** — most fragile IPC, zero coverage |
| `src-tauri/src/commands/export.rs` | 212 | 0 | HTML/SVG/PDF export paths |
| `src-tauri/src/commands/versions.rs` | 164 | 0 | Project version snapshot/restore |
| `src-tauri/src/commands/conflict.rs` | 143 | 0 | Conflict graph IPC |
| `src-tauri/src/commands/ai.rs` | 61 | 0 | **AI provider plumbing bug confirmed (Task-5)** — no test guards regression |
| `src-tauri/src/commands/project.rs` | 26 | 0 | Project CRUD |
| `src-tauri/src/commands/parse_md.rs` | 11 | 0 | Thin wrapper, low risk |
| `src-tauri/src/commands/mod.rs` | 12 | 0 | Module re-exports only |
| `src-tauri/src/storage/mod.rs` | ? | 0 | Project persistence to disk |
| `src-tauri/src/ai/{mod,prompts,openai_compat,ollama,types}.rs` | 744 | 0 | **All HTTP/LLM client code untested** — no mock, no retry test |
| `src-tauri/src/parser/{mod,chapters,characters,locations,themes,epsilon}.rs` | ~4500 | 0 | Diverged duplicates of litgraph-core; drift bugs undetected |
| `src-tauri/src/models/{mod,node,edge,project,version}.rs` | 168 | 0 | Diverged from core (Concept/Organization variants added) |
| `src-tauri/src/linguistic_entities.rs` | large | 0 | Static data, low risk |
| `litgraph-core/src/ai/*` | 744 | 0 | Same as src-tauri/ai |
| `litgraph-core/src/models/*` | 168 | 0 | Pure structs |
| `litgraph-core/src/parser/{chapters,characters,locations,themes}.rs` | ~1500 | 0 | Covered only via integration tests, no unit tests |
| `litgraph-core/src/dict/cognate.rs` | 85 | 2 | **Cognate map divergence bug confirmed (Task-1)** — only 2 tests guard |

**Coverage summary:** 19 of 46 `.rs` files have at least one test; 27 files (59%) have zero tests. The reasoning subsystem (src-tauri/src/reasoning/, 22k LOC) is heavily tested (252 tests, density ~1 test/87 LOC), but the **IPC command layer and AI/HTTP client layer are essentially untested**.

### 3.2 Untested TypeScript (P0)

| Concern | Status |
|---|---|
| Component tests for 25 `src/components/**/*.tsx` files | **0 tests** |
| Integration tests for `src/lib/tauri-commands.ts` wrappers | **0 tests** |
| State management tests for `src/lib/litgraph/store.ts` (Zustand) | **0 tests** |
| POLER algorithm tests for `src/lib/poler/*.ts` (6 files, ~3k LOC) | **0 tests** |
| Conflict-graph logic tests for `src/lib/conflict/*.ts` | **0 tests** |
| Export utility tests for `src/lib/litgraph/export*.ts` | **0 tests** |
| E2E tests for LitApp user flows | **0 tests** |
| `package.json` `"test"` script | **Missing** |
| Test runner installed (vitest/jest/playwright/@testing-library) | **None** |

`package.json` devDependencies include only `eslint`, `typescript`, `vite`, `tailwindcss` — no test framework. The `tests/corpus/` directory exists with 5 small Markdown fixtures and expected-output JSON snapshots, but these are consumed only by ad-hoc scripts in `scripts/test_*.{ts,mjs,py}`, not by any structured test runner.

### 3.3 Untested Python (P2)

- `src-tauri/python/{ner_extract,poler_entities,svo_extract}.py` — exercised only by ad-hoc `scripts/test_ner.py` (110 LOC), not by pytest or CI.

---

## 4. CI/CD Pipeline Audit

### 4.1 Workflow inventory

| File | Triggers | Purpose |
|---|---|---|
| `.github/workflows/release.yml` | `push: tags: ["v*"]` + `workflow_dispatch` | Build & release Linux desktop binary |

**That is the entire CI surface.** No additional workflows exist for: pull requests, push to main, scheduled runs, dependabot, code scanning, or nightly builds.

### 4.2 release.yml detailed findings

```yaml
name: Release
on:
  push:
    tags: ["v*"]
  workflow_dispatch:
jobs:
  build-linux:
    runs-on: ubuntu-22.04
    steps:
      - checkout, setup-node@v4 (node 20), rust-toolchain@stable
      - apt-get install libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
      - npm ci
      - tauri-apps/tauri-action@v0
        args: --target x86_64-unknown-linux-gnu
```

**Critical gaps:**
1. **No `npm test` step** — even if a test script existed, CI wouldn't run it.
2. **No `cargo test` step** — 413 Rust tests are written but never executed in CI.
3. **No `cargo clippy` / `cargo fmt --check`** — code-quality regressions undetected.
4. **No `tsc --noEmit` step** — TS type errors ship to release.
5. **No `npm run lint` step** — `eslint .` defined in package.json but never invoked.
6. **No `cargo audit` / `npm audit`** — security vulnerabilities in deps undetected.
7. **No caching** of `~/.cargo/registry`, `target/`, `~/.npm` — each release rebuilds from scratch (slow).
8. **No coverage upload** (codecov/tarpaulin).
9. **Single-target Linux build only** — see §6 Build Matrix.
10. **`tauri-action@v0`** — major version pinned to v0; v0.x is the legacy pre-1.0 line. Should pin to a specific recent tag.
11. **No code signing** for Linux (no GPG for .deb / .rpm).
12. **Release is drafted** (`releaseDraft: true`) — no auto-publish.
13. **No artifact attestation / SLSA provenance**.
14. **No secrets scanning step** (gitleaks/trufflehog).

### 4.3 CI/CD maturity score

| Dimension | Score | Notes |
|---|---|---|
| Build automation | 3/10 | Tag-only, no PR CI |
| Test automation | 0/10 | Tests exist but never run in CI |
| Lint/format automation | 0/10 | eslint exists, not invoked |
| Cross-platform matrix | 1/10 | Linux-only |
| Security scanning | 0/10 | No cargo/npm audit, no code scanning |
| Caching | 0/10 | No cache config |
| Release pipeline | 4/10 | tauri-action works, but no signing/attestation |
| **Overall** | **~1/10** | Significantly below industry baseline for a Tauri 2 app |

---

## 5. Tauri 2 Capabilities Audit

### 5.1 Correction to worklog Task-1

Worklog Task-1 stated "отсутствие capabilities/" — **this is incorrect**. The directory `src-tauri/capabilities/default.json` exists with 44 lines of configuration. The capabilities model is correctly wired for Tauri 2.

### 5.2 Capabilities content review

`src-tauri/capabilities/default.json` grants to the `main` window:

| Permission | Scope | Risk |
|---|---|---|
| `core:default` | All core Tauri APIs | Baseline, acceptable |
| `core:window:allow-{minimize,maximize,unmaximize,close,set-title,start-dragging}` | Window controls | Acceptable |
| `core:event:default` | Event system | Acceptable |
| `dialog:default` + `dialog:allow-open` + `dialog:allow-save` | Native file dialogs | Acceptable |
| `store:default` | Persistent key-value store | Acceptable |
| **`fs:default`** | Default fs plugin | Acceptable baseline |
| **`fs:allow-read-file`** | `{ "path": "**" }` | **CRITICAL: any file readable** |
| **`fs:allow-read-text-file`** | `{ "path": "**" }` | **CRITICAL: any text file readable** |
| **`fs:allow-write-text-file`** | `{ "path": "**" }` | **CRITICAL: any text file writable** |

**Security finding (HIGH):** Filesystem permissions grant `**` (any path on the host filesystem). A compromised renderer or malicious markdown payload could exfiltrate `~/.ssh/id_rsa`, `~/.aws/credentials`, or overwrite `~/.bashrc`. The Tauri 2 capabilities model exists specifically to scope these to safe directories (e.g., `$HOME/Documents/LitGraph/*`, `$APPDATA/*`).

**Recommended fix:**
```json
{
  "identifier": "fs:allow-read-text-file",
  "allow": [
    { "path": "$HOME/Documents/LitGraph/**" },
    { "path": "$APPDATA/LitGraph/**" }
  ]
}
```

Plus explicit user-approved file-open via `dialog:allow-open` (which is already present).

### 5.3 Missing capabilities

- No `shell:` permissions — good (no external process spawning from frontend).
- No `http:` permissions — good (frontend can't make arbitrary HTTP requests; all network goes through Rust `reqwest`).
- No `notification:` permissions — acceptable (no notifications in app).
- No `os:` permissions — acceptable.

---

## 6. tauri.conf.json CSP Audit

```json
"app": {
  "security": {
    "csp": null
  }
}
```

**Security finding (HIGH):** `csp: null` disables Content Security Policy entirely. Any XSS-style injection (e.g., via user-supplied markdown rendered with `dangerouslySetInnerHTML`, or a malicious SVO highlight injected into the LitCanvas) can execute arbitrary scripts in the WebView context — and because the WebView has Tauri IPC access via `@tauri-apps/api`, this effectively grants the attacker the full Tauri capability surface (including the `fs:**` permissions above).

**Recommended CSP:**
```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https://tauri.localhost; connect-src 'self' ipc: http://ipc.localhost https://api.z.ai; font-src 'self' data:"
```

---

## 7. Build Matrix Audit

### 7.1 Current matrix

| OS | Target | Bundle format |
|---|---|---|
| Ubuntu 22.04 | `x86_64-unknown-linux-gnu` | `deb`, `appimage`, `rpm` |

### 7.2 Missing targets

| Target | Users locked out | Tauri 2 support |
|---|---|---|
| `aarch64-apple-darwin` (Apple Silicon) | All M1/M2/M3 Mac users | ✅ supported |
| `x86_64-apple-darwin` (Intel Mac) | Intel Mac users | ✅ supported |
| `x86_64-pc-windows-msvc` (Windows) | All Windows users | ✅ supported |
| `aarch64-unknown-linux-gnu` (Linux ARM) | Raspberry Pi / ARM server users | ✅ supported |

**Bundle formats missing:**
- macOS: `.dmg` (and `.app` code-signed/notarized)
- Windows: `.msi` (via NSIS or WiX), `.exe` (via NSIS)

### 7.3 Recommended matrix

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - { os: ubuntu-22.04, target: x86_64-unknown-linux-gnu, bundles: "deb,appimage,rpm" }
      - { os: macos-14,     target: aarch64-apple-darwin,     bundles: "dmg" }
      - { os: macos-13,     target: x86_64-apple-darwin,      bundles: "dmg" }
      - { os: windows-latest, target: x86_64-pc-windows-msvc, bundles: "msi,nsis" }
```

---

## 8. Build Configuration Audit

### 8.1 Root project layout

- **No root `Cargo.toml` workspace file exists.** `litgraph-core/` and `src-tauri/` are independent crates linked via `path = "../litgraph-core"` dependency in `src-tauri/Cargo.toml`.
- `xtask/` is a third independent crate (dev-time codegen) — not in a workspace either.
- **Recommendation:** introduce root `Cargo.toml` with `[workspace] members = ["litgraph-core", "src-tauri", "xtask"]` to enable `cargo test --workspace`, `cargo clippy --workspace`, shared `Cargo.lock`.

### 8.2 litgraph-core/Cargo.toml

- Edition 2021, no `rust-version` pin.
- 22 dependencies including `tokio` (full features — heavy), `reqwest` (json), `petgraph` 0.6, `phf` 0.11, `flate2`, `fancy-regex`, `regex`, `unicode-segmentation`, `chrono`, `uuid` (v4), `thiserror`, `dirs`, `serde`/`serde_json`.
- **No `[dev-dependencies]` section** — tests use only main deps. Acceptable since no `proptest`, `pretty_assertions`, `tempfile`, etc. are used.
- **No `[features]` section** — no way to gate test-only or optional features.
- No `[[bench]]` or `[[example]]` declarations (though `examples/test_sfera.rs` exists).

### 8.3 src-tauri/Cargo.toml

- `rust-version = "1.70"` — declared but workspace lacks MSRV check in CI.
- `litgraph-core = { path = "../litgraph-core" }` — path dep, no version pin.
- Tauri 2 + plugins `tauri-plugin-store`, `tauri-plugin-dialog`, `tauri-plugin-fs` — all at major version 2.
- `crate-type = ["staticlib", "cdylib", "rlib"]` — supports both Tauri mobile (staticlib) and desktop (cdylib) builds.
- **Duplicate deps with litgraph-core:** `serde`, `serde_json`, `fancy-regex`, `regex`, `unicode-segmentation`, `reqwest`, `tokio`, `dirs`, `chrono`, `uuid`, `thiserror`, `phf`, `flate2` — 13 deps duplicated. Should rely on litgraph-core re-exports.
- `tokio = { version = "1", features = ["full"] }` — "full" pulls in `tokio::net`, `tokio::process`, `tokio::signal`, `tokio::fs`, etc. Many unused. Should be `features = ["rt-multi-thread", "macros"]`.
- **No `[dev-dependencies]`** — same issue as core.
- **No `[features]`** beyond `custom-protocol`. No way to gate Python-subprocess features, LLM features, etc.

### 8.4 package.json

- **No `"test"` script.** Only `dev`, `build`, `preview`, `tauri`, `lint`.
- `eslint .` defined but not invoked by any hook or CI.
- No `prettier`, `husky`, `lint-staged` — no pre-commit hooks.
- No `vitest`/`jest`/`playwright`/`@testing-library/react` in devDependencies.
- React 19, Vite 6, Tailwind 4, TypeScript 5, `@xyflow/react` 12, Zustand 5 — modern stack, ready for vitest.
- `utif` 3.1.0 — TIFF decoder for background image import (has pre-existing build failure noted in worklog Layer F.2).

### 8.5 tauri.conf.json (non-security)

- `version: "0.2.2"` — matches package.json and src-tauri/Cargo.toml. ✓
- `identifier: "com.litgraph.desktop"` — valid reverse-DNS. ✓
- `build.beforeDevCommand: "npm run dev"` + `devUrl: "http://localhost:1420"` — standard Vite + Tauri dev setup. ✓
- `build.beforeBuildCommand: "npm run build"` (which runs `tsc && vite build`) — type errors fail the build. ✓
- Window: 1440x900 default, 1024x700 min, resizable, not fullscreen. ✓
- Bundle targets: `["deb", "appimage", "rpm"]` — Linux only.
- No `macOSPrivateFramework`, no `app.minimumSystemVersion`, no `windows.certificateThumbprint`, no `windows.webviewInstallMode` — cross-platform build config missing.

### 8.6 xtask (dev-time codegen)

- 3 subcommands: `build-lemmatizer`, `build-pos-tables`, `build-svo-templates`.
- Generates `resources/ua-linguistic/derivatives/*.json.gz` artifacts consumed at runtime by litgraph-core.
- Default action generates PHF cognate map into BOTH `src-tauri/src/dict/generated_cognates.rs` AND `litgraph-core/src/dict/generated_cognates.rs` — duplication flagged in Task-6 worklog.
- **Not invoked by CI.** If artifacts drift, runtime breaks silently.

---

## 9. Layer G Test Strategy

### 9.1 Existing Layer G scaffold (already shipped)

`src-tauri/src/reasoning/llm_bridge.rs` (1099 LOC, 10 `#[test]`):
- `LlmBridge` struct (sync, no async)
- `build_prompt(&req, &world, &facts) -> (String, String)` — builds (system, user) prompts
- `validate_response(&generated, &req, &world, &facts, &resolver, &chapters) -> ValidationResult`
- `ValidationResult::{Accept, Reject, Retry}` enum
- 10 unit tests covering existing validators

### 9.2 Test gaps for Layer G production-readiness

#### P0 — Unit tests (Rust)
1. **Paradox→prompt mapping coverage**: tests for every `ParadoxKind` variant (`TemporalInconsistency`, `ResurrectWithoutDeath`, `SpatialTeleportation` [currently never emitted per Task-3], `CausalLoop`). Currently only `TemporalInconsistency` is exercised.
2. **`ValidationResult::Accept` branch**: validator accepts valid flashback explanation. (Partially exists in `test_verifier_accepts_flashback_resolution` in `hypotheses.rs:721`.)
3. **`ValidationResult::Reject` branch with feedback_prompt**: reject + generate correct feedback. (Exists in `hypotheses.rs:791` for resurrect-without-event — needs LLM-bridge-level mirror.)
4. **`ValidationResult::Retry` branch**: never tested; needs a test where validator returns `Retry { reason }`.
5. **JSON-mode response parsing**: when LLM is asked to return structured JSON (`response_format=json_object`), `validate_response` must parse JSON envelope, not free-text. No test today.
6. **Token budget / context window truncation**: prompt must fit in provider's context window. `build_assistant_prompt` currently truncates at hardcoded 4000 chars (Task-4 G12). Need property test: prompt size ≤ N tokens.
7. **Hypothesis deduplication & ranking**: when multiple hypotheses generated for same paradox, `HypothesisLog` must dedupe and rank by confidence. `test_hypothesis_log_assigns_sequential_ids` exists (`hypotheses.rs:850`) but no dedup/rank test.

#### P0 — Integration tests (Rust, with mock LLM)
8. **Mock `AiClient` trait**: introduce `trait AiClient { async fn chat(&self, provider: &AiProvider, messages: Vec<ChatMessage>) -> Result<String, AiError>; }` and a `MockAiClient` impl. Inject into `LlmBridge` so integration tests don't hit network. (Currently `LlmBridge` doesn't call `ai::chat` — async boundary — but the Tauri command layer does. Mock must be at command layer.)
9. **Full cycle integration test**: paradox detection → prompt construction → mock LLM returns flashback explanation → validator Accept → events committed → cycle report contains `hypotheses_accepted ≥ 1`.
10. **Retry-loop integration test**: mock LLM returns invalid text first (e.g., "Пётр воскрес без причины") → validator Reject + feedback_prompt → second LLM call returns valid explanation → Accept.
11. **Idempotency**: same paradox fed twice → no duplicate hypotheses, no duplicate facts. (Pattern exists in `test_cycle_idempotent_on_same_events` at `integration_tests.rs:272` for non-LLM cycle; needs LLM-bridge equivalent.)

#### P1 — Validator regression tests
12. **Constraint-violating generation rejected**: LLM returns text where dead character speaks → validator rejects with `dead_cannot_speak` constraint name.
13. **Partial generation with caveats accepted**: LLM returns valid explanation but introduces new fact not in prompt → Accept with warnings (not Reject).
14. **Empty / malformed LLM response**: validator returns `Retry { reason: "empty response" }`, not `Accept`.

#### P1 — Wire/serialization tests
15. **`ParadoxReportDto` round-trip**: serialize → deserialize preserves all fields (especially `evidence_text` and composite-key `id` per Task-3).
16. **`HypothesisReportDto` round-trip**: same.
17. **camelCase wire field stability**: tests asserting `provider.apiKey` (not `api_key`) etc. — currently relies on per-field `#[serde(rename = "...")]` (Task-5).

#### P2 — Performance/regression tests
18. **Prompt-size regression**: snapshot test on `build_prompt` output size for a canonical paradox — fail if size grows >10%.
19. **LLM call count regression**: integration test asserts at most 2 LLM calls per paradox (initial + 1 retry).
20. **Token-usage accounting** (requires Task-4 R3 trait extension): test that `tokens_used` is correctly summed across retries.

### 9.3 TS-side test needs for Layer G UI (P1)

- Component test for `<ReasoningDialog>` (currently 0 tests) — render with mock paradox list, assert paradox cards displayed.
- Component test for `<AIDialog>` / `<AssistantDialog>` (Task-5 confirmed provider plumbing bug) — assert that `provider` is passed to `invoke()` (regression guard).
- Integration test for `src/lib/tauri-commands.ts` wrappers — verify camelCase wire field names match Rust `#[serde(rename)]`.

---

## 10. Findings (Atomic)

1. **F-01**: Total Rust test count = **413** (117 litgraph-core/src + 7 litgraph-core/tests + 282 src-tauri/src `#[test]` + 7 src-tauri/src `#[tokio::test]`). Worklog Task-1 figure of 406 omits the 7 integration tests in `litgraph-core/tests/`. (§2.3)
2. **F-02**: Test density is heavily skewed — `src-tauri/src/reasoning/semantic_parser.rs` alone has 135 tests (33% of all src-tauri tests), while 27 of 46 `.rs` files have **zero** tests. (§3.1)
3. **F-03**: All 11 `src-tauri/src/commands/*.rs` modules except `poler.rs` (9 tests) and `reasoning.rs` (7 tokio tests) have **zero** test coverage. (§3.1)
4. **F-04**: All 5 `src-tauri/src/ai/*.rs` files have **zero** test coverage — HTTP/LLM client code is unguarded. (§3.1)
5. **F-05**: **Zero TypeScript tests** anywhere in repo — no `.test.ts`/`.test.tsx`/`.spec.ts`/`.spec.tsx` files, no test runner in `package.json`, no `"test"` script. (§3.2)
6. **F-06**: `tests/corpus/` directory contains 5 small Markdown fixtures + expected-output JSON snapshots, but consumed only by ad-hoc `scripts/test_*.{ts,mjs,py}` — not by any structured test runner. (§2.4, §3.2)
7. **F-07**: `.github/workflows/release.yml` is the **only** CI workflow. Tag-triggered + `workflow_dispatch`. Linux-only (`ubuntu-22.04`, `x86_64-unknown-linux-gnu`). (§4.1)
8. **F-08**: release.yml has **no `cargo test`, `cargo clippy`, `cargo fmt --check`, `tsc --noEmit`, `npm run lint`, `cargo audit`, or `npm audit` step**. All 413 Rust tests are written but never executed in CI. (§4.2)
9. **F-09**: release.yml has **no caching** of `~/.cargo/registry`, `target/`, or `~/.npm` — each release rebuilds from scratch. (§4.2)
10. **F-10**: Build matrix is **Linux-only**. No macOS (aarch64-apple-darwin, x86_64-apple-darwin) and no Windows (x86_64-pc-windows-msvc) builds. Tauri 2 supports all three; current setup locks out Mac and Windows users. (§7.2)
11. **F-11**: Bundle targets = `["deb", "appimage", "rpm"]` — Linux package formats only. Missing `.dmg` (macOS) and `.msi`/`.exe` (Windows). (§7.2)
12. **F-12** (CORRECTION to worklog Task-1): `src-tauri/capabilities/default.json` **EXISTS** with 44 lines of Tauri 2 capability config. Worklog Task-1 statement "отсутствие capabilities/" is incorrect. (§5.1)
13. **F-13** (SECURITY HIGH): Capabilities grant `fs:allow-read-file`, `fs:allow-read-text-file`, `fs:allow-write-text-file` with `**` (any path) scope — renderer can read/write any file on user's filesystem. (§5.2)
14. **F-14** (SECURITY HIGH): `tauri.conf.json` has `"csp": null` — Content Security Policy disabled. Combined with F-13, XSS in renderer → full filesystem access. (§6)
15. **F-15**: No root `Cargo.toml` workspace file. `litgraph-core`, `src-tauri`, `xtask` are independent crates. `cargo test --workspace` would fail. (§8.1)
16. **F-16**: `src-tauri/Cargo.toml` declares 13 dependencies duplicated with `litgraph-core/Cargo.toml` — should rely on re-exports. (§8.3)
17. **F-17**: `tokio = { features = ["full"] }` in both crates — pulls in unused `tokio::net`, `tokio::process`, `tokio::signal`. Should be `["rt-multi-thread", "macros"]`. (§8.3)
18. **F-18**: `tauri-apps/tauri-action@v0` — major version pinned to v0 (legacy pre-1.0 line). Should pin to a specific recent tag. (§4.2)
19. **F-19**: `examples/test_sfera.rs` has hardcoded path `/home/z/my-project/upload/sfera-predela.md` — env-specific, won't run anywhere else. (§2.4)
20. **F-20**: `integration_tests.rs::test_eval_sfera_predela_full` and `test_eval_kasiopia_full` have hardcoded absolute paths `/home/vitalij/Музика/...` and `/home/vitalij/Документи/...` — env-specific fallbacks. Tests silently `return` if files not found (mask failures in CI). (§2.4, integration_tests.rs:299-315)
21. **F-21**: Layer G scaffold already exists: `src-tauri/src/reasoning/llm_bridge.rs` (1099 LOC, 10 tests) with `LlmBridge::build_prompt` + `validate_response` + `ValidationResult::{Accept,Reject,Retry}`. (§9.1)
22. **F-22**: Layer G production-readiness needs 20 additional tests across 4 categories (unit, integration w/ mock LLM, validator regression, wire/serialization). Detailed in §9.2. (§9.2)
23. **F-23**: No `MockAiClient` trait/impl exists — Layer G integration tests currently cannot avoid real network calls. Trait abstraction (Task-4 R3) is a prerequisite for testable Layer G. (§9.2 P0 item 8)
24. **F-24**: `cargo` not installed in sandbox — `cargo test --no-run --workspace` could not be executed; counts are source-grep-derived, not build-verified. (§2.3 Note)
25. **F-25**: xtask codegen artifacts (`resources/ua-linguistic/derivatives/*.json.gz`, `dict/generated_cognates.rs`) are **not regenerated in CI** — drift between source and artifacts undetected. (§8.6)

---

## 11. Status

COMPLETED

---

## 12. Next Actions (Prioritized)

| # | Priority | Action | Effort |
|---|---|---|---|
| 1 | **P0** | Add `.github/workflows/ci.yml` running `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `tsc --noEmit`, `npm run lint` on every PR + push to main | 1 day |
| 2 | **P0** | Add `cargo audit` + `npm audit` step to CI; fail on high-severity advisories | 0.5 day |
| 3 | **P0** | Restrict `fs:**` capability scope to `$HOME/Documents/LitGraph/**` + `$APPDATA/LitGraph/**` + dialog-approved files | 0.5 day |
| 4 | **P0** | Set restrictive CSP in `tauri.conf.json` (see §6 recommended value) | 0.5 day |
| 5 | **P0** | Add macOS + Windows targets to release.yml matrix; add `.dmg`/`.msi` bundle formats | 1 day |
| 6 | **P0** | Install `vitest` + `@testing-library/react` + `jsdom`; add `package.json` `"test": "vitest run"` script; add 3 pilot tests (tauri-commands wrapper, store reducer, ReasoningDialog render) | 2 days |
| 7 | **P1** | Introduce root `Cargo.toml` workspace aggregating litgraph-core + src-tauri + xtask; enable `cargo test --workspace` | 0.5 day |
| 8 | **P1** | Introduce `trait AiClient` + `MockAiClient` impl (Task-4 R3); refactor `LlmBridge` consumers to inject trait | 1 day |
| 9 | **P1** | Write 7 Layer G integration tests using `MockAiClient` (per §9.2 items 9-11, 12-14) | 2 days |
| 10 | **P1** | Add unit tests for `src-tauri/src/commands/{ai,ner,parse_md_full,export,versions,conflict}.rs` (P0 modules with 0 tests) | 3 days |
| 11 | **P1** | Add caching to CI: `Swatinem/rust-cache@v2` + `actions/setup-node@v4` cache | 0.5 day |
| 12 | **P1** | Pin `tauri-apps/tauri-action` to a specific recent release tag (e.g., `@v0.16.0`) | 0.1 day |
| 13 | **P2** | Replace env-specific absolute paths in `integration_tests.rs:299-315` and `examples/test_sfera.rs:4` with `CARGO_MANIFEST_DIR`-relative paths or `option_env!` | 0.5 day |
| 14 | **P2** | Add `xtask` invocation step to CI to regenerate linguistic artifacts and fail if diff | 0.5 day |
| 15 | **P2** | Reduce `tokio` features to `["rt-multi-thread", "macros"]` in both crates | 0.2 day |
| 16 | **P2** | Add Layer G prompt-size snapshot test (§9.2 item 18) | 0.5 day |
| 17 | **P3** | Add `tarpaulin` coverage reporting + Codecov upload | 1 day |
| 18 | **P3** | Add Playwright E2E tests for LitApp user flows (parse → graph → export) | 3 days |

---

**Report saved:** `/home/z/my-project/subagent-reports/16-tests-ci-build.md`
