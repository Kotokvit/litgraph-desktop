# Subagent 01: litgraph-core Linguistic Layer (Lemmatizer + POS + SVO + Dict)

## 1. Scope
- Files inspected: 9 (7 in-scope + `litgraph-core/src/lib.rs` + `litgraph-core/Cargo.toml` for context)
- Total LOC: 12,499
  - `linguistic/mod.rs`: 29
  - `linguistic/lemmatizer.rs`: 334
  - `linguistic/pos_tagger.rs`: 1,412
  - `linguistic/svo_parser.rs`: 397
  - `dict/mod.rs`: 2
  - `dict/cognate.rs`: 67
  - `dict/generated_cognates.rs`: 10,220 (auto-generated PHF data)
  - `lib.rs`: 13, `Cargo.toml`: 25
- Key entry points:
  - `litgraph-core/src/lib.rs:10-11` — `pub mod dict; pub mod linguistic;`
  - `litgraph-core/src/linguistic/mod.rs:27-29` — `pub mod lemmatizer; pub mod pos_tagger; pub mod svo_parser;`
  - `litgraph-core/src/dict/mod.rs:1-2` — `pub mod cognate; pub mod generated_cognates;`
  - `lemmatizer.rs:167` — `pub fn lemmatize(word: &str) -> Vec<LemmaEntry>` (Layer A primary API)
  - `pos_tagger.rs:700` — `pub fn tag_sentence(tokens, candidates) -> Vec<TaggedToken>` (Layer B primary API)
  - `svo_parser.rs:123` — `SvoParser::parse_text(&self, sentence_text: &str) -> Vec<SvoTriplet>` (Layer C primary API)
  - `dict/cognate.rs:35` — `pub fn normalize_token(token: &str) -> Option<(&'static str, f32, SourceType)>` (token normalization API)

## 2. Atomic Inventory

### 2.1 Modules / Files
| File | LOC | Purpose | Public API | Dependencies |
|------|-----|---------|------------|--------------|
| `linguistic/mod.rs` | 29 | Module root + architecture docstring | Re-exports `lemmatizer`, `pos_tagger`, `svo_parser` | None |
| `linguistic/lemmatizer.rs` | 334 | Layer A: word-form → lemma resolution via dict_uk (ВЕСУМ) gzip index | `LemmaEntry`, `lemmatize`, `lemmatize_first`, `is_known`, `index_size`, `is_loaded` | `flate2`, `serde_json`, `dirs`, `std::sync::OnceLock` |
| `linguistic/pos_tagger.rs` | 1,412 | Layer B: 3-pass POS disambiguation (rules → case government → fallback heuristics) | `PosTag`, `PosClass`, `GrammaticalCase`, `Gender`, `Number`, `Animacy`, `Aspect`, `Tense`, `TokenCondition`, `DisambigAction`, `DisambiguationRule`, `CompiledRules`, `TaggedToken`, `tag_sentence`, `tag_word`, `compiled`, `rule_count`, `cases_for_verb` | `flate2`, `regex`, `serde_json`, `dirs`, `crate::linguistic::lemmatizer` |
| `linguistic/svo_parser.rs` | 397 | Layer C: rule-based Subject-Verb-Object triplet extraction | `SvoTriplet`, `SvoPatternRule`, `SvoTemplateData`, `SvoParser` | `flate2`, `serde_json`, `dirs`, `crate::linguistic::lemmatizer`, `crate::linguistic::pos_tagger` |
| `dict/mod.rs` | 2 | Module root | Re-exports `cognate`, `generated_cognates` | None |
| `dict/cognate.rs` | 67 | Cognate dictionary + token normalization (RU↔UK, barbarism, spelling fixes) | `SourceType`, `CognateEntry`, `CognateMap`, `normalize_token` | `phf`, `serde`, `super::generated_cognates` |
| `dict/generated_cognates.rs` | 10,220 | Auto-generated PHF map of 8,507 cognate entries (xtask output) | `pub static COGNATE_MAP: phf::Map<&'static str, CognateEntry>` | `phf`, `super::cognate` |

### 2.2 Public Types / Interfaces
- `LemmaEntry` (`lemmatizer.rs:58`) — One lemma candidate for a word form: `{lemma, pos, paradigm_class}`.
- `PosClass` (`pos_tagger.rs:79`) — Enum of 12 Ukrainian POS classes (`Noun`/`Verb`/`Adjective`/`Adverb`/`Pronoun`/`Numeral`/`Preposition`/`Conjunction`/`Particle`/`Interjection`/`Punctuation`/`Unknown`).
- `GrammaticalCase` (`pos_tagger.rs:97`) — Enum of 7 Ukrainian cases (`v_naz`/`v_rod`/`v_dav`/`v_zna`/`v_oru`/`v_mis`/`v_kly`).
- `Gender` (`pos_tagger.rs:145`) — Enum: `Masculine`/`Feminine`/`Neuter`/`Plural`.
- `Number` (`pos_tagger.rs:153`) — Enum: `Singular`/`Plural`.
- `Animacy` (`pos_tagger.rs:161`) — Enum: `Animate`/`Inanimate`.
- `Aspect` (`pos_tagger.rs:168`) — Enum: `Perfective`/`Imperfective`.
- `Tense` (`pos_tagger.rs:177`) — Enum: `Present`/`Past`/`Future`/`Infinitive`/`Imperative`.
- `PosTag` (`pos_tagger.rs:187`) — Fully disambiguated POS tag: `{class, case, gender, number, animacy, aspect, tense, raw_tag}`.
- `TokenCondition` (`pos_tagger.rs:295`) — One token-matching condition in a rule: `{text, text_regex, postag, postag_regex, inflected, case_sensitive, negate, min, max}`.
- `DisambigAction` (`pos_tagger.rs:369`) — Enum: `ReplaceTag(PosTag)`/`RemoveTagPattern(String, Option<Regex>)`/`FilterByCase(GrammaticalCase)`/`AddTag(PosTag)`/`Immunize`.
- `DisambiguationRule` (`pos_tagger.rs:398`) — Compiled rule: `{id, name, pattern, marker_start, marker_end, action}`.
- `CompiledRules` (`pos_tagger.rs:522`) — Loaded artifact: `{rules, case_government, rule_count}`.
- `TaggedToken` (`pos_tagger.rs:687`) — Output per token: `{word, lemma, selected_tag, candidates, is_disambiguated, applied_rule}`.
- `SvoTriplet` (`svo_parser.rs:24`) — Extracted semantic event: `{actor, verb, target, instrument, location, polarity, confidence}`.
- `SvoPatternRule` (`svo_parser.rs:43`) — Template from UD-Ukrainian-IU: `{verb_lemma, allowed_subject_cases, allowed_object_cases, allowed_instrument_cases, is_transitive, frequency_count}`.
- `SvoTemplateData` (`svo_parser.rs:54`) — Container: `{total_sentences, total_verbs_extracted, patterns: HashMap<String, SvoPatternRule>}`.
- `SvoParser` (`svo_parser.rs:115`) — Engine struct (zero state, `Default`-derivable).
- `SourceType` (`cognate.rs:9`) — Enum: `Barbarism`/`Spelling`/`Grammar`/`Manual`.
- `CognateEntry` (`cognate.rs:17`) — `{target: &'static str, weight: f32, source_type: SourceType}`.
- `CognateMap` (`cognate.rs:23`) — Type alias: `phf::Map<&'static str, CognateEntry>`.
- `COGNATE_MAP` (`generated_cognates.rs:5`) — Static PHF map, 8,507 entries.

### 2.3 Public Functions / Commands
- `PosTag::parse(raw: &str) -> Self` (`pos_tagger.rs:207`) — Parse a dict_uk POS tag string into structured `PosTag`.
- `PosTag::unknown() -> Self` (`pos_tagger.rs:270`) — Construct empty/unknown tag.
- `PosTag::matches_pattern(&self, pattern: &Regex) -> bool` (`pos_tagger.rs:276`) — Regex-match against `raw_tag`.
- `PosTag::matches_prefix(&self, prefix: &str) -> bool` (`pos_tagger.rs:281`) — Exact-prefix match against `raw_tag`.
- `GrammaticalCase::from_tag_code(code: &str) -> Option<Self>` (`pos_tagger.rs:116`) — Parse `"v_naz"` → `Nominative`.
- `GrammaticalCase::to_tag_code(self) -> &'static str` (`pos_tagger.rs:130`) — Reverse of `from_tag_code`.
- `TokenCondition::matches(&self, word: &str, candidates: &[PosTag]) -> bool` (`pos_tagger.rs:333`) — Test token+candidate tags against this condition.
- `DisambiguationRule::try_apply(&self, tokens, candidates, pos) -> bool` (`pos_tagger.rs:410`) — Try matching the rule pattern starting at position `pos`; mutate candidates if matched.
- `CognateEntry::new(target, weight, source_type) -> Self` (`cognate.rs:26`, `const`) — Constructor.
- `lemmatizer::lemmatize(word: &str) -> Vec<LemmaEntry>` (`lemmatizer.rs:167`) — Look up all lemma candidates for a word form.
- `lemmatizer::lemmatize_first(word: &str) -> Option<String>` (`lemmatizer.rs:188`) — Convenience: first lemma only.
- `lemmatizer::is_known(word: &str) -> bool` (`lemmatizer.rs:195`) — Dictionary membership test.
- `lemmatizer::index_size() -> usize` (`lemmatizer.rs:203`) — Total unique word forms in the loaded index.
- `lemmatizer::is_loaded() -> bool` (`lemmatizer.rs:210`) — Trigger load, return true on success.
- `pos_tagger::compiled() -> &'static Option<CompiledRules>` (`pos_tagger.rs:658`) — Lazy-load + cache compiled rules.
- `pos_tagger::tag_sentence(tokens, candidates) -> Vec<TaggedToken>` (`pos_tagger.rs:700`) — Run 3-pass disambiguation pipeline.
- `pos_tagger::tag_word(word: &str, candidates: &[PosTag]) -> TaggedToken` (`pos_tagger.rs:848`) — Convenience: tag single token (no context).
- `pos_tagger::rule_count() -> usize` (`pos_tagger.rs:858`) — Number of loaded rules (0 if artifact missing).
- `pos_tagger::cases_for_verb(verb_lemma: &str) -> Vec<GrammaticalCase>` (`pos_tagger.rs:863`) — Look up allowed cases for a verb lemma.
- `SvoParser::new() -> Self` (`svo_parser.rs:118`) — Construct engine (zero state).
- `SvoParser::parse_text(&self, sentence_text: &str) -> Vec<SvoTriplet>` (`svo_parser.rs:123`) — Tokenize + lemmatize + POS-tag + extract triplets.
- `SvoParser::extract_triplets(&self, tokens: &[TaggedToken]) -> Vec<SvoTriplet>` (`svo_parser.rs:145`) — Extract triplets from pre-tagged tokens.
- `dict::cognate::normalize_token(token: &str) -> Option<(&'static str, f32, SourceType)>` (`cognate.rs:35`) — Look up replacement target + weight + source type.

## 3. Current State
- **What works:**
  - Lemmatizer: full dict_uk gzip index lookup (~1.7M word forms per docstring), lazy `OnceLock` cache, 3-tier fallback path search (dev / user / system), graceful degradation when index absent. 7 tests (5 are index-gated; 2 unconditional).
  - POS tagger: complete 3-pass pipeline (450 LanguageTool rules + 37,728 verb→case entries + fallback heuristics), full Ukrainian morphology type system (12 POS classes, 7 cases, 4 genders, 2 numbers, 2 animacies, 2 aspects, 5 tenses), regex-based pattern matching with `(?i)` for Cyrillic case-insensitivity. 24 tests covering parsing, conditions, rule actions, fallbacks, integration.
  - SVO parser: actor/verb/target/instrument/location extraction with negation detection (`не`/`ні`), genitive-of-negation handling, post-verb actor fallback (Ukrainian flexible word order), confidence scoring with UD-template boost. 3 tests (basic affirmative, negated kill, with instrument).
  - Cognate dict: 8,507 PHF-compiled entries covering RU↔UK manual cognates + LanguageTool UK/RU `replace.txt` + UK `grammar-barbarism.xml`. 2 tests.
  - Resources present: `lemma_index.json.gz`, `pos_rules.json.gz`, `svo_templates.json.gz` all exist at `resources/ua-linguistic/derivatives/` (verified via LS).
- **What's stubbed:**
  - `DisambigAction::Immunize` (`pos_tagger.rs:468-470`) is a no-op — comment says "Mark by leaving a single tag (no-op for now; future: add a flag)". No immunization state is tracked on `TaggedToken`.
  - POS tagger fallback heuristics (`pos_tagger.rs:811-845`) only SORT candidates by class priority — they don't actually disambiguate (still leave multiple candidates). The `is_disambiguated` flag is `cands.len() == 1`, which fallbacks don't achieve.
  - `svo_templates()` template data is loaded but only used to BUMP confidence by 0.05 if the verb lemma appears in the pattern map (`svo_parser.rs:254-258`). The `allowed_subject_cases`, `allowed_object_cases`, `allowed_instrument_cases`, `is_transitive`, and `frequency_count` fields of `SvoPatternRule` are NEVER consumed for validation/filtering.
  - `TokenCondition.min`/`max` fields (for LanguageTool XML `<token min="1" max="3">` quantifiers) are deserialized and stored but never consulted in `TokenCondition::matches()` (`pos_tagger.rs:333-364`) — quantifier matching is incomplete.
- **What's missing:**
  - No real tokenizer: `SvoParser::parse_text` uses naive `split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '\u{2019}')` (`svo_parser.rs:124-127`). No sentence boundary detection, no hyphen handling, no smart-quote normalization beyond U+2019.
  - No sentence splitter — `parse_text` accepts a single "sentence text" but does no internal segmentation if a multi-sentence string is passed.
  - No anaphora resolution / coreference — "він" (he) is never resolved to a named character; SVO actor becomes the literal token "він".
  - No multi-token named entity support — "Стара Марта" is treated as two separate tokens, only one of which becomes the actor.
  - `PosTag::parse` drops verb person (1st/2nd/3rd) and mood — the `"3"` in `"verb:pres:3:s:imperf"` is silently discarded.
  - `PosTag::parse` only handles `m` or `"m:v_naz"` exact match for Masculine (`pos_tagger.rs:245`); compound codes like `"m:v_rod"` are not recognized as Masculine.
  - `SourceType::Grammar` variant exists but xtask never generates entries with this source type — dead enum variant.
  - Phrase-level cognates are mixed into the per-token cognate map (e.g., `"з приводу" → "надання їм хабара"`, `"оптом і" → "уроздріб"`, `"стрибнув не" → "вагаючись"` at `generated_cognates.rs:10197-10199`). These can never be matched by `normalize_token` (which operates on a single token).
  - No way to query "second-best" candidate from `TaggedToken` — only `selected_tag` (first) and full `candidates` Vec.

## 4. Gaps / Bugs / TODOs
- [BUG] `litgraph-core/src/dict/cognate.rs:35-39` — `normalize_token()` lacks the pronoun-exclusion fix that exists in `src-tauri/src/dict/cognate.rs:35-46`. The src-tauri version explicitly returns `None` for 17 RU/UK pronoun tokens (`"он"|"она"|"оно"|"они"|"він"|"вона"|"воно"|"вони"|"я"|"ти"|"мы"|"вы"|"ми"|"ви"|"це"|"ця"|"цю"|"цей"|"цим"`) to prevent the cognate map from spuriously mapping, e.g., `"вона"` → `"листуватися"` (via LanguageTool replace.txt data). The canonical `litgraph-core` version has the regression — any caller using `litgraph_core::dict::cognate::normalize_token` directly (rather than the src-tauri re-export) hits the bug. The two files exist as duplicates with diverged behavior.
- [BUG] `pos_tagger.rs:711-714` — Dead code inside hot loop: `let before: Vec<usize> = (0..tokens.len()).filter(...).collect(); let _ = before; // for debugging`. This allocates a `Vec<usize>` on every `(rule, pos)` iteration — O(N × R × N) wasted allocations. R=450, N=20-tokens → 180k allocations per `tag_sentence` call. Should be deleted.
- [STUB] `pos_tagger.rs:468-470` — `DisambigAction::Immunize` is a no-op. LanguageTool rules can mark tokens as immunized (locked from further modification), but this implementation does nothing — subsequent rules can still mutate "immunized" tokens.
- [TODO] `pos_tagger.rs:244-262` — `PosTag::parse` gender parsing only handles `"m"` or `"m:v_naz"` exact match for Masculine. dict_uk sometimes emits compound codes like `"m:v_rod"`, `"m:v_dav"` as a single field — these would not be recognized as Masculine. Need to split on `':'` first or use prefix matching.
- [TODO] `svo_parser.rs:275-318` — `candidates_for_word()` heuristic fallback (used when lemmatizer is unloaded) covers only 5 suffix patterns: `-ла/-в/-ти/-ть` (verbs), `-а/-я` (fem nom / masc acc-gen), `-у/-ю` (fem acc / masc dat), `-ом/-ем/-ею/-ою` (instrumental), capitalized (proper noun). Misses: neuter `-е/-я`, plural `-и/-і`, past feminine other than `-ла`, past neuter `-ло`, past plural `-ли`, future tense, conditional, etc.
- [TODO] `svo_parser.rs:253-258` — SVO template data is loaded but only used for +0.05 confidence bump. The `allowed_subject_cases`, `allowed_object_cases`, `allowed_instrument_cases`, `is_transitive`, `frequency_count` fields of `SvoPatternRule` are never read. Major untapped validation potential.
- [TODO] `lemmatizer.rs:87-109` — `locate_index_file()` checks 3 paths (dev / user / system), but does NOT check `"../resources/ua-linguistic/derivatives/lemma_index.json.gz"` (running from `litgraph-core/`). Inconsistent with `pos_tagger.rs:532-538` which DOES check `"../resources/..."`. If a user runs `cargo test` from `litgraph-core/`, the lemmatizer index will not be found.
- [TODO] `pos_tagger.rs:530-551` — `locate_artifact()` has 4 candidate paths but candidates #2 and #3 are byte-identical (`"../resources/ua-linguistic/derivatives/pos_rules.json.gz"`). Duplicate logic.
- [BUG] `svo_parser.rs:165-166` — Negation detection window: `i > 0 && is_negation_word(&tokens[i - 1].word)` OR `i > 1 && is_negation_word(&tokens[i - 2].word)`. The 2-token lookback is too generous — it would mark "Петро не дуже вбив" as negated (correct), but also "Петро дуже не вбив" (where "не" is 2 tokens back) — which may be intended but is fragile. Also misses negation at distance >2 (e.g., "Петро, здавалось, не вбив" — "не" is 3 tokens back due to inserted clause).
- [STUB] `svo_parser.rs:194-196` — Fallback actor `"Хтось"` ("Someone") is hardcoded for verbs with no detectable subject. This produces phantom actors that pollute the narrative graph (Layer E) and downstream LLM prompts (Layer G). Should be `None` or a configurable placeholder.
- [TODO] `pos_tagger.rs:185-197` — `PosTag` does not preserve verb person (1st/2nd/3rd). The `"3"` in `"verb:pres:3:s:imperf"` is silently dropped during parsing. This loses information needed for anaphora resolution (3rd person → potential coreference with previous subject).
- [TODO] `generated_cognates.rs:10197-10199` and others — Phrase-level replacements from LanguageTool (`"з приводу" → "надання їм хабара"`, `"оптом і" → "уроздріб"`, `"стрибнув не" → "вагаючись"`, `"це" → "становить виняток"`) are mixed into the per-token cognate map. `normalize_token` operates on a single token, so these multi-word keys can never be matched. They should be split into a separate phrase-level dictionary.
- [TODO] `pos_tagger.rs:707-722` — Rule application loop iterates `for rule in rules { for pos in 0..tokens.len() {...} }`. Once a rule fires and modifies candidates, subsequent rules at the same `pos` see modified candidates — but there's no short-circuit when a token has only 1 candidate left (i.e., already disambiguated). Performance opportunity.
- [BUG] `pos_tagger.rs:736-748` — `tag_sentence` builds `TaggedToken` with `candidates: cands.clone()` AFTER all 3 passes have run. This means `TaggedToken.candidates` reflects the post-rule candidate list, not the original input candidates. There's no way to inspect "what were the original POS candidates before disambiguation" from a `TaggedToken`.

## 5. Refactoring Opportunities
- [REFACTOR] **Consolidate the three `locate_*_file()` functions** (`lemmatizer.rs:87-109`, `pos_tagger.rs:531-551`, `svo_parser.rs:62-83`) into a single shared `crate::resources::locate(filename: &str) -> Option<PathBuf>` helper. Saves ~60 LOC, eliminates the path-list inconsistency bug, ensures all three layers check the same search paths. S effort.
- [REFACTOR] **Extract `generated_cognates.rs` to a runtime-loaded `.gz` artifact** (like `lemma_index.json.gz`). Currently 10,220 LOC of pure PHF data bloats compile time and `litgraph-core`'s binary size by ~1 MB. A `cognates.json.gz` would load in ~3ms via `OnceLock`. L effort.
- [REFACTOR] **Split `tag_sentence`** (`pos_tagger.rs:699-751`) into 4 focused functions: `apply_pattern_rules()`, `apply_case_government()` (already extracted but takes raw params), `apply_fallbacks()` (already extracted), and `build_tagged_tokens()`. The current function does too much in one body. S effort.
- [REFACTOR] **`PosTag::parse`** (`pos_tagger.rs:207-267`) — Split the 60-line nested match into `parse_class(&str)`, `parse_case(&str)`, `parse_gender(&str)`, etc. Improves unit-testability of each morphological dimension. S effort.
- [REFACTOR] **Move `candidates_for_word()` SVO fallback** (`svo_parser.rs:275-318`) to a data-driven config: `static SUFFIX_CANDIDATES: &[(&str, &[&str])] = &[("ла", &["verb:past:f:s:imperf", ...]), ...]`. Easier to extend with new declension patterns. S effort.
- [REFACTOR] **Make `CompiledRules` private** (`pos_tagger.rs:522-528`) — It's `pub` but only constructed internally and only accessed via `tag_sentence` and `cases_for_verb`. Encapsulate behind methods. S effort.
- [REFACTOR] **Standardize error handling** — `load_index()` returns `Result<Index, String>` (lemmatizer.rs:116), `compile_rules()` returns `Option<CompiledRules>` (pos_tagger.rs:554), `load_svo_templates()` returns `Result<SvoTemplateData, String>` (svo_parser.rs:85). Pick one (preferably `thiserror`-typed errors, since `thiserror = "1"` is already in Cargo.toml). S effort.
- [REFACTOR] **Extract integration tests** — `lemmatizer.rs:214-334` (121 LOC, 7 tests) and `pos_tagger.rs:866-1411` (545 LOC, ~24 tests) bloat the source files. Move to `litgraph-core/tests/lemmatizer_integration.rs` and `litgraph-core/tests/pos_tagger_integration.rs`. S effort.
- [REFACTOR] **Consolidate `src-tauri/src/dict/cognate.rs`** into `litgraph-core/src/dict/cognate.rs` — port the pronoun-exclusion fix upstream, delete the src-tauri duplicate. Eliminates the divergence bug and removes one source of truth confusion. S effort.
- [REFACTOR] **`TokenCondition`** (`pos_tagger.rs:294-365`) — Has 9 fields including pre-compiled `Regex` objects. Custom `PartialEq`/`Eq` skip Regex (lines 313-324). Consider `#[non_exhaustive]` + Builder pattern; or `Box<str>` for `text`/`postag` to allow `derive(PartialEq, Eq)`. M effort.
- [REFACTOR] **Extract phrase-level cognates** to a separate `phrase_cognates.rs` module with a `normalize_phrase(tokens: &[&str]) -> Option<...>` API. The current mixing of single-token and multi-token entries in `COGNATE_MAP` is misleading. M effort.

## 6. Layer G Relevance
- **SVO triplets are the primary structured input to Layer G (LLM Reasoning Bridge)**: `ManuscriptAnalysis::triplets_per_chapter` (`reasoning/mod.rs:89`) bundles `Vec<Vec<SvoTriplet>>` and passes it to `ConflictAnalyzer::analyze()`. The narrative graph (Layer E) uses these triplets to build the character adjacency matrix `A_POS`, whose Frobenius norm `Ω_conf` and spectral radius `ρ(A_POS)` feed the `ε_climax` formula. Layer G consumes both the structured triplets (for prompt context) and the derived conflict metrics (for paradox hypothesis generation).
- **Cognate `source_type` directly modulates Layer G trust**: `src-tauri/src/reasoning/semantic_parser.rs:1358-1380` multiplies confidence by `0.95` (Spelling), `0.90` (Barbarism), `0.92` (Grammar), `1.0` (Manual) based on `crate::dict::cognate::SourceType`. So the dict layer's classification of an entity mapping (manual vs. auto-extracted from LanguageTool) directly controls how much the LLM should trust the mapping.
- **POS tagger identifies speech verbs for paradox detection**: `reasoning/paradox.rs:67-77` hardcodes a list of speech-verb lemmas (`сказати`, `відповісти`, `промовити`, `крикнути`, etc.) used to detect DeadSpeaking paradoxes (a deceased character speaking in a later chapter). This list is currently **decoupled from the POS tagger** — it should ideally be derived from a verb-class lexicon queried via `pos_tagger::cases_for_verb()` plus a speech-verb tag, so that Layer G gets structured "this is a speech verb" signals rather than a parallel static list. Layer G's paradox hypotheses (flashback / dream / resurrection / disguise) are triggered by these signals.
- **Lemmatizer produces canonical forms for LLM prompts**: The lemma string (`ходити`, `бити`, `бути`) is the form embedded in Layer G prompts — the LLM never sees inflected forms (`ходив`, `вбила`, `є`) in the structured context. This keeps prompt tokens stable across conjugations.
- **`TaggedToken.is_disambiguated` is currently UNUSED by Layer G**: This field (`pos_tagger.rs:740`, set when `cands.len() == 1`) could flag low-confidence tokens for the LLM bridge ("this token's POS is uncertain — consider re-tagging with context X"). Currently no reasoning code reads this field — a clear opportunity for tighter Layer B → Layer G integration.
- **`SvoTriplet.polarity` drives paradox detection**: Negated actions (`polarity == false`) are filtered out of the `A_SVO` climax boost (`parser/epsilon.rs:423-427`: `svo_triplets.iter().filter(|t| t.polarity)`) — so the SVO parser's negation detection directly affects the climax score that Layer G uses to prioritize which scenes to reason about.
- **No Layer G → linguistic feedback loop**: The data flow is strictly one-way (linguistic → reasoning → LLM). A future enhancement could let the LLM re-invoke the POS tagger with refreshed candidates after a hint (e.g., "try parsing this as imperative mood").

## 7. Recommended Next Actions
1. **Fix the cognate divergence bug** — Port the pronoun-exclusion fix (17 RU/UK pronouns) from `src-tauri/src/dict/cognate.rs:35-46` to `litgraph-core/src/dict/cognate.rs:35-39`, then delete the src-tauri duplicate. Eliminates the regression and removes duplication. — **S effort**
2. **Wire SVO template validation** — Use `SvoPatternRule.allowed_object_cases` / `is_transitive` / `allowed_subject_cases` to FILTER triplets in `extract_triplets`, not just bump confidence by 0.05. Drop triplets where the actor's case is not in `allowed_subject_cases` for the verb. — **S effort**
3. **Delete dead code in `tag_sentence`** — Remove the `let before: Vec<usize> = ...; let _ = before;` block at `pos_tagger.rs:711-714`. Eliminates O(N×R×N) wasted allocations per call. — **S effort**
4. **Extract `locate_resource()` helper** — DRY the three near-identical file-location functions into a shared `crate::resources::locate(filename)` helper. Ensures consistent search paths across all three layers (fixes the lemmatizer's missing `../resources/` path). — **S effort**
5. **Implement `Immunize` action** — Add an `immunized: bool` field to `TaggedToken` (or a separate `BitVec`), set it when `DisambigAction::Immunize` fires, and skip immunized tokens in subsequent rule passes. Restores fidelity to LanguageTool's disambiguation semantics. — **M effort**
6. **Replace hardcoded `SPEECH_MARKERS` in paradox.rs** — Move speech-verb lexicon to a data file (or derive from POS tagger + verb-class tag) so Layer G gets structured signals. Eliminates parallel static list. — **M effort**
7. **Surface `TaggedToken.is_disambiguated` to Layer G** — Let the LLM bridge request re-tagging for low-confidence tokens. Add a method `TaggedToken::needs_context() -> bool` returning `!self.is_disambiguated`. — **M effort**
8. **Move `generated_cognates.rs` to runtime `.gz` artifact** — Saves ~10,220 LOC of compile-time PHF data, reduces binary size by ~1 MB, makes cognate updates possible without recompiling. Requires adding a `cognates.json.gz` build step to xtask. — **L effort**
9. **Add multi-token named entity support to SVO extraction** — Detect character names like "Стара Марта" or "Іван Франко" as single actor/target entities. Requires integration with the NER layer (Python `ner_extract.py` or its Rust port). — **L effort**
10. **Fix `PosTag::parse` gender handling** — Split compound codes like `"m:v_naz"` on `':'` before matching, or use prefix matching for gender codes. Currently `"m:v_rod"` is not recognized as Masculine. — **S effort**

## 8. Dependencies / Blockers
- **Depends on (runtime):**
  - xtask build artifacts at `resources/ua-linguistic/derivatives/`:
    - `lemma_index.json.gz` (built by `xtask build-lemmatizer`, consumes dict_uk / ВЕСУМ corpus)
    - `pos_rules.json.gz` (built by `xtask build-pos-tables`, consumes LanguageTool UK `disambiguation.xml` + `case_government.txt`)
    - `svo_templates.json.gz` (built by `xtask build-svo-templates`, consumes UD-Ukrainian-IU treebank)
    - All three verified present via LS.
  - Crates: `phf = "0.11"`, `flate2 = "1.0"`, `regex = "1.10"`, `serde`/`serde_json`, `dirs = "5"`. Note: `unicode-segmentation = "1.11"` and `petgraph = "0.6"` are in `litgraph-core/Cargo.toml` but NOT used by the linguistic layer (used by parser and reasoning respectively).
- **Depends on (build-time):** xtask fetches LanguageTool UK/RU `replace.txt` and UK `grammar-barbarism.xml` from GitHub at build time (`xtask/src/main.rs:91-105`). Requires network access for first build; subsequent builds use cached `generated_cognates.rs`.
- **Blocks:**
  - Layer D (`parser/epsilon.rs:420`) — `compute_epsilon_canonical` calls `SvoParser::new().parse_text(chapter_text)` to compute `A_SVO` term in the ε formula.
  - Layer E (`reasoning/mod.rs:47`, `reasoning/narrative_graph.rs:31`, `reasoning/paradox.rs:26`, `reasoning/stub.rs:78`) — All reasoning modules consume `SvoTriplet` as input.
  - Layer G (`src-tauri/src/reasoning/semantic_parser.rs:146, 1241-1280, 1358-1380, 2459, 2550, 2725`) — Consumes `SourceType`, `normalize_token()` for entity mapping with confidence weighting. Note: src-tauri uses its own `crate::dict::cognate` (with pronoun fix), NOT `litgraph_core::dict::cognate`.
  - `src-tauri/src/linguistic/mod.rs` — Re-export shim: `pub use litgraph_core::linguistic::{lemmatizer, pos_tagger, svo_parser};` — so any change to litgraph-core linguistic modules automatically affects src-tauri.
  - Frontend: SVO triplets are surfaced in the UI via `src/components/litgraph/SvoHighlighter.tsx` (per LS).
- **Cross-layer consistency blocker:** The `src-tauri/src/dict/cognate.rs` duplicate (with pronoun fix) vs. `litgraph-core/src/dict/cognate.rs` (without fix) means callers depending on `litgraph_core::dict::cognate::normalize_token` get different behavior than callers using `crate::dict::cognate::normalize_token` (from src-tauri). This must be resolved before Layer G can reliably consume cognate mappings.
