# Subagent 02: litgraph-core Parser Layer (Chapters + Characters + Locations + Themes + Epsilon)

## 1. Scope
- Files inspected: **7** (6 parser files + 1 linguistic_entities data file for context)
- Total LOC: **31,313** (chapters.rs 349 + characters.rs 897 + locations.rs 103 + themes.rs 209 + epsilon.rs 1,001 + mod.rs 2,112 + linguistic_entities.rs 26,642)
- Key entry points:
  - `litgraph-core/src/parser/mod.rs:36` — `pub fn build_graph(markdown, project_title, author) -> Result<ParseResult, ParseError>` — orchestrator invoked by Tauri `parse_md` command
  - `litgraph-core/src/parser/chapters.rs:91` — `pub fn detect(text) -> (Vec<ParsedChapter>, String /*prologue*/)`
  - `litgraph-core/src/parser/characters.rs:262` — `pub fn detect(text) -> Vec<ParsedCharacter>` (3-signal detector, v0.3.0 + v0.4.2 closed-loop)
  - `litgraph-core/src/parser/locations.rs:18` — `pub fn detect(text) -> Vec<ParsedLocation>`
  - `litgraph-core/src/parser/themes.rs:155` — `pub fn detect(text) -> Vec<ParsedTheme>` (NOTE: module is commented out in `mod.rs:7` — dead code)
  - `litgraph-core/src/parser/epsilon.rs:329` — `pub fn compute_epsilon(...)`
  - `litgraph-core/src/parser/epsilon.rs:346` — `pub fn compute_epsilon_lemmatized(...)` (v7.0-LEM)
  - `litgraph-core/src/parser/epsilon.rs:472` — `pub fn compute_epsilon_climax(...)` (legacy placeholder)
  - `litgraph-core/src/parser/epsilon.rs:504` — `pub fn compute_epsilon_climax_with_analyzer(...)` (canonical Layer-E climax)
  - `litgraph-core/src/parser/mod.rs:506` — `pub fn lemmatize_simple(word)` (the crude stemmer used by both `characters::detect` and `locations::detect`)

## 2. Atomic Inventory

### 2.1 Modules / Files

| File | LOC | Purpose | Public API | Dependencies |
|------|-----|---------|------------|--------------|
| `parser/mod.rs` | 2,112 | Orchestrator `build_graph()` — wires chapters → characters → locations → edges → ε-metas → layout. Also exposes `lemmatize_simple`, `ALIASES`, `EXTENDED_ALIASES`, `merge_aliases`, `lemmatize_ukrainian`, `generate_ukrainian_declensions`, `looks_like_russian_*` case predicates, `RussianCase`/`RussianGender`/`RussianNumber` enums + `RUSSIAN_NOUN_CASE_ENDINGS` table (52 entries), `RU_UK_COGNATE_PAIRS` table (~110 pairs), wrappers around `crate::languagetool_weights` and `crate::linguistic_entities`. | `build_graph`, `new_uid`, `lemmatize_simple`, `lemmatize_ukrainian`, `generate_ukrainian_declensions`, `looks_like_russian_genitive/dative/instrumental/prepositional`, `find_russian_tautology`, `find_ukrainian_barbarism`, `russian_paronym_correction`, `russian_collocation_correction`, `languagetool_rules_count`, `detect_russian_case_by_ending`, `detect_russian_case_with_gender_number`, `find_cognate_pair`, `is_ru_known_name`, `is_uk_known_name`, `find_ru_replacement`, `find_uk_replacement`, `find_ru_word_root_tautology`, `is_ru_weekday/month/profession/color/nation/human_quality/vvodnoe`, `total_replacement_entries`, `total_word_root_entries`, `EXTENDED_ALIASES`, `all_aliases`, `merge_aliases`, `RUSSIAN_NOUN_CASE_ENDINGS`, `RU_UK_COGNATE_PAIRS`, `ALIASES` | `fancy_regex`, `chrono`, `uuid`, `thiserror`, `serde_json`, `crate::models`, `crate::languagetool_weights`, `crate::linguistic_entities` |
| `parser/chapters.rs` | 349 | Chapter detection: 9 regex patterns (RU/UK/EN line-anchored + markdown-hash) with sub-chapter letter suffix support (e.g. `28б`). Fallback to non-anchored regex if no match. Includes title-cleaning pipeline that strips `«(Робоча назва)», «(Фінальна версія)», «Арка …», «Локація: …», «Місце дії: …»` etc. | `ParsedChapter`, `detect` | `fancy_regex`, `std::collections::HashMap` |
| `parser/characters.rs` | 897 | Multi-signal character detector. v0.3.0 introduced speech-verb + direct-address filter; v0.4.0 added `EntityType` (Character/Organization/Concept) classification with `ORG_CONTEXT_WORDS`; v0.4.2 closed the diagnostic loop with `ABSTRACT_NOUNS` reclassification + low-speech-ratio + delete-weak rules. | `ParsedCharacter`, `EntityType`, `STOP_WORDS`, `SPEECH_VERBS` (~70 verbs), `ORG_CONTEXT_WORDS` (~55 words), `detect`, `count_in_text` | `fancy_regex`, `std::collections::{HashMap, HashSet}`, `serde` |
| `parser/locations.rs` | 103 | Location detector: preposition (в/на/біля/під/над/за/около/под/возле/перед/in/at/on/near/under/over/behind) + Capitalized word. Groups via `super::lemmatize_simple`, truncates to 15. v0.4.0 narrowed preposition list (removed до/із/від/через — caused false-positives from personal names in oblique cases). | `ParsedLocation`, `detect`, `count_in_text` | `fancy_regex`, `std::collections::{HashMap, HashSet}`, `super::characters::STOP_WORDS`, `super::lemmatize_simple` |
| `parser/themes.rs` | 209 | Theme/motif keyword detector. ~80 keywords in UK/RU/EN mapped to ~35 canonical theme names. Word-boundary regex match, threshold `count >= 5`, truncates to 10. | `ParsedTheme`, `THEME_KEYWORDS`, `detect`, `count_in_text` | `fancy_regex`, `std::collections::HashMap` |
| `parser/epsilon.rs` | 1,001 | POLER v7.5-LEM Canonical ε importance formula. Three public entry points (canonical, lemmatized, climax) plus `compute_epsilon_climax_with_analyzer` (Layer E DI). 4 lexicons (EMOTIONAL_MARKERS, CANON_ANCHORS, ACTION_VERBS, STOP_WORDS). 17 unit tests covering log10-vs-ln, clamp bounds, polarity filtering, SVO replacement semantics, Layer E DI determinism. | `EpsilonResult`, `DELTA_BIAS`, `THETA_BASE`, `CLIMAX_THRESHOLD`, `GAMMA_EMO`, `LAMBDA_CONF`, `RARITY_MIN`, `RARITY_MAX`, `build_word_counts`, `compute_epsilon`, `compute_epsilon_lemmatized`, `compute_epsilon_climax`, `compute_epsilon_climax_with_analyzer`, `normalize_epsilons` | `std::collections::{HashMap, HashSet}`, `crate::linguistic::lemmatizer`, `crate::linguistic::svo_parser::SvoParser`, `crate::parser::characters`, `crate::reasoning::{ConflictAnalyzer, ConflictReport}` |
| `linguistic_entities.rs` | 26,642 | LanguageTool-derived flat tables (LGPL v2.1): `RU_REPLACEMENTS` (309), `UK_REPLACEMENTS` (7,915), `UK_REPLACEMENTS_SOFT/RENAMED/SPELLING_2019`, `RU_WORD_ROOTS` (13,126 tautology pairs), `RU_WEEKDAYS/MONTHS/COLORS/NATIONS/PROFESSIONS/HUMAN_QUALITIES/VVODNOE_WORDS/PREP_V_WORDS/PREP_NA_WORDS` etc. 25+ `find_*`/`is_*` lookups. Used by `parser/mod.rs` wrappers. | Many `pub const` tables + 25 `pub fn` lookups | (none — pure static tables) |

### 2.2 Public Types / Interfaces

**`chapters::ParsedChapter`** (chapters.rs:42-50)
```rust
pub struct ParsedChapter {
    pub num: u32,
    pub title: String,
    pub body: String,         // 400-char preview
    pub full_text: String,    // full chapter text
    pub pos: usize,           // byte offset in source
    pub end: usize,           // byte end in source
}
```

**`characters::ParsedCharacter`** (characters.rs:31-50)
```rust
pub struct ParsedCharacter {
    pub name: String,
    pub aliases: Vec<String>,
    pub count: usize,
    pub description: String,
    pub speech_count: usize,   // Signal 2 hits
    pub direct_count: usize,   // Signal 3 hits
    pub reason: String,        // X-ray audit trail: "character:rule=linguistic_signal;freq=...;speech_verb_hits=...;lemma=...;forms=[...]"
    pub entity_type: EntityType,
}
pub enum EntityType { Character, Organization, Concept }  // serde lowercase
```

**`locations::ParsedLocation`** (locations.rs:7-13)
```rust
pub struct ParsedLocation {
    pub name: String, pub aliases: Vec<String>,
    pub count: usize, pub description: String,
}
```

**`themes::ParsedTheme`** (themes.rs:7-12)
```rust
pub struct ParsedTheme { pub name: String, pub count: usize, pub description: String }
```

**`epsilon::EpsilonResult`** (epsilon.rs:171-201)
```rust
pub struct EpsilonResult {
    pub epsilon: f64, pub normalized: f64,
    pub word_count: usize, pub unique_words: usize,
    pub emotion_count: usize, pub kw_count: usize,
    pub canon_count: usize, pub action_count: usize,
    pub theta_rel: f64, pub is_noise: bool, pub is_climax: bool,
    pub formula_variant: &'static str,  // "canonical" | "canonical_lemmatized" | "climax"
}
```

**`parser::ParseError`** (mod.rs:17-25): `Empty | Regex(fancy_regex::Error) | Json(serde_json::Error)`

### 2.3 Public Functions / Commands

| Function | Signature | Where used |
|----------|-----------|------------|
| `parser::build_graph` | `(markdown: &str, project_title: &str, author: &str) -> Result<ParseResult, ParseError>` | `src-tauri/src/commands/parse_md.rs` (Tauri command `parse_md`) |
| `parser::new_uid` | `(prefix: &str) -> String` | storage/mod.rs |
| `parser::lemmatize_simple` | `(word: &str) -> String` | characters.rs, locations.rs, mod.rs (internal) |
| `parser::merge_aliases` | `(chars: Vec<ParsedCharacter>) -> Vec<ParsedCharacter>` | `build_graph` |
| `chapters::detect` | `(text: &str) -> (Vec<ParsedChapter>, String)` | `build_graph` |
| `characters::detect` | `(text: &str) -> Vec<ParsedCharacter>` | `build_graph`, epsilon.rs (Layer E integration) |
| `characters::count_in_text` | `(aliases: &[String], text: &str) -> usize` | `build_graph` (edge building) |
| `locations::detect` | `(text: &str) -> Vec<ParsedLocation>` | `build_graph` |
| `locations::count_in_text` | `(aliases: &[String], text: &str) -> usize` | `build_graph` (edge building) |
| `themes::detect` | `(text: &str) -> Vec<ParsedTheme>` | **Dead** (module commented out in mod.rs:7) |
| `epsilon::build_word_counts` | `(text: &str) -> (HashMap<String, usize>, usize)` | `build_graph` |
| `epsilon::compute_epsilon` | `(chapter_text, global_counts, total_words, keyword: Option<&str>, kappa: f64) -> EpsilonResult` | `build_graph`, Tauri cmd `cmd_compute_epsilon` |
| `epsilon::compute_epsilon_lemmatized` | same signature | Tests only (no production caller in this layer) |
| `epsilon::compute_epsilon_climax` | `(chapter_text, keyword, kappa, omega_conf: f64) -> EpsilonResult` | Tests only (legacy) |
| `epsilon::compute_epsilon_climax_with_analyzer` | `(chapter_text, keyword, kappa, analyzer: &impl ConflictAnalyzer) -> EpsilonResult` | `src-tauri/src/commands/poler.rs::cmd_compute_epsilon_climax` |
| `epsilon::normalize_epsilons` | `(&mut [EpsilonResult])` | `build_graph` |

## 3. Current State

**Working (production-tested, used by `build_graph`):**
- Chapter detection with line-anchored regex + sub-chapter letter suffix (28, 28б, 28в, 28г) — covers RU/UK/EN + 3 markdown-hash variants; v0.4.1 fixed catastrophic backtracking by simplifying patterns.
- Character detection: 3-signal pipeline (frequency → speech-verb → direct-address) with `EntityType` classification and v0.4.2 closed-loop reclassification of `ABSTRACT_NOUNS` to Concept/Organization. Produces human-readable `reason` audit trail for X-ray export.
- Location detection: narrowed preposition list with `lemmatize_simple` grouping (15 max).
- Epsilon canonical formula with δ_bias=15.0, θ_base=3.5, log10 rarity, polarity-filtered SVO replacement of `action_count`. **17 unit tests pass** including the math-strict assertion that `ε_aff - ε_neg = 2.0/√18 ≈ 0.4714`.
- Epsilon climax with Layer-E DI: `compute_epsilon_climax_with_analyzer` calls `characters::detect` + `SvoParser` + `ConflictAnalyzer::analyze_chapter` to derive `Ω_conf` and `I_loc = 1 + canon_count`. Determinism test verifies same-input/same-output.
- Cross-dedup characters ↔ locations using `lemmatize_simple` (handles «Алексея»-loc vs «Алексей»-char).
- Alias merging: `ALIASES` (38 pairs) + `EXTENDED_ALIASES` (~190 pairs) covering RU/UK/Slavic diminutives and patronymics.
- 26K-line LanguageTool weights table is loaded as static `&'static` slices (no runtime allocation).

**Stubbed / Placeholder:**
- `compute_epsilon_climax` (legacy variant without analyzer) — hardcodes `I_loc = 1.0` and `Ω_conf = omega_conf` parameter. Doc itself labels it "Deprecated" and "placeholder".
- In `compute_epsilon_climax_inner`: `Ω_conf = 0.0` if no analyzer is wired; `I_loc = 1.0` in legacy path.
- `Spatial Teleportation Paradox` detection in `crate::reasoning::paradox.rs:90` — explicitly marked *"Not yet implemented. Requires Layer F location normalization to detect non-adjacent location pairs."*
- `themes` module is fully implemented (209 LOC, tests would pass) but `mod.rs:7` has `// pub mod themes; // убран — не нужен` — **dead code, themes are never surfaced in the graph**.

**Missing:**
- Real `Ω_conf` J-matrix integration in Rust (only Python `scripts/build_j_matrix.py` exists; spec doc references `src-tauri/python/build_j_matrix.py` but it's not invoked from `build_graph`).
- Real `I_loc` "canonical anchors intensity" — only `1 + canon_count` heuristic in `compute_epsilon_climax_with_analyzer`; spec wanted it derived from canonical anchor graph.
- Location normalization: `lemmatize_simple` is a naive ending-stripper (returns ≥3 chars of stem), explicitly documented as inadequate for short names ≤4 chars (Рэй/Рэя won't merge without alias map) and stem alternations (Веня ↔ Вениамин).
- No coreference resolution: pronouns («он», «она») are filtered out as STOP_WORDS, so a character's pronoun mentions don't contribute to `count` or `speech_count`.
- No temporal/chapter-aware character tracking: `characters::detect(text)` runs on the entire manuscript at once, then `count_in_text` is invoked per-chapter — this loses chapter-local speech/direct counts.

## 4. Gaps / Bugs / TODOs

### 4.1 `detect_characters` strictness (speech-verb filter) — root cause of db8abf3 test failures

**Context:** commit `db8abf3` ("test(poler): update smoke test strings with speech verbs to satisfy character detector thresholds") patched `src-tauri/src/commands/poler.rs` tests because the original test text `"Петро вбив ворога у бою."` did NOT contain any speech verb, so `detect_characters` returned 0 characters → `compute_epsilon_climax_with_analyzer` was running with empty `detected_chars` → `Ω_conf` was 0 → the assertion `dto.omega_conf >= 0.0` passed vacuously but the test was meaningless. The fix added `"Петро сказав прощання і ..."` so "сказав" triggers Signal 2 and registers Петро as a Character.

**Root cause:** in `characters.rs:357-419`, Signal 2 requires the substring match of a `SPEECH_VERBS` entry. The list (`characters.rs:131-153`) contains ~70 past-tense verb forms (`сказал/сказала/ответил/...`), but **does NOT include**:
- Infinitives: «сказать», «говорить» (only «сказал»/«говорил» match)
- Future tense: «скажет», «ответит»
- Noun-derived speech markers: «слова», «голос», «речь»
- Imperatives: «скажи», «ответь» (only «Скажи» is in STOP_WORDS, so a direct imperative "Скажи, ..." wouldn't fire Signal 2 either way)

**Side effects:**
1. Test authors are forced to remember to include a past-tense speech verb in any text that needs character detection — fragile.
2. Real prose with present/future/infinitive speech verbs produces false negatives: a chapter where characters only speak in present tense («Петро каже: ...») → 0 characters detected → no Character→Chapter edges → empty graph.
3. v0.4.2 ABSTRACT_NOUNS reclassification compounds the issue: if "Архив" appears with `сказал` once (Signal 2 hit, ratio=1/N), it's first classified as Character, then reclassified as Concept by Rule 1 (ABSTRACT_NOUNS). The `reason` audit trail is correct, but if the test asserts `entity_type == Character` it will fail.
4. The `speech_count >= 1 OR direct_count >= 1` filter is binary — there's no confidence score, so a single substring match (`"ответил Имя"` anywhere in 2MB text) is enough to promote a Concept to Character.

**Recommended fix:** expand `SPEECH_VERBS` with infinitive/noun/imperative forms (or use `lemmatize_token` + check against a lemma set `{"сказати", "говорити", "відповідати", ...}`).

### 4.2 Epsilon formula variant v7.5-LEM correctness

The v7.5-LEM implementation has **5 spec-divergent spots** identified by reading `compute_epsilon_inner` (epsilon.rs:357-440):

1. **`I_kw` uses natural log (`ln`), but `rarity(w)` uses `log10`.** Doc comment claims this is intentional ("натуральний логарифм спеціально для згладжування інтенсивності ключового слова"), but the canonical POLER spec uses log10 consistently. Test `test_word_rarity_uses_log10_not_ln` only verifies `rarity`, not `I_kw`.

2. **`compute_epsilon_inner` ignores `_global_counts` and `_total_words` parameters.** They're prefixed with `_` and the `word_rarity` function takes them as `_global_counts: &HashMap<String, usize>` and `_total_words: usize` — but never uses them. Instead, `p_w` is determined by length-heuristic and lexicon membership only. **This means a word's rarity is the same in a 100-word flash fiction and a 200K-word novel** — which contradicts the spec's "rarity = -log10(p_w) where p_w = corpus_frequency / total_words".

3. **`compute_epsilon_climax_inner` (epsilon.rs:571-607) doesn't lemmatize even when called via `compute_epsilon_climax_with_analyzer`.** The climax formula always uses raw tokens, so the LEM optimization (α≈0.7, +9.9% ε) is NOT applied to climax scores. This is asymmetric with the canonical `compute_epsilon_lemmatized` path.

4. **`compute_epsilon_climax_with_analyzer` calls `characters::detect(chapter_text)` on the chapter alone** (epsilon.rs:516-517) — not on the manuscript. This means `EntityType` classification, ABSTRACT_NOUNS reclassification, and alias merging all run on a small per-chapter slice, producing different character sets than `build_graph`'s manuscript-level detection. A character introduced in ch.1 and referenced in ch.2 will be missing from ch.2's analysis.

5. **`I_loc = 1.0 + canon_count_in_chapter`** (epsilon.rs:525-529) counts canon anchors detected as characters, but `CANON_ANCHORS` (epsilon.rs:73-78) are mostly lowercase common nouns («етерія», «буфер», «сектор») — these will NEVER be detected as characters because `characters::detect` only matches Capitalized words. So `canon_count_in_chapter` is effectively always 0 → `I_loc` is always 1.0. The "no longer hardcoded" claim in the docstring is misleading.

6. **`A_SVO` replacement logic** (epsilon.rs:416-423): when `svo_triplets.is_empty()`, falls back to `2.0 * action_count`. When SVO is non-empty, replaces entirely with `2.0 * svo_validated_weight`. The `action_count` field in `EpsilonResult` is populated even when SVO is used, but its contribution is silently dropped — this is correct per spec but confusing.

### 4.3 Location normalization (spatial_teleportation paradox blocker)

`locations::detect` (locations.rs:18-87) groups location candidates by `super::lemmatize_simple(word)` — the same naive stemmer used for characters. Documented limitations (mod.rs:455-475):

- Short names (≤4 chars) returned as-is: «Рэй» and «Рэя» do NOT merge.
- Stem alternations (Веня ↔ Вениамин) not handled.
- Feminine ending special-case (mod.rs:486-503): `Аэлира → "аэлира"` (preserved), but `Аэлире → "аэлир"` (truncated). These do NOT match → **two groups for one location**.

For the **spatial_teleportation paradox** (`reasoning/paradox.rs:90`), the algorithm requires:
1. A canonical location ID per real-world place (so «в Яме» and «на Яму» are the same location).
2. A per-chapter location fingerprint per character (so we can detect "Character X in Y, ch.5 → Character X in Z, ch.6" without transit).
3. A transit-event detector (verbs of motion: «поехал», «прибыл», «дошёл», «перелетел»).

None of these exist. The current `ParsedLocation` has no `chapter_idx`, no `character_co_occurrences`, no `transit_verbs` field. The `locations::count_in_text` (locations.rs:91-103) is just a substring count with **no word-boundary check on the alias** (uses `lower[start..].find(&alias_lower)` and `start = pos + alias_lower.len()`) — this means searching for alias «Ям» would match «Яма», «Яму», «Яме», «Ямы» — but also «Ямка», «Ямщик». The characters::count_in_text does have proper word-boundary byte checks (characters.rs:742-767); locations does NOT — **inconsistent boundary handling**.

### 4.4 Other gaps

- **`themes.rs` is dead code.** `mod.rs:7` comments it out, but the file still exists with 209 LOC and a working `detect` function. Either delete it or wire it into `build_graph` (theme nodes + theme→chapter edges).
- **`build_graph` doesn't expose ε_climax**, only `compute_epsilon` (canonical). The PolerPanel UI (`src/components/litgraph/PolerPanel.tsx`) calls `cmdComputeEpsilonClimax` separately — meaning the user sees two different ε scores for the same chapter.
- **No streaming / chunked parsing.** `build_graph` calls `chapters::detect`, `characters::detect`, `locations::detect` each scanning the entire markdown — for a 2MB manuscript that's 3 full passes plus the SVO/epsilon passes (5+ total). Fancy-regex is SIMD-accelerated but still O(N·R) where R is regex count.
- **`merge_aliases` truncates to 25** (mod.rs:743) — `characters::detect` already truncates to 25, so alias-merged results can be silently dropped if aliases push a character below the cutline.
- **`new_uid` uses `Utc::now().timestamp_millis()`** — if two calls happen in the same millisecond, the UUID v4 suffix disambiguates, but only 8 hex chars (4 bytes = 32 bits) means birthday collisions are possible after ~65K nodes.
- **`detect_russian_case_by_ending` returns `Option<RussianCase>`** but the first match wins by longest-ending heuristic — many Russian endings are case-ambiguous (e.g., `-е` could be Prepositional or Nominative neuter or Dative feminine). No disambiguation context.
- **`RU_UK_COGNATE_PAIRS`** has 2 duplicates: `("Днепр", "Дніпро")` appears at both line 1991 and 2030. Harmless but indicates the table was auto-generated and not deduped.
- **`ALIASES`** has duplicate `("рэю", "рэй")` (mod.rs:592 and 594). Harmless due to HashMap dedup, but it's a code smell.

## 5. Refactoring Opportunities

1. **Consolidate duplicate parser layer.** `litgraph-core/src/parser/` is byte-identical to `src-tauri/src/parser/` (per Subagent 01 finding). Pick one as canonical, the other re-exports via `pub use`. Saves ~2,700 LOC of tech debt.

2. **Extract `lemmatize_simple` into `crate::linguistic::lemmatizer`** as a fallback when `lemmatizer::is_loaded() == false`. Currently both `characters.rs` and `locations.rs` call `super::lemmatize_simple` — coupling them to `parser::mod`. A `linguistic::stem_simple` would decouple.

3. **Replace `Vec<(&'static str, Regex)>` in `chapters::patterns()`** with `OnceLock<Vec<...>>` — currently recompiles 9 regexes on every `chapters::detect` call. For a manuscript parsed on every keystroke (live preview), this is significant.

4. **Same for `characters::detect`'s `re` and `re_sent_end`** — the latter is documented as a v0.4.0 optimization (was inside loop, now hoisted), but `re` is still compiled per-call (characters.rs:264-269).

5. **Hoist `SPEECH_VERBS`, `ORG_CONTEXT_WORDS`, `STOP_WORDS`, `ABSTRACT_NOUNS` to `HashSet`** via `OnceLock<HashSet<&'static str>>` — currently each is a `&[&str]` and `stop.contains(word)` is O(N) linear scan. STOP_WORDS has ~250 entries; called inside `re.captures_iter` loop = O(N·M) per chapter.

6. **`themes.rs` decision**: either delete (saves 209 LOC + maintenance) or wire into `build_graph` as a 4th node type with `theme → chapter` edges. The `THEME_KEYWORDS` table is useful for paradox detection (theme inconsistency across chapters).

7. **`compute_epsilon_inner` should use `_global_counts` and `_total_words`** for real `p_w = count / total_words`. Currently they're dead parameters — calling code computes them via `build_word_counts(markdown)` and passes them in, but they're ignored. Either use them or remove from the signature.

8. **Lift `compute_epsilon_climax_with_analyzer`'s `characters::detect` call to the caller.** Currently each `compute_epsilon_climax_with_analyzer` invocation re-runs character detection on the chapter. For a 50-chapter manuscript, that's 50 separate `detect()` calls. Better: `build_graph` runs `detect(markdown)` once, then per-chapter filters to `characters_mentioned_in_chapter`.

9. **Unify `count_in_text` API.** `characters::count_in_text` does proper word-boundary byte checks; `locations::count_in_text` does naive substring search. Both should use the same boundary-aware function. Bug: «Ям» alias will match «Ямщик» in locations.

10. **`ParsedChapter.title` cleaning pipeline** (chapters.rs:261-326) is 14 sequential `regex_replace` calls, each compiling a new regex. Pre-compile or use a single combined regex.

11. **`EXTENDED_ALIASES` (~190 pairs) and `ALIASES` (38 pairs) overlap.** Doc says `all_aliases()` returns `EXTENDED_ALIASES` because "он уже содержит все пары из ALIASES" — but `merge_aliases` uses `ALIASES`, not `all_aliases()`. Either delete `ALIASES` or actually deprecate it. ~38 LOC of dead constants.

12. **`build_graph` is 200+ lines, does 7 things** (parse chapters, detect characters, merge aliases, detect locations, cross-dedup, build nodes, build edges, layout). Extract into smaller functions: `build_chapter_nodes`, `build_character_nodes`, `build_location_nodes`, `build_flow_edges`, `build_character_chapter_edges`, `build_location_chapter_edges`, `apply_layout`.

## 6. Layer G Relevance

The parser layer is the **primary signal source for Layer G (LLM Reasoning Bridge)**:

1. **Paradox detection inputs:**
   - `chapters::detect` → chapter boundaries + full_text per chapter. Used by `ParadoxDetector` to iterate chapter-by-chapter.
   - `characters::detect` → per-character `entity_type`, `speech_count`, `direct_count`, `aliases`. Used to identify dead-speaking paradoxes (character speaks after death marker).
   - `characters::ParsedCharacter.reason` → audit trail ("character:rule=linguistic_signal;freq=42;speech_verb_hits=8;direct_address_hits=3;lemma=петр;forms=[Петро,Петра,Петру]") — should be embedded verbatim in LLM prompts so the model can verify the parser's classification.
   - `locations::detect` → per-location `aliases` + `count`. Needed for spatial_teleportation paradox (currently unimplemented).
   - `epsilon::EpsilonResult` → per-chapter ε, is_climax, is_noise, formula_variant. Identifies "high-stakes" chapters where the LLM should focus narrative analysis.

2. **World state construction:**
   - `ParsedCharacter.aliases` → coreference set for LLM ("Веня = Вениамин = Вельямін").
   - `ParsedCharacter.entity_type` → tells LLM whether to treat "Архив" as a character, organization, or concept (current v0.4.2 reclassification is heuristic; LLM can override).
   - `ALIASES` + `EXTENDED_ALIASES` → domain knowledge to inject into prompts.
   - `RU_UK_COGNATE_PAIRS` → cross-language entity linking for multilingual manuscripts.

3. **LLM prompt inputs (future):**
   - For **dead-speaking paradox** prompts: include the death-marker sentence, the post-death speech sentence, and the chapter gap. Parser already extracts chapters; SVO parser provides triplets; LLM receives `{paradox.character, paradox.chapter_idx, paradox.origin_chapter_idx, chapter_text(origin), chapter_text(manifest), death_marker_sentence, speech_sentence, svo_triplets_at_manifest}`.
   - For **spatial_teleportation paradox** prompts (when implemented): include per-chapter location sets from `locations::detect` (run per chapter, not per manuscript — currently not supported), character co-occurrence matrix, and the absence of transit verbs. Parser must grow a `per_chapter_locations: Vec<Vec<ParsedLocation>>` field on `ParseResult`.
   - For **climax verification** prompts: include `EpsilonResult.formula_variant`, the lexicon hits (`canon_count`, `emotion_count`, `kw_count`), and ask LLM to confirm/refute the climax classification.

4. **`reason` audit trail as prompt context:** every `ParsedCharacter.reason` string is a structured `key=value;key=value` log. A Layer G prompt template can parse this and ask: "The parser classified X as Concept with rule=abstract_noun_reclassify because it's in ABSTRACT_NOUNS. Review: is X actually a character in disguise?"

5. **Confidence calibration:** `epsilon::EpsilonResult.normalized` (0-100) and `is_climax` boolean can gate LLM invocation — only call the LLM for chapters with `normalized > 75` to control cost.

## 7. Recommended Next Actions

1. **[P0] Expand `SPEECH_VERBS` to lemma set.** Replace the past-tense-only list with a lemma-based check using `crate::linguistic::lemmatizer::lemmatize_first` against `{"сказати", "говорити", "відповідати", "спитати", "промовити", "крикнути", "пробормотати", "прошепотіти", ...}`. This eliminates the db8abf3 class of test fixes and reduces false negatives in present-tense prose.

2. **[P0] Fix `locations::count_in_text` word-boundary bug.** Copy the boundary check from `characters::count_in_text` (characters.rs:742-767). Otherwise alias «Ям» matches «Ямщик», inflating edge counts and producing false-positive location-chapter edges.

3. **[P1] Add per-chapter location detection.** New function `locations::detect_in_chapter(chapter_text) -> Vec<ParsedLocation>` (or extend `ParsedChapter` with `locations: Vec<ParsedLocation>`). Prerequisite for `SpatialTeleportation` paradox detector.

4. **[P1] Implement transit-verb detection.** New module `parser::transit_verbs` with lexicon `{поїхати, приїхати, прибути, дійти, перелетіти, попливти, ...}` + detector that scans chapter text for `character_alias + transit_verb`. Used by paradox detector to suppress false positives ("X in Y ch.5, X in Z ch.6, BUT transit verb present in ch.5 or ch.6 → no paradox").

5. **[P1] Wire real `Ω_conf` into epsilon climax.** Port `scripts/build_j_matrix.py` to Rust (or call it via `std::process::Command` like `ner_extract.py`). The current `NarrativeGraph` analyzer returns `Ω_conf = frobenius_norm(J)` but `J` is built from per-chapter SVO only — needs full-manuscript J matrix with character indices.

6. **[P1] Use `_global_counts` and `_total_words` in `word_rarity`.** Either delete the parameters (and the doc claim of corpus-relative rarity) or actually compute `p_w = global_counts.get(word).copied().unwrap_or(0) / total_words.max(1)`.

7. **[P2] Lemmatize climax path.** `compute_epsilon_climax_inner` should accept `use_lemmatizer: bool` like `compute_epsilon_inner` does. Currently climax scores are always computed on raw tokens, breaking v7.5-LEM symmetry.

8. **[P2] Either delete `themes.rs` or wire it in.** Dead 209-LOC module is a maintenance trap. Recommended: wire in — add `theme → chapter` edges in `build_graph` so authors see "Тема 'Память' встречается в главах 3, 7, 12" visually.

9. **[P2] Consolidate `ParsedCharacter` truncation.** `detect` truncates to 25 (after taking 20 characters + 5 concepts). `merge_aliases` truncates to 25 again. If aliases merge a 26th character upward (e.g., merging 5 short-name aliases bumps a character from rank 30 to rank 5), the rank 25 character is dropped. Either move truncation to the very end of `build_graph` or use a higher cap (50).

10. **[P3] Pre-compile regexes via `OnceLock`.** `chapters::patterns()`, `characters::detect`'s `re` and `re_sent_end`, `themes::detect`'s per-keyword regex. For live-preview parsing, this is a measurable speedup.

11. **[P3] Deduplicate `ALIASES` / `EXTENDED_ALIASES` / `RU_UK_COGNATE_PAIRS`.** Three static tables with overlapping entries; consolidate into one `&[(&str, &str, AliasKind)]` with `AliasKind::Diminutive | Cognate | Patronymic`.

12. **[P3] Add `ParsedLocation.chapter_indices: Vec<usize>` field.** Pre-compute during `build_graph` so the spatial teleportation detector doesn't have to re-scan per chapter.

## 8. Dependencies / Blockers

- **`fancy-regex` 0.13** — used everywhere for lookahead/lookbehind. Standard `regex` crate doesn't support these. v0.4.1 chapters.rs already simplified patterns to avoid catastrophic backtracking; characters.rs still uses `(?<![a-zA-Z\x{0400}-\x{04FF}])` lookbehind which IS the catastrophic-backtracking risk on long texts.

- **`crate::linguistic::lemmatizer`** — optional dependency. `epsilon::compute_epsilon_lemmatized` checks `lemmatizer::is_loaded()` and falls back gracefully. `characters::detect` does NOT use lemmatizer (uses `lemmatize_simple` only) — so character grouping is brittle for short names.

- **`crate::linguistic::svo_parser::SvoParser`** — required by epsilon climax. If SVO parser fails (returns empty), `A_SVO = 2.0 * action_count` (fallback). Currently `action_count` is computed from `ACTION_VERBS` lexicon (35 verbs) — very small coverage.

- **`crate::reasoning::{ConflictAnalyzer, ConflictReport, NarrativeGraph, StubConflictAnalyzer}`** — required by `compute_epsilon_climax_with_analyzer`. Layer E is implemented (commit 37be4b6). `NarrativeGraph` builds J-matrix from per-chapter SVO; `StubConflictAnalyzer` returns configurable `Ω_conf` for testing.

- **`crate::languagetool_weights`** — wrappers in `parser/mod.rs:1145-1180`. Used for `find_russian_tautology`, `find_ukrainian_barbarism`, `russian_paronym_correction`, `russian_collocation_correction`. Not currently invoked by `build_graph` — dead in production path.

- **`crate::linguistic_entities`** — 26K-line static table module. Lookups are O(N) linear scan; for `find_ru_replacement` (309 entries) this is fast enough, but `find_ru_word_root_tautology` (13,126 entries) called per-sentence could be slow. No `phf::Map` integration yet.

- **Python scripts (`build_j_matrix.py`, `ner_extract.py`, `svo_extract.py`)** — invoked via `std::process::Command` from Tauri commands (not from `build_graph`). The parser layer is pure Rust; Python is only used for the heavy NLP pipeline (spaCy + pymorphy3). Layer G LLM prompts would also go through Python or directly through `reqwest` from Rust.

- **Blocker for SpatialTeleportation paradox:** needs (1) per-chapter location detection, (2) location canonicalization (better than `lemmatize_simple`), (3) transit-verb lexicon, (4) character-location co-occurrence matrix. All four are missing. Estimated effort: 2-3 days.

- **Blocker for production ε_climax:** J-matrix is currently per-chapter, not manuscript-wide. Need to port `build_j_matrix.py` (~150 LOC Python) to Rust or shell out to it. Estimated effort: 1 day.

- **Blocker for LLM-driven paradox resolution (Layer G):** the parser must expose structured `ParseResult` extensions: `per_chapter_characters: Vec<Vec<String>>`, `per_chapter_locations: Vec<Vec<String>>`, `per_chapter_svo: Vec<Vec<SvoTriplet>>`, `death_events: Vec<(String /*char*/, usize /*ch*/)>`, `transit_events: Vec<(String /*char*/, usize /*ch*/, String /*from*/, String /*to*/)>`. Currently `ParseResult` only has flat `nodes` and `edges`. Estimated effort: 1 day to extend struct + populate in `build_graph`.
