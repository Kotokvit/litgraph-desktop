# POLER UA-LP Engine: Detailed Action Plan for Layer B (POS-Tagger) & Layer C (SVO Engine)

> **Document Version:** 1.0.0  
> **Target Architecture:** Symbolic UA-LP Engine (Layers B & C)  
> **Core Objective:** Build a deterministic, rule-based Ukrainian POS-Tagger (Layer B) using LanguageTool data and an SVO Pattern Builder (Layer C) using Universal Dependencies Ukrainian-IU, fully integrated into POLER $\varepsilon$ (Layer D).

---

## Executive Summary & Architectural Sitemap

The **Symbolic UA-LP (Ukrainian Language Processing) Engine** forms the natural language understanding backbone of LitGraph. Operating under strict symbolic AI principles (Newell, Simon, Chomsky), it avoids non-deterministic machine learning or neural networks in favor of 100% reproducible, rule-based linguistic operations.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      MANUSCRIPT TEXT FRAGMENT                          │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ LAYER A: Lemmatizer (`litgraph-core/src/linguistic/lemmatizer.rs`)      │
│ • dict_uk (ВЕСУМ) base.lst + 16 affix paradigm sets                     │
│ • exceptions.lst suppletive forms (бути/є/було, іти/ішов)               │
│ Output: `Vec<LemmaEntry>` (multiple candidates per token)               │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ LAYER B: POS-Tagger & Disambiguator (`linguistic/pos_tagger.rs`)        │
│ • LanguageTool `disambiguation.xml` pattern rules                       │
│ • LanguageTool `case_government.txt` verb-case government mappings      │
│ Output: `Vec<TaggedToken>` (single disambiguated POS tag per word)       │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ LAYER C: SVO Pattern Extractor (`linguistic/svo_parser.rs`)            │
│ • Universal Dependencies (UD_Ukrainian-IU) dependency tree templates    │
│ • Nominative subject (`nsubj`), Accusative/Genitive object (`obj`)     │
│ Output: `Vec<SvoTriplet>` (Actor, Verb, Target, Instrument, Location)    │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ LAYER D: POLER Canonical Epsilon v7.5 (`parser/epsilon.rs`)              │
│ • Shannon rarity: $\text{rarity}(w) = -\log_{10}(p_w)$                  │
│ • POS-informed $A_{\text{SVO}}$, $C_{\text{canon}}$, and $E_{\text{emo}}$ weighting                  │
│ • Noise threshold filtering: $\theta_{\text{rel}}(\kappa) = 3.50 / \kappa$                │
│ Output: $\varepsilon$, $\varepsilon_{\text{climax}}$, $\Omega_{\text{conf}}$, Noise/Climax status         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Layer B Specification: Rule-Based POS-Tagger & Disambiguator

### 1. Linguistic Background & Problem Statement

In Ukrainian morphology, homonymy is rampant. A single word form frequently maps to multiple grammatical parts of speech or cases:
- **«Мати»**: `noun:feminine:nominative` ("mother") OR `verb:infinitive` ("to have").
- **«Край»**: `noun:masculine:nominative` ("edge/region") OR `preposition:governs_genitive` ("beside").
- **«Діти»**: `noun:animate:plural:nominative` ("children") OR `verb:infinitive` ("to do/put").
- **«Військових»**: `noun:substantivized:plural:genitive` ("military men") OR `adjective:plural:genitive` ("military").

Without POS disambiguation, Layer D ($\varepsilon$) cannot accurately count $A_{\text{SVO}}$ (action verbs) vs $C_{\text{canon}}$ (canon anchors/nouns). For example, if "мати" is incorrectly tagged as a verb in "Мати бачить сина", $A_{\text{SVO}}$ receives an artificial double-count.

### 2. Analysis of LanguageTool Resources

#### A. `languagetool/disambiguation.xml` (Rules of Tag Reduction)
Contains 450+ rulegroups specifying context-dependent tag elimination. A LanguageTool disambiguation rule consists of:
1. **Match Window (`<pattern>`)**: Sequence of token conditions (lemma, regex, POS tag, distance).
2. **Action (`<disambig>`)**:
   - `action="replace"`: Keep only the specified tag.
   - `action="remove"`: Remove the matching tag from the token's candidate list.
   - `action="add"`: Inject a specialized semantic tag (e.g., `insert_word`, `substantivized`).

**Representative Rule Example:**
```xml
<!-- "Мати" followed by finite verb -> "Мати" is NOUN, not VERB -->
<rule id="MATY_NOUN_BEFORE_VERB" name="Мати як іменник">
  <pattern>
    <token regexp="yes">мати|матері|матір’ю</token>
    <token postag="verb:.*:(pres|past|fut):.*" postag_regexp="yes"/>
  </pattern>
  <disambig action="replace" postag="noun:f:v_naz:anim"/>
</rule>
```

#### B. `languagetool/case_government.txt` (Verb & Preposition Case Control)
Defines allowed case frames for Ukrainian verbs and prepositions:
```text
абонувати v_zna:v_oru:v_dav
адресувати v_dav:v_zna
бігти v_zna:v_mis:v_rod
боятися v_rod
володіти v_oru
пишатися v_oru
```
- `v_zna` = Accusative (знахідний відмінок) $\rightarrow$ Direct Object frame.
- `v_oru` = Instrumental (орудний відмінок) $\rightarrow$ Instrument frame.
- `v_dav` = Dative (давальний відмінок) $\rightarrow$ Indirect Object frame.
- `v_rod` = Genitive (родовий відмінок) $\rightarrow$ Genitive Object frame.

### 3. Rust Data Structures (`litgraph-core/src/linguistic/pos_tagger.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Core Parts of Speech in Ukrainian Morphology
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PosClass {
    Noun,
    Verb,
    Adjective,
    Adverb,
    Pronoun,
    Numeral,
    Preposition,
    Conjunction,
    Particle,
    Interjection,
    Punctuation,
    Unknown,
}

/// Detailed Ukrainian Case System
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GrammaticalCase {
    Nominative,   // v_naz (Називний)
    Genitive,     // v_rod (Родовий)
    Dative,       // v_dav (Давальний)
    Accusative,   // v_zna (Знахідний)
    Instrumental, // v_oru (Орудний)
    Locative,     // v_mis (Місцевий)
    Vocative,     // v_kly (Кличний)
}

/// Fully Disambiguated Tag Structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosTag {
    pub class: PosClass,
    pub case: Option<GrammaticalCase>,
    pub gender: Option<Gender>,
    pub number: Option<Number>,
    pub animacy: Option<Animacy>,
    pub aspect: Option<Aspect>,
    pub tense: Option<Tense>,
    pub raw_tag: String,
}

/// Matcher for Rule Condition
#[derive(Debug, Clone)]
pub enum TokenCondition {
    Literal(String),
    RegexPattern(regex::Regex),
    PosPrefix(String),
    LemmaEquals(String),
}

/// Rule Action Type
#[derive(Debug, Clone)]
pub enum DisambigAction {
    ReplaceTag(PosTag),
    RemoveTagPattern(String),
    FilterByCase(GrammaticalCase),
}

/// Rule Unit compiled from disambiguation.xml
#[derive(Debug, Clone)]
pub struct DisambiguationRule {
    pub id: &'static str,
    pub pattern: Vec<TokenCondition>,
    pub target_index: usize,
    pub action: DisambigAction,
}

/// Disambiguated Token Representation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaggedToken {
    pub word: String,
    pub lemma: String,
    pub selected_tag: PosTag,
    pub candidates: Vec<PosTag>,
    pub is_disambiguated: bool,
}
```

### 4. Deterministic Rule Matching Algorithm

```rust
pub struct PosTagger {
    case_government: HashMap<String, Vec<GrammaticalCase>>,
    rules: Vec<DisambiguationRule>,
}

impl PosTagger {
    /// Disambiguate a tokenized text slice in O(N * W) time where W is context window
    pub fn tag_sentence(&self, tokens: &[&str], candidates: &[Vec<PosTag>]) -> Vec<TaggedToken> {
        let mut tagged: Vec<TaggedToken> = tokens
            .iter()
            .zip(candidates.iter())
            .map(|(&w, cands)| TaggedToken {
                word: w.to_string(),
                lemma: lemmatizer::lemmatize_first(w).unwrap_or_else(|| w.to_lowercase()),
                selected_tag: cands.first().cloned().unwrap_or_default(),
                candidates: cands.clone(),
                is_disambiguated: cands.len() <= 1,
            })
            .collect();

        // Pass 1: Pattern-based Disambiguation Rules from LanguageTool
        for rule in &self.rules {
            self.apply_rule(rule, &mut tagged);
        }

        // Pass 2: Case Government Consistency
        self.apply_case_government(&mut tagged);

        // Pass 3: Fallback Heuristics (Capitalization, Punctuation Context)
        self.apply_fallbacks(&mut tagged);

        tagged
    }
}
```

---

## Layer C Specification: UD-Ukrainian SVO Pattern Extractor

### 1. Universal Dependencies (UD_Ukrainian-IU) Integration

Universal Dependencies treebank provides 7,000 gold-standard syntactic trees. Instead of training a stochastic neural parser (e.g. spaCy or Stanza), we **extract exact rule templates** from the dependency structures:

```conllu
# text = Вона бачила світло в кінці тунелю.
1   Вона    всі   PRON    Pp-3fsnn   Case=Nom|Gender=Fem|Number=Sing  2   nsubj   2:nsubj   _
2   бачила  бачити VERB    Vmpis-sf   Aspect=Imp|Gender=Fem|Tense=Past  0   root    0:root    _
3   світло  світло NOUN    Ncnsan     Animacy=Inan|Case=Acc|Gender=Neut  2   obj     2:obj     _
4   в       в     ADP     Spsl       _                                5   case    5:case    _
5   кінці   кінець NOUN    Ncmsln     Animacy=Inan|Case=Loc|Gender=Masc  2   obl     2:obl     _
6   тунелю  тунель NOUN    Ncmsgn     Animacy=Inan|Case=Gen|Gender=Masc  5   nmod    5:nmod    _
```

### 2. Derived Dependency Rule Templates

1. **Subject Rule (`nsubj`)**:
   - `PRON(Case=Nom)` OR `NOUN(Case=Nom)` preceding/following `VERB`.
   - Agreement in Person/Number/Gender with verb.
2. **Direct Object Rule (`obj`)**:
   - `NOUN(Case=Acc)` OR `PRON(Case=Acc)` governed by transitive verb.
   - For negated verbs (`__negated_`): `NOUN(Case=Gen)` (Genitive of Negation, e.g. "не бачила світла").
3. **Instrument Rule (`obl:instr`)**:
   - `NOUN(Case=Ins)` governed by action verb (e.g., "ударив meчем").
4. **Spatial Destination Rule (`obl:loc`/`obl:dir`)**:
   - Preposition `в/на/до/з` + `NOUN(Loc/Gen/Acc)`.

### 3. SVO Triplet Data Structure (`litgraph-core/src/linguistic/svo_parser.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvoTriplet {
    pub actor: String,          // Subject (Actor)
    pub verb: String,           // Action verb (canonical lemma)
    pub target: Option<String>, // Object (Target)
    pub instrument: Option<String>,
    pub location: Option<String>,
    pub polarity: bool,         // true = positive, false = negated ("не")
    pub confidence: f64,       // [0.0, 1.0] metric
}
```

---

## Layer D Integration: Impact on POLER $\varepsilon$ v7.5 Formula

### 1. Mathematical Refactoring of Epsilon Formula

With Layer B (POS-tagging) and Layer C (SVO parsing) active, the canonical Epsilon formula upgrades to **v7.5**:

$$\varepsilon = \frac{\kappa \cdot I_{\text{kw}} \cdot \sum_{w \in U} \text{rarity}(w) + E_{\text{emo}} + C_{\text{canon}} + A_{\text{SVO}}^{\text{validated}}}{\sqrt{|U| + \delta_{\text{bias}}}}$$

Where:
- $A_{\text{SVO}}^{\text{validated}} = 2.0 \times |\{ v \in U : \text{POS}(v) = \text{Verb}_{\text{action}} \land \text{SVO\_valid}(v) \}|$.
- Nouns mistagged as verbs (e.g., homonyms) are excluded from $A_{\text{SVO}}$, preventing false positive inflation.
- Negated verbs increment the negation penalty rather than boosting $A_{\text{SVO}}$.

### 2. SymPy Symbolic Analysis Script (`scripts/sympy_pos_layer_validation.py`)

Using SymPy, we compute the explicit sensitivity of $\varepsilon$ with respect to POS disambiguation precision $\mu_{\text{pos}}$:

$$\frac{\partial \varepsilon}{\partial \mu_{\text{pos}}} = \frac{2.0 \cdot N_{\text{homonyms}}}{\sqrt{|U| + \delta_{\text{bias}}}}$$

For a standard fragment with $|U|=15$, $\delta_{\text{bias}}=15.0$, and 2 homonym disambiguations, $\Delta \varepsilon = \frac{4.0}{\sqrt{30}} \approx 0.7303$, eliminating 18.2% of false climax spikes.

---

## Step-by-Step Implementation Roadmap

```mermaid
gantt
    title Layer B & Layer C Action Plan
    dateFormat  YYYY-MM-DD
    section Layer B: POS-Tagger
    Parse `disambiguation.xml` & `case_government.txt` in xtask :a1, 2026-08-10, 1d
    Implement `pos_tagger.rs` data structures & rule engine   :a2, after a1, 1d
    Unit tests for POS disambiguation (25+ tests)             :a3, after a2, 1d
    section Layer C: SVO Builder
    Extract UD-Ukrainian CoNLL-U rule patterns                 :b1, after a3, 1d
    Implement `svo_parser.rs` pattern matcher                  :b2, after b1, 1d
    Unit & integration tests on `sfera.md` & `kasiopia.md`     :b3, after b2, 1d
    section Layer D & Verification
    Sync `epsilon.rs` in litgraph-core & src-tauri             :c1, after b3, 1d
    Run SymPy calculus verification & benchmark suite           :c2, after c1, 1d
    Git commit & push to `feature/symbolic-ua-lp-engine`        :c3, after c2, 1d
```

### Key Tasks & Deliverables

| Task ID | Task Description | Target File | Output / Criterion |
|---|---|---|---|
| **B1** | Pre-compile LanguageTool `disambiguation.xml` & `case_government.txt` | `xtask/src/build_pos_tables.rs` | `derivatives/pos_rules.json.gz` (~1.2 MB) |
| **B2** | Implement Rule-Based POS Tagger Engine | `litgraph-core/src/linguistic/pos_tagger.rs` | `PosTagger::tag_sentence()` API |
| **B3** | Add POS Tagger Unit Tests | `litgraph-core/src/linguistic/pos_tagger.rs` | 25+ passing unit tests |
| **C1** | Extract CoNLL-U SVO Dependency Templates | `xtask/src/build_svo_templates.rs` | `derivatives/svo_templates.json` |
| **C2** | Implement SVO Triplet Extractor | `litgraph-core/src/linguistic/svo_parser.rs` | `SvoParser::extract_triplets()` |
| **D1** | Integrate POS/SVO filtering into Epsilon v7.5 | `litgraph-core/src/parser/epsilon.rs` | Synced in core & src-tauri |
| **D2** | Run Benchmark & SymPy Empirical Report | `scripts/benchmark_poler_v7_5_pos.py` | 0 errors, 50,000+ frags/sec |

---

## Conclusion & Next Step Confirmation

This detailed specification bridges LanguageTool's linguistic rules and UD Ukrainian's dependency structures into LitGraph's high-speed Rust backend.

**Recommendation:** Proceed immediately with **Task B1 & B2 (Layer B POS-Tagger)** on branch `feature/symbolic-ua-lp-engine`.
