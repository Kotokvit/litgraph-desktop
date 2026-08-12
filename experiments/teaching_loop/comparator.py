#!/usr/bin/env python3
"""
comparator.py — Phase 2 Step 4

Порівнює rust_nodes.json та candidate_nodes.json, рахує diff:
  - matched: і lemma, і type співпадають (case-insensitive)
  - missing: Rust знайшов, LLM — ні (false positive від Rust, або false negative від LLM)
  - extra:   LLM знайшов, Rust — ні (false negative від Rust, або false positive від LLM)

Для кожної Rust-сутності зберігає її 8-feature vector + rust_confidence
(для подальшого тренування Burn).

Output: out/diff.json

Usage:
    python3 comparator.py out/rust_nodes.json out/candidate_nodes.json out/diff.json
"""

import argparse
import json
import sys
from pathlib import Path
from typing import List, Dict, Any


def normalize_lemma(lemma: str) -> str:
    """Normalize for matching: lowercase, strip whitespace."""
    return lemma.strip().lower()


def compare_records(rust_records: List[Dict], llm_records: List[Dict]) -> List[Dict]:
    """Compare two lists of records (one per source file). Match by source name."""
    llm_by_source = {r["source"]: r for r in llm_records}

    diffs = []
    for rust_record in rust_records:
        source = rust_record["source"]
        llm_record = llm_by_source.get(source, {"entities": [], "model": "missing", "version": ""})

        rust_entities = {normalize_lemma(e["lemma"]): e for e in rust_record.get("entities", [])}
        llm_entities = {normalize_lemma(e["lemma"]): e for e in llm_record.get("entities", [])}

        rust_keys = set(rust_entities.keys())
        llm_keys = set(llm_entities.keys())

        matched_keys = rust_keys & llm_keys
        missing_keys = rust_keys - llm_keys  # Rust-only
        extra_keys = llm_keys - rust_keys    # LLM-only

        matched = []
        for k in sorted(matched_keys):
            rust_e = rust_entities[k]
            llm_e = llm_entities[k]
            matched.append({
                "lemma": rust_e["lemma"],
                "rust_features": rust_e.get("features", []),
                "rust_confidence": rust_e.get("rust_confidence", 0.0),
                "rust_count": rust_e.get("count", 0),
                "llm_count": llm_e.get("count", 0),
                "llm_forms": llm_e.get("forms", []),
            })

        missing = []
        for k in sorted(missing_keys):
            rust_e = rust_entities[k]
            missing.append({
                "lemma": rust_e["lemma"],
                "rust_features": rust_e.get("features", []),
                "rust_confidence": rust_e.get("rust_confidence", 0.0),
                "rust_count": rust_e.get("count", 0),
            })

        extra = []
        for k in sorted(extra_keys):
            llm_e = llm_entities[k]
            extra.append({
                "lemma": llm_e["lemma"],
                "llm_count": llm_e.get("count", 0),
                "llm_forms": llm_e.get("forms", []),
            })

        # Compute precision/recall (treating LLM as ground truth)
        # matched + missing = total rust positives
        # matched + extra = total llm positives (ground truth)
        total_llm = len(matched) + len(extra)
        total_rust = len(matched) + len(missing)
        precision = len(matched) / total_rust if total_rust > 0 else 1.0
        recall = len(matched) / total_llm if total_llm > 0 else 1.0
        f1 = (2 * precision * recall / (precision + recall)) if (precision + recall) > 0 else 0.0

        diffs.append({
            "source": source,
            "text_sha256": rust_record.get("text_sha256", ""),
            "text_length": rust_record.get("text_length", 0),
            "rust_model": rust_record.get("model", ""),
            "llm_model": llm_record.get("model", ""),
            "matched": matched,
            "missing": missing,
            "extra": extra,
            "metrics": {
                "matched_count": len(matched),
                "missing_count": len(missing),
                "extra_count": len(extra),
                "precision": round(precision, 4),
                "recall": round(recall, 4),
                "f1": round(f1, 4),
            },
        })

    return diffs


def main():
    parser = argparse.ArgumentParser(description="Compare Rust vs LLM entities")
    parser.add_argument("rust_json", type=Path, help="rust_nodes.json")
    parser.add_argument("llm_json", type=Path, help="candidate_nodes.json")
    parser.add_argument("output", type=Path, help="Output diff.json")
    args = parser.parse_args()

    if not args.rust_json.exists():
        print(f"ERROR: {args.rust_json} not found", file=sys.stderr)
        sys.exit(1)
    if not args.llm_json.exists():
        print(f"ERROR: {args.llm_json} not found", file=sys.stderr)
        sys.exit(1)

    with open(args.rust_json, "r", encoding="utf-8") as f:
        rust_records = json.load(f)
    with open(args.llm_json, "r", encoding="utf-8") as f:
        llm_records = json.load(f)

    print(f"Comparing {len(rust_records)} Rust records vs {len(llm_records)} LLM records...")

    diffs = compare_records(rust_records, llm_records)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(diffs, f, ensure_ascii=False, indent=2)

    # Summary
    total_matched = sum(d["metrics"]["matched_count"] for d in diffs)
    total_missing = sum(d["metrics"]["missing_count"] for d in diffs)
    total_extra = sum(d["metrics"]["extra_count"] for d in diffs)
    print(f"\n✓ Wrote {args.output}")
    print(f"  Total: matched={total_matched}, missing={total_missing}, extra={total_extra}")
    for d in diffs:
        m = d["metrics"]
        print(f"  {d['source']}: matched={m['matched_count']}, missing={m['missing_count']}, extra={m['extra_count']}, "
              f"precision={m['precision']}, recall={m['recall']}, f1={m['f1']}")


if __name__ == "__main__":
    main()
