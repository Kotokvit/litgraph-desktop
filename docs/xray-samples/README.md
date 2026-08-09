# X-ray HTML Samples

Interactive HTML X-ray exports of LitGraph workspaces — self-contained `.html` mini-programs
that recreate the GUI (pan/zoom canvas + sidebar with full meta / reason / SVO) in any browser.

## Files

| File | Source | Nodes | Edges | Notes |
|------|--------|-------|-------|-------|
| `1-сфера-предела.xray.html` | «Сфера Предела» (172 709 words, 36 chapters) | 73 | 208 | parserVersion 0.2.2 + Smart X-Ray heuristics v1 — 6 suspect characters flagged, 1 merge suggestion |

## How to view

Open any file in a browser — no server, no dependencies, fully offline.
Click a node to see its `Algorithm Reason` in the sidebar; click an edge to see its reason.

To inspect raw data: open the file in a text editor and search for
`<script type="application/json" id="litgraph-data">` — the full JSON payload is embedded there.

## What the X-ray exposes

Each node carries a `reason` string showing the full parser decision path, e.g.:

```
character:rule=linguistic_signal;freq=268;speech_verb_hits=73;direct_address_hits=0;prefix=Рэй;NOT_IN_STOPLIST;forms=[Рэй]
```

```
chapter:words=1950;ε=32;emotion=4;unique=917;chars=[Веня];locs=[Землю]
```

Each edge carries its own reason:

```
kind=flow;avg_ε=31.0;src_ε=32;tgt_ε=30;reason=sequence_by_chapter_num
kind=character;reason=char_mentioned_in_chapter;char_freq=268
kind=location;reason=loc_appears_in_chapter;loc_freq=31
```

## Adding a new sample

1. Export from the LitGraph desktop app: **Toolbar → Экспорт HTML (X-ray)**.
2. Save the `.xray.html` file into this folder.
3. Add a row to the table above.

## Smart X-Ray (v1)

Starting from heuristics v1, every X-ray export runs all nodes through a
post-processing diagnostic layer (`src/lib/litgraph/heuristics.ts`) and embeds
the results in the HTML. The mini-program renders:

- **Diagnostic bar at the top**: `67 ok · 6 suspect · 0 error · 1 suggestion`
- **Colored borders on suspect nodes**: yellow (suspect) / red (error)
- **Badge icon** (`?` for suspect, `!` for error) in the top-right corner of each flagged node
- **Diagnostic block in the sidebar** (when a flagged node is clicked):
  - Confidence score (0–100%)
  - Each warning with level (`error` / `warn` / `info`), code, message, detail
  - Each suggestion with code, message, target node id (for merge hints)

### Heuristic rules

| Code | Level | Trigger |
|------|-------|---------|
| `SUSPECT_WORD` | warn | Node title is in `SUSPECT_WORDS` list (70 polysemantic abstractions: Архив, Бездна, Совет, Голос, Эхо, Клан, Тишина, Тень, Сфера, …) |
| `MINIMAL_SPEECH` | warn | `speechCount < 3` && `freq > 20` — too few speech verbs for a frequent character |
| `NO_SPEECH_VERBS` | error | `speechCount == 0 && directCount == 0` — defensive (parser v0.3.0 should already prevent this) |
| `LOW_SPEECH_RATIO` | warn | `freq > 50 && speechCount / freq < 5%` — likely concept, not character |
| `DIRECT_ADDRESS_PATTERN_MISS` | info | `freq > 50 && directCount == 0 && speechCount >= 3` — character is OK, but parser's `— Name,` pattern is too strict |
| `MERGE_WITH_CHARACTER` | suggestion | Location's 4-char prefix matches a character's prefix (e.g. «Алексея» ↔ «Алексей») |
| `SUSPECT_LOCATION_NAME` | info | Location title is in `SUSPECT_WORDS` |
| `LOW_FREQ_LOCATION` | info | Location mentioned < 5 times — likely noise |

### What's NOT in v1 (roadmap)

- **pymorphy3 morphological analysis** — to detect inanimate nouns programmatically
- **Lemma-matching beyond 4-char prefix** — currently «Рэя» ↔ «Рэй» is not detected because the 3rd letter differs; needs proper lemmatizer
- **Context-window analysis** — reading the chapter text to determine if «Архив» in this mention is a character, location, or concept
- **Manual override button** — letting the user click "this is not a character" and persisting the correction
