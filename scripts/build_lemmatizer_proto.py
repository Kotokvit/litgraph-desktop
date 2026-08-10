#!/usr/bin/env python3
"""
Python prototype of build_lemmatizer.rs — validates the parsing logic
before the user runs the Rust version locally.
"""
import re
import json
import gzip
import os
from pathlib import Path
from collections import defaultdict

DICT_UK_PATH = Path("/home/z/my-project/litgraph-desktop/resources/ua-linguistic/dict_uk")
OUT_PATH = Path("/home/z/my-project/litgraph-desktop/resources/ua-linguistic/derivatives/lemma_index.json.gz")


def parse_base_lst(path):
    """Parse base.lst → list of (lemma, paradigm_class, modifiers)."""
    lemmas = []
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            # Strip trailing comment
            line = line.split('#')[0].strip()
            if not line:
                continue
            # Split on '/'
            parts = line.split('/', 1)
            if len(parts) != 2:
                continue
            lemma = parts[0].strip()
            tag_str = parts[1].strip()
            if not lemma or not tag_str:
                continue
            tag_parts = tag_str.split('.')
            paradigm_class = tag_parts[0]
            modifiers = tag_parts[1:]
            lemmas.append({
                'lemma': lemma,
                'paradigm_class': paradigm_class,
                'modifiers': modifiers,
            })
    return lemmas


def parse_affix_file(path):
    """Parse a single .aff file → list of (group_name, [rules])."""
    groups = []
    current_group = None
    pending_regex = None

    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.rstrip()
            stripped = line.strip()
            if not stripped or stripped.startswith('#'):
                continue

            if stripped.startswith('group '):
                if current_group:
                    groups.append(current_group)
                name = stripped[len('group '):].strip()
                current_group = {'name': name, 'rules': []}
                pending_regex = None
                continue

            if stripped.startswith('subgroup '):
                pending_regex = None
                continue

            # Header line ending with ':' (regex constraint for next rules)
            if stripped.endswith(':') and '\t' not in stripped:
                pending_regex = stripped[:-1].strip()
                continue

            # Rule line — strip comment, keep @ tag
            if '#' in line:
                idx = line.index('#')
                rule_part = line[:idx]
                comment = line[idx:]
                pos_tag = ''
                if '@' in comment:
                    pos_tag = comment.split('@', 1)[1].strip()
            else:
                rule_part = line
                pos_tag = ''

            cols = [c.strip() for c in rule_part.split('\t') if c.strip()]
            # Fallback: dict_uk sometimes uses spaces instead of tabs
            if len(cols) < 2:
                cols = [c.strip() for c in rule_part.split() if c.strip()]

            if len(cols) < 2:
                continue

            # Format A (n1.aff): <from> <to> [<regex>]  — 3 cols, regex optional
            # Format B (n2n.aff): <from> <to> <example_word>  — 3 cols, 3rd is example (not regex!)
            # Distinguish: regex starts with '[' or '.', example word starts with letter
            from_suffix = cols[0]
            to_suffix = cols[1]
            regex_str = None
            if len(cols) >= 3:
                third = cols[2]
                if third.startswith('[') or third.startswith('.') or third.startswith('^'):
                    # Format A with regex
                    regex_str = third
                # else: Format B — third column is example word, ignore it
            if regex_str is None:
                regex_str = pending_regex

            current_group['rules'].append({
                'from_suffix': from_suffix,
                'to_suffix': to_suffix,
                'regex': regex_str,
                'pos_tag': pos_tag,
            }) if current_group is not None else None

    if current_group:
        groups.append(current_group)
    return groups


def parse_all_affix(affix_dir):
    """Parse all .aff files → dict paradigm_class → [rules]."""
    groups = {}
    for entry in os.listdir(affix_dir):
        if not entry.endswith('.aff'):
            continue
        path = os.path.join(affix_dir, entry)
        file_groups = parse_affix_file(path)
        for g in file_groups:
            groups[g['name']] = g['rules']
    return groups


def apply_rule(lemma, rule):
    """Apply one rule to a lemma → word form, or None if rule doesn't match."""
    # Check regex constraint
    if rule['regex']:
        # dict_uk regex matches end of word
        # Skip "0" placeholder used as regex (means "no suffix", handled by from_suffix=="0")
        regex_str = rule['regex']
        if regex_str == '0' or regex_str == '.':
            pass  # No actual constraint
        else:
            pattern = regex_str + '$'
            try:
                if not re.search(pattern, lemma, re.IGNORECASE):
                    return None
            except re.error:
                return None

    # Handle special tokens
    from_suffix = rule['from_suffix']
    to_suffix = rule['to_suffix']

    # "0" means "no suffix to remove" — just append to_suffix
    if from_suffix == '0' or from_suffix == '':
        return lemma + to_suffix

    # "." means "match anything" — also just append
    if from_suffix == '.':
        return lemma + to_suffix

    # Check from_suffix matches end of lemma
    if not lemma.lower().endswith(from_suffix.lower()):
        return None
    # Strip from_suffix and append to_suffix
    stem = lemma[:len(lemma) - len(from_suffix)]
    return stem + to_suffix


def main():
    print("=== Python prototype of build_lemmatizer ===")
    base_lst = DICT_UK_PATH / "data/dict/base.lst"
    affix_dir = DICT_UK_PATH / "data/affix"

    print("[1/4] Parsing affix rules...")
    affix_groups = parse_all_affix(affix_dir)
    total_rules = sum(len(r) for r in affix_groups.values())
    print(f"      Loaded {len(affix_groups)} paradigm groups, {total_rules} total rules")
    print(f"      Groups: {sorted(affix_groups.keys())}")

    print("[2/4] Parsing lemmas from base.lst...")
    lemmas = parse_base_lst(base_lst)
    print(f"      Loaded {len(lemmas)} lemma records")

    # Count by paradigm class
    by_class = defaultdict(int)
    for l in lemmas:
        by_class[l['paradigm_class']] += 1
    print(f"      Top 10 paradigm classes:")
    for cls, cnt in sorted(by_class.items(), key=lambda x: -x[1])[:10]:
        print(f"        {cls:10s} {cnt:>6}")

    print("[3/4] Generating word forms (this may take a while)...")
    word_form_index = defaultdict(list)
    total_forms = 0
    lemmas_no_paradigm = 0

    for i, lemma_rec in enumerate(lemmas):
        if i % 25000 == 0:
            print(f"      Processing lemma {i}/{len(lemmas)} ({total_forms} forms so far)")

        paradigm = lemma_rec['paradigm_class']
        rules = affix_groups.get(paradigm)
        if rules is None:
            lemmas_no_paradigm += 1
            word_form_index[lemma_rec['lemma'].lower()].append({
                'lemma': lemma_rec['lemma'],
                'pos': 'lemma:base',
                'paradigm_class': paradigm,
            })
            total_forms += 1
            continue

        for rule in rules:
            form = apply_rule(lemma_rec['lemma'], rule)
            if form:
                word_form_index[form.lower()].append({
                    'lemma': lemma_rec['lemma'],
                    'pos': rule['pos_tag'],
                    'paradigm_class': paradigm,
                })
                total_forms += 1

        # Also include lemma itself
        word_form_index[lemma_rec['lemma'].lower()].append({
            'lemma': lemma_rec['lemma'],
            'pos': f'lemma:base:{paradigm}',
            'paradigm_class': paradigm,
        })
        total_forms += 1

    print(f"      Generated {total_forms} total word forms")
    print(f"      Unique word forms in index: {len(word_form_index)}")
    print(f"      Lemmas without matching paradigm: {lemmas_no_paradigm}")

    print("[4/4] Serializing to JSON.gz...")
    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(OUT_PATH, 'wt', encoding='utf-8') as f:
        json.dump(dict(word_form_index), f, ensure_ascii=False)

    file_size = OUT_PATH.stat().st_size
    print(f"      Output size: {file_size / 1024 / 1024:.2f} MB")
    print("=== Done! ===")

    # Sanity check: look up some known forms
    print("\n=== Sanity Check ===")
    test_words = [
        ("ходити", "should be lemma"),
        ("ходив", "should lemmatize to ходити"),
        ("ходить", "should lemmatize to ходити"),
        ("ходили", "should lemmatize to ходити"),
        ("абонував", "should lemmatize to абонувати"),
        ("абонувала", "should lemmatize to абонувати"),
        ("страх", "should be noun lemma"),
        ("страху", "should be genitive of страх"),
        ("страшний", "should be adj"),
        ("етерія", "should NOT be in dict (made-up word)"),
    ]
    for word, expected in test_words:
        entries = word_form_index.get(word.lower(), [])
        if entries:
            lemmas_found = list(set(e['lemma'] for e in entries))
            pos_tags = list(set(e['pos'] for e in entries))[:3]
            print(f"  {word:20s} → lemma={lemmas_found}, pos={pos_tags}")
        else:
            print(f"  {word:20s} → NOT FOUND ({expected})")


if __name__ == '__main__':
    main()
