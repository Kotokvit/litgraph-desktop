#!/usr/bin/env python3
"""
auto_reviewer.py — Phase 2 Step 4

Автоматичний reviewer: читає diff.json, для кожної Rust-сутності
приймає рішення approve/reject і генерує training examples для Burn.

Логіка рішень:
  - matched (Rust=Found, LLM=Found)  → label=1.0 (approve)
  - missing (Rust=Found, LLM=NotFound) → label=0.0 (reject — Rust false positive)
  - extra   (LLM=Found, Rust=NotFound) — не впливає на Rust-тренування
    (це candidate для розширення Rust-парсера в майбутньому)

Для кожного training example зберігаємо:
  - features (8 floats з rust_entities)
  - label (0.0 або 1.0)
  - rust_confidence (що Rust гадав — Burn має навчитись це уточнювати)
  - decision source (matched/missing)
  - text_sha256 + source file

Output: dataset.jsonl (one JSON object per line, append-only)

Usage:
    python3 auto_reviewer.py out/diff.json dataset.jsonl
"""

import argparse
import json
import sys
from pathlib import Path
from typing import List, Dict, Any


def make_training_examples(diffs: List[Dict]) -> List[Dict]:
    """Convert diff records into training examples for Burn."""
    examples = []

    for diff in diffs:
        source = diff["source"]
        text_sha = diff.get("text_sha256", "")
        text_len = diff.get("text_length", 0)

        # Matched entities → label=1.0 (true positives)
        for m in diff["matched"]:
            if not m.get("rust_features"):
                continue
            examples.append({
                "source": source,
                "text_sha256": text_sha,
                "text_length": text_len,
                "lemma": m["lemma"],
                "features": m["rust_features"],
                "rust_confidence": m["rust_confidence"],
                "label": 1.0,
                "decision": "approve_matched",
                "rust_count": m["rust_count"],
                "llm_count": m["llm_count"],
            })

        # Missing entities (Rust found, LLM didn't) → label=0.0 (false positives)
        for m in diff["missing"]:
            if not m.get("rust_features"):
                continue
            examples.append({
                "source": source,
                "text_sha256": text_sha,
                "text_length": text_len,
                "lemma": m["lemma"],
                "features": m["rust_features"],
                "rust_confidence": m["rust_confidence"],
                "label": 0.0,
                "decision": "reject_missing",
                "rust_count": m["rust_count"],
                "llm_count": 0,
            })

    approve_count = sum(1 for e in examples if e["label"] == 1.0)
    reject_count = sum(1 for e in examples if e["label"] == 0.0)

    target_min_rejects = (approve_count + 2) // 3
    if reject_count < target_min_rejects:
        needed = target_min_rejects - reject_count
        synthetic_rejects = [
            {"lemma": "Бездна", "features": [1.0, 0.0, 0.0, 1.0, 0.1, 0.0, 0.0, 0.0, 0.0, 0.1, 0.2], "decision": "reject_concept"},
            {"lemma": "ковальня", "features": [0.0, 0.0, 0.0, 1.0, 0.05, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0], "decision": "reject_common_noun"},
            {"lemma": "Львів", "features": [1.0, 0.0, 0.0, 1.0, 0.05, 0.0, 0.0, 1.0, 0.1, 0.0, 0.0], "decision": "reject_location"},
            {"lemma": "Таких Слів", "features": [1.0, 0.0, 0.0, 0.0, 0.05, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0], "decision": "reject_multi_token_non_char"},
            {"lemma": "Марті", "features": [1.0, 0.0, 0.0, 1.0, 0.1, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0], "decision": "reject_dative_object"},
        ]
        for i in range(needed):
            tmpl = synthetic_rejects[i % len(synthetic_rejects)]
            examples.append({
                "source": "synthetic_rebalance",
                "text_sha256": "",
                "text_length": 0,
                "lemma": tmpl["lemma"],
                "features": tmpl["features"],
                "rust_confidence": 0.3,
                "label": 0.0,
                "decision": tmpl["decision"],
                "rust_count": 1,
                "llm_count": 0,
            })

    return examples


def main():
    parser = argparse.ArgumentParser(description="Auto-reviewer: convert diffs to training examples")
    parser.add_argument("diff_json", type=Path, help="diff.json from comparator")
    parser.add_argument("dataset", type=Path, help="dataset.jsonl (append-only)")
    parser.add_argument("--clear", action="store_true",
                        help="Clear dataset before writing (default: append)")
    args = parser.parse_args()

    if not args.diff_json.exists():
        print(f"ERROR: {args.diff_json} not found", file=sys.stderr)
        sys.exit(1)

    with open(args.diff_json, "r", encoding="utf-8") as f:
        diffs = json.load(f)

    print(f"Reviewing {len(diffs)} diffs...")
    examples = make_training_examples(diffs)

    approve_count = sum(1 for e in examples if e["label"] == 1.0)
    reject_count = sum(1 for e in examples if e["label"] == 0.0)
    print(f"  Generated {len(examples)} examples (approve={approve_count}, reject={reject_count})")

    # Write/append to dataset.jsonl
    args.dataset.parent.mkdir(parents=True, exist_ok=True)
    mode = "w" if args.clear else "a"
    with open(args.dataset, mode, encoding="utf-8") as f:
        for ex in examples:
            f.write(json.dumps(ex, ensure_ascii=False) + "\n")

    # Count total
    if mode == "a" and args.dataset.exists():
        with open(args.dataset, "r", encoding="utf-8") as f:
            total = sum(1 for _ in f)
    else:
        total = len(examples)

    print(f"\n✓ Wrote {args.dataset} ({mode} mode, {len(examples)} new, {total} total)")
    if total < 50:
        print(f"  NOTE: {total}/50 examples — need more data to train (minimum 50)")


if __name__ == "__main__":
    main()
