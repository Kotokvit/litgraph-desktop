# X-ray HTML Samples

Interactive HTML X-ray exports of LitGraph workspaces — self-contained `.html` mini-programs
that recreate the GUI (pan/zoom canvas + sidebar with full meta / reason / SVO) in any browser.

## Files

| File | Source | Nodes | Edges | Notes |
|------|--------|-------|-------|-------|
| `1-сфера-предела.xray.html` | «Сфера Предела» (172 709 words, 36 chapters) | 73 | 208 | parserVersion 0.2.2 — pre-POLER[Ψ] Centaur patch |

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
