# UA Linguistic Resources — Composite Symbolic NLP Stack for LitGraph

This directory contains **external open-source linguistic resources** for the Ukrainian language, used by LitGraph as the **foundation of the symbolic UA-LP engine** (Layers A/B/C in the architecture). Resources are downloaded at build time and not committed to the repository (see `.gitignore`).

All resources are **LGPL** or **CC BY-SA** licensed. See each subdirectory for LICENSE.

---

## Architecture: 4-Layer Symbolic Pipeline

```
[Manuscript text]
        ↓
[Layer A: Lemmatization]      ← dict_uk (ВЕСУМ) + LanguageTool derivats.txt
        ↓
[Layer B: POS + Disambig]     ← LanguageTool disambiguation.xml + case_government.txt
        ↓
[Layer C: SVO Parser]         ← UD Ukrainian-IU treebank (as rule templates, not ML)
        ↓
[Layer D: POLER ε]            ← LitGraph native (CANON_ANCHORS, EMOTIONAL_MARKERS, ACTION_VERBS)
        ↓
[ε, ε_climax, J-matrix, canon]
```

**Philosophy:** Symbolic AI (Newell, Simon, Chomsky) — every step is a deterministic rule, no ML weights, no neural networks. We glue together thousands of human-hours of UA-linguistic work (LanguageTool team: Andriy Rysin; brown-uk team: Mykola Rykov et al.; UD-Ukrainian: Natalia Kotsyba, mova.institute) into a single desktop LP-engine.

---

## Directory Layout

| Path | Source repo | Size | Purpose |
|---|---|---|---|
| `dict_uk/` | [brown-uk/dict_uk](https://github.com/brown-uk/dict_uk) | 23 MB source | Full UA dictionary: 239,212 lemmas + 16 affix rule sets (POS-tagged paradigms) |
| `languagetool/` | [languagetool-org/languagetool](https://github.com/languagetool-org/languagetool) `uk` module | 6.5 MB | Grammar/style rules + disambiguation + case government + derivatives |
| `ud-ukrainian/` | [UniversalDependencies/UD_Ukrainian-IU](https://github.com/UniversalDependencies/UD_Ukrainian-IU) | 17 MB | Gold-standard dependency treebank: 7,000 sentences, 122K tokens |
| `derivatives/` | (generated) | — | Build artifacts: lemmatizer index, POS tables, SVO templates — produced by `xtask build-ua-resources` |

---

## Resource Inventory & File Roles

### `dict_uk/` — ВЕСУМ (Великий електронний словник української мови)

| File | Role |
|---|---|
| `data/dict/base.lst` | **239,212 lemmas** with POS tags, e.g. `абонувати /v1` = verb, paradigm class 1 |
| `data/dict/base-abbr.lst` | Abbreviations |
| `data/dict/base-compound.lst` | Compound words |
| `data/dict/colors.lst` | Color names |
| `data/dict/geo-ukr-koatuu.lst` | Ukrainian toponyms (KOATUU codes) |
| `data/dict/exceptions.lst` | Irregular forms |
| `data/affix/v.aff` | **Verb paradigms**: rules for tense/person/number/imperative/advp forms |
| `data/affix/n1.aff` | Noun declension, 1st declension (feminine -а/-я) |
| `data/affix/n2.aff` | Noun declension, 2nd declension (masculine/neuter) |
| `data/affix/n3.aff` | Noun declension, 3rd declension (feminine -ь) |
| `data/affix/n4.aff` | Noun declension, 4th declension (neuter -я) |
| `data/affix/a.aff` | Adjective declension |
| `data/affix/np.aff` | Proper nouns |
| `data/affix/numr.aff` | Numerals |
| `data/affix/vr.aff` | Reflexive verbs (-ся/-сь) |
| `data/affix/v_impers.aff` | Impersonal verbs |
| `data/affix/v_advp.aff` | Adverbial participles (діеприслівники) |
| `data/sem/` | Semantic tags (rare, bad, slang) |
| `data/stem/` | Stem exceptions |

**Tag system example:** `noun:f:v_rod:v_mis` = noun, feminine, genitive/prepositional case.
- `noun:anim` / `noun:inanim` — animacy
- `:v_naz` (nominative) `:v_rod` (genitive) `:v_dav` (dative) `:v_zna` (accusative) `:v_oru` (instrumental) `:v_mis` (prepositional) `:v_kly` (vocative)

### `languagetool/` — UA rules from LanguageTool

| File | Size | Role |
|---|---|---|
| `lt_disambiguation.xml` | 452 KB | **POS disambiguation rules**: "Мати бачить доньку" → "бачить" is VERB → "Мати" must be NOUN (not verb). Resolves homonymy. |
| `lt_case_government.txt` | 1.3 MB | **Verb → case mapping**: `абонувати v_zna:v_oru:v_dav` = "абонувати щось/чим/чому". Critical for SVO extraction. |
| `lt_derivats.txt` | 3.2 MB | **Derivative word pairs**: `абонувавши абонувати` = "having subscribed" → "subscribe". Reverse index = lemmatization helper. |
| `lt_grammar-barbarism.xml` | 511 KB | Barbarisms / russisms (style violations: "і→я" replacements) |
| `lt_grammar-grammar.xml` | 186 KB | UA grammar rules: case agreement, gender, number |
| `lt_grammar-style.xml` | 254 KB | Style: tautologies, bureaucratese |
| `lt_grammar-punctuation.xml` | 125 KB | Punctuation rules |
| `lt_grammar-spelling.xml` | 89 KB | Spelling rules |
| `lt_common_words.txt` | 148 KB | Most frequent words (for spell-check) |
| `lt_masc_fem.txt` | 4 KB | Male/female name pairs: "Олег↔Ольга" |
| `lt_multiwords.txt` | 2 KB | Multiword expressions: "внаслідок", "нехай" |
| `lt_entities.txt` | 4 KB | Named entities (people, places) |
| `lt_replace.txt` | 303 KB | Replacement suggestions (incorrect→correct) |
| `lt_disambig_dups.txt` | 4 KB | Duplicate-form cleanup rules |
| `lt_disambig_remove.txt` | 33 KB | POS-tag removal rules (when to discard one reading) |
| `lt_added.txt`, `lt_added_custom.txt` | 1 KB | Custom added words |
| `lt_removed.txt`, `lt_removed_custom.txt` | 0.2 KB | Words removed from dictionary |
| `lt_dash_*.txt` | 11 KB | Hyphenated word rules |

### `ud-ukrainian/` — UD Treebank (Gold-Standard Syntax)

| File | Role |
|---|---|
| `uk_iu-ud-train.conllu` | 5,496 sentences / 92K tokens (training set) |
| `uk_iu-ud-dev.conllu` | Development set |
| `uk_iu-ud-test.conllu` | Test set |
| `stats.xml` | Corpus statistics |

**CoNLL-U format** (one token per line, fields: ID, FORM, LEMMA, UPOS, XPOS, FEATS, HEAD, DEPREL, DEPS, MISC):
```
2  домі  дім  NOUN  Ncmsln  Animacy=Inan|Case=Loc|Gender=Masc|Number=Sing  6  obl  6:obl  ...
```

**Usage in LitGraph:** NOT as ML training data. Used as **rule templates** — extract patterns like:
- `nsubj(NOUN, NOM) → verb-head` → "subject is in nominative, governs verb"
- `obj(NOUN, ACC) → verb-head` → "object is in accusative, governed by verb"

This gives us ~7,000 hand-annotated examples to **derive SVO rules from**, not to train on.

---

## Build Pipeline

Resources are NOT used directly at runtime. Instead, `xtask build-ua-resources` converts them into compact binary indexes:

```
[dict_uk/data/dict/base.lst + data/affix/*.aff]
        ↓ (xtask build-ua-resources)
[derivatives/lemma_index.bin]   ← word-form → lemma (perfect hash, ~5 MB)
[derivatives/pos_table.bin]     ← lemma → POS paradigm
[derivatives/case_gov.json]     ← verb → allowed cases
[derivatives/svo_templates.json] ← SVO patterns from UD treebank
```

Loaded by `litgraph-core/src/linguistic/` at startup with zero-copy + perfect hash (Rust `phf` crate).

---

## Update Procedure

To refresh resources from upstream:

```bash
cd /home/z/my-project/litgraph-desktop
./scripts/refresh_ua_resources.sh   # (to be created)
```

This re-clones dict_uk, re-downloads LanguageTool UK files, and re-pulls UD-Ukrainian. Then re-run `xtask build-ua-resources` to regenerate binary indexes.

---

## Licensing

- **dict_uk**: LGPL — see `dict_uk/LICENSE`
- **LanguageTool UK resources**: LGPL — see [LT license](https://www.languagetool.org/download/LanguageTool-licensing.pdf)
- **UD Ukrainian-IU**: CC BY-SA 4.0 — see `ud-ukrainian/LICENSE.txt`

LitGraph itself uses these as a library and remains under its own license (see repo root LICENSE).

---

## Why This Approach (Not ML)

1. **Determinism**: Same input → same ε, today and in 5 years. LLM gives ±15% variance.
2. **Transparency**: Every decision traceable to a rule file. "Why is this fragment ε=20?" → because lemma X has weight Y per rule Z.
3. **UA-specific accuracy**: General ML models trained on Common Crawl are 70%+ English; UA morphology (7 cases, 4 declensions, aspect pairs) is edge-case for them. Symbolic rules are UA-native.
4. **Offline**: All data is local. No API calls. Desktop-first.
5. **Composable**: Each layer can be replaced/upgraded independently. New LanguageTool release → re-run build → LitGraph improves automatically.

**Why nobody did this before:** This is integrator work, not research. ML community wants to train models (more academically prestigious). UA-NLP niche is open: LanguageTool does orthography, dict_uk does dictionary, UD-Ukrainian does ML training data. **Nobody glued them into a desktop literary-analysis engine.** That's LitGraph's niche.
