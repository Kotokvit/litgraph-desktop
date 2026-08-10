#!/usr/bin/env python3
"""
Test the lemma index on real manuscript fragments.
Loads resources/ua-linguistic/derivatives/lemma_index.json.gz and
runs lemmatization on samples from sfera.md and kasiopia.md.
"""
import gzip
import json
import re
from pathlib import Path

INDEX_PATH = Path("/home/z/my-project/litgraph-desktop/resources/ua-linguistic/derivatives/lemma_index.json.gz")
SFERA_PATH = Path("/home/z/my-project/litgraph-desktop/litgraph-core/tests/sfera.md")
KASIOPIA_PATH = Path("/home/z/my-project/litgraph-desktop/litgraph-core/tests/kasiopia.md")

print("Loading lemma index...")
with gzip.open(INDEX_PATH, 'rt', encoding='utf-8') as f:
    lemma_index = json.load(f)
print(f"Loaded {len(lemma_index):,} word forms")

def lemmatize_token(token):
    """Look up token in lemma index. Returns list of (lemma, pos) tuples."""
    token_lower = token.lower()
    entries = lemma_index.get(token_lower)
    if entries:
        # Return unique lemmas
        seen = set()
        result = []
        for e in entries:
            key = (e['lemma'], e['pos'])
            if key not in seen:
                seen.add(key)
                result.append(key)
        return result
    return []

def tokenize(text):
    """Simple UA-aware tokenizer."""
    return [t for t in re.findall(r"[А-ЯІЇЄа-яіїєЁё'a-zA-Z]+", text) if len(t) > 1]

def process_fragment(text, label=""):
    """Show lemmatization for a fragment."""
    print(f"\n{'='*70}")
    print(f"Fragment: {label}")
    print(f"Text: {text[:200]}{'...' if len(text) > 200 else ''}")
    print(f"{'='*70}")

    tokens = tokenize(text)
    print(f"\nTokens: {len(tokens)}")

    lemmatized = []
    not_found = []
    for t in tokens:
        entries = lemmatize_token(t)
        if entries:
            # Pick first lemma (most common case)
            lemma = entries[0][0]
            pos = entries[0][1]
            lemmatized.append((t, lemma, pos))
        else:
            not_found.append(t)
            lemmatized.append((t, t, "UNKNOWN"))

    print(f"\nLemmatization results:")
    for orig, lemma, pos in lemmatized[:25]:
        marker = "✓" if orig.lower() != lemma.lower() else " "
        print(f"  {marker} {orig:25s} → {lemma:25s}  [{pos}]")

    if len(lemmatized) > 25:
        print(f"  ... +{len(lemmatized) - 25} more")

    found_pct = (len(tokens) - len(not_found)) / max(1, len(tokens)) * 100
    print(f"\nCoverage: {found_pct:.1f}% ({len(tokens) - len(not_found)}/{len(tokens)} tokens)")

    if not_found:
        print(f"\nNot found ({len(not_found)}):", ", ".join(not_found[:20]))

# Real fragments from manuscripts
fragments = [
    # From Сфера Предела — top ε fragment
    ("Сфера #1 (ε=20.96)",
     "0 мЗв/цикл Адаптация детей: множественная, трёхвидовая кооперация (орк/гоблин/человек), микро-контур выживания подтверждён"),

    # From Сфера Предела — #4
    ("Сфера #4 (ε=15.59)",
     "Его Мнемарское сознание, усиленное топливом Марты, просчитало варианты за секунды: доложить Главному Аудитору"),

    # From Касіопея — #1
    ("Касіопея #1 (ε=10.22)",
     "Він уже прокручував у голові план: спочатку роздобути детальну карту небезпечного регіону, відомого як Сектор Гамма-3"),

    # From Касіопея — #4
    ("Касіопея #4 (ε=9.21)",
     "Юна сиділа на підлозі, обхопивши коліна, її погляд був порожнім — її аналітичний розум, зіткнувшись із парадоксом"),

    # Test specifically for verb forms that should lemmatize
    ("Verb forms test (UA)",
     "Вона ходила по кімнаті, думала про минуле, згадувала давніх друзів, і тихо плакала, сидячи біля вікна."),

    # Test for case variation
    ("Case variation (UA)",
     "Страх охопив воїна. Страху не було меж. Страшний звук пролунав у темряві."),

    # Russian fragments (Сфера is mostly RU)
    ("Russian fragment",
     "Он медленно шёл по коридору, смотрел на стены, вспоминал прошлые годы и тихо говорил сам с собой."),
]

for label, text in fragments:
    process_fragment(text, label)

print("\n" + "=" * 70)
print("SUMMARY")
print("=" * 70)

# Aggregate stats
total_tokens = 0
total_found = 0
for label, text in fragments:
    tokens = tokenize(text)
    found = sum(1 for t in tokens if lemmatize_token(t))
    total_tokens += len(tokens)
    total_found += found
    print(f"  {label:30s}: {found}/{len(tokens)} = {found/max(1,len(tokens))*100:.0f}%")

print(f"\nOverall coverage: {total_found}/{total_tokens} = {total_found/total_tokens*100:.1f}%")
