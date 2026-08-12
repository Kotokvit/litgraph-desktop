#!/usr/bin/env python3
"""
run_pipeline.py — Phase 2 Step 4

Оркестратор: ingest → propose → compare → review.

Запускає повний цикл на папці з текстами, генерує dataset.jsonl.

Usage:
    python3 run_pipeline.py corpus/ out/
    python3 run_pipeline.py corpus/ out/ --max-chars 15000
"""

import argparse
import subprocess
import sys
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description="Run full teaching loop pipeline")
    parser.add_argument("corpus_dir", type=Path, help="Directory with .md/.txt files")
    parser.add_argument("out_dir", type=Path, help="Output directory for intermediate files")
    parser.add_argument("--max-chars", type=int, default=20000,
                        help="Max chars per LLM chunk (default: 20000)")
    parser.add_argument("--dataset", type=Path, default=None,
                        help="dataset.jsonl path (default: <out_dir>/dataset.jsonl)")
    parser.add_argument("--clear-dataset", action="store_true",
                        help="Clear dataset before running")
    args = parser.parse_args()

    args.out_dir.mkdir(parents=True, exist_ok=True)
    dataset = args.dataset or (args.out_dir / "dataset.jsonl")

    rust_json = args.out_dir / "rust_nodes.json"
    llm_json = args.out_dir / "candidate_nodes.json"
    diff_json = args.out_dir / "diff.json"

    script_dir = Path(__file__).resolve().parent

    # Step 1: Ingest corpus through Rust CLI
    print("=" * 60)
    print("STEP 1: ingest_corpus.py")
    print("=" * 60)
    subprocess.run([
        sys.executable, str(script_dir / "ingest_corpus.py"),
        str(args.corpus_dir), str(rust_json),
    ], check=True)

    # Step 2: For each text, call LLM proposer
    print("\n" + "=" * 60)
    print("STEP 2: proposer.py (LLM)")
    print("=" * 60)
    # Remove old candidate_nodes.json so we don't append to stale data
    if llm_json.exists():
        llm_json.unlink()

    text_files = sorted([p for p in args.corpus_dir.iterdir()
                         if p.is_file() and p.suffix.lower() in (".md", ".txt")])

    for text_file in text_files:
        print(f"\n--- {text_file.name} ---")
        subprocess.run([
            sys.executable, str(script_dir / "proposer.py"),
            str(text_file), str(llm_json),
            "--max-chars", str(args.max_chars),
        ], check=False)  # don't fail on individual file errors

    # Step 3: Compare
    print("\n" + "=" * 60)
    print("STEP 3: comparator.py")
    print("=" * 60)
    subprocess.run([
        sys.executable, str(script_dir / "comparator.py"),
        str(rust_json), str(llm_json), str(diff_json),
    ], check=True)

    # Step 4: Auto-review
    print("\n" + "=" * 60)
    print("STEP 4: auto_reviewer.py")
    print("=" * 60)
    review_args = [
        sys.executable, str(script_dir / "auto_reviewer.py"),
        str(diff_json), str(dataset),
    ]
    if args.clear_dataset:
        review_args.append("--clear")
    subprocess.run(review_args, check=True)

    # Summary
    print("\n" + "=" * 60)
    print("PIPELINE COMPLETE")
    print("=" * 60)
    print(f"  Rust output:    {rust_json}")
    print(f"  LLM output:     {llm_json}")
    print(f"  Diff:           {diff_json}")
    print(f"  Dataset:        {dataset}")

    if dataset.exists():
        with open(dataset, "r", encoding="utf-8") as f:
            total = sum(1 for _ in f)
        print(f"  Total examples: {total}")
        if total >= 50:
            print(f"\n✓ Ready to train! Run:")
            print(f"  cargo run --release --bin train_scorer -- \\")
            print(f"      --dataset {dataset} \\")
            print(f"      --weights <output_weights.json> \\")
            print(f"      --epochs 200")


if __name__ == "__main__":
    main()
