#!/usr/bin/env python3
"""
ingest_corpus.py — Phase 2 Step 4

Приймає папку з .md/.txt файлами, для кожного викликає Rust CLI
(`litgraph-core/target/debug/rust_ner_cli`) і зберігає результат у
`out/rust_nodes.json`.

Вихідний формат: список record'ів:
    [
      {
        "source": "book1.md",
        "text_sha256": "abc123...",
        "text_length": 12345,
        "entities": [
          { "lemma": "...", "label": "PER", "features": [...], "rust_confidence": 0.7, ... }
        ]
      },
      ...
    ]

Usage:
    python3 ingest_corpus.py corpus/ out/rust_nodes.json
    python3 ingest_corpus.py corpus/ out/rust_nodes.json --rust-cli /path/to/rust_ner_cli
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path


def find_text_files(corpus_dir: Path):
    """Yield all .md and .txt files in corpus_dir (non-recursive for now)."""
    for path in sorted(corpus_dir.iterdir()):
        if path.is_file() and path.suffix.lower() in (".md", ".txt"):
            yield path


def run_rust_cli(rust_cli: Path, text_path: Path) -> dict:
    """Call rust_ner_cli on text_path, return parsed JSON."""
    try:
        result = subprocess.run(
            [str(rust_cli), str(text_path)],
            capture_output=True,
            text=True,
            timeout=60,  # 1 minute per file — should be plenty for <1MB texts
            check=True,
        )
        return json.loads(result.stdout)
    except subprocess.TimeoutExpired:
        print(f"  ERROR: Rust CLI timed out on {text_path.name}", file=sys.stderr)
        return {"entities": [], "stats": {"total": 0}, "model": "timeout", "version": ""}
    except subprocess.CalledProcessError as e:
        print(f"  ERROR: Rust CLI failed on {text_path.name}: {e.stderr[:200]}", file=sys.stderr)
        return {"entities": [], "stats": {"total": 0}, "model": "error", "version": ""}
    except json.JSONDecodeError as e:
        print(f"  ERROR: Invalid JSON from Rust CLI for {text_path.name}: {e}", file=sys.stderr)
        return {"entities": [], "stats": {"total": 0}, "model": "invalid", "version": ""}


def sha256_of_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser(description="Ingest corpus through Rust NER CLI")
    parser.add_argument("corpus_dir", type=Path, help="Directory with .md/.txt files")
    parser.add_argument("output", type=Path, help="Output JSON path (rust_nodes.json)")
    parser.add_argument("--rust-cli", type=Path,
                        default=Path(__file__).resolve().parents[2] / "litgraph-core" / "target" / "debug" / "rust_ner_cli",
                        help="Path to rust_ner_cli binary")
    args = parser.parse_args()

    if not args.corpus_dir.is_dir():
        print(f"ERROR: {args.corpus_dir} is not a directory", file=sys.stderr)
        sys.exit(1)

    if not args.rust_cli.exists():
        print(f"ERROR: Rust CLI not found at {args.rust_cli}", file=sys.stderr)
        print(f"Build it first: cd litgraph-core && cargo build --bin rust_ner_cli", file=sys.stderr)
        sys.exit(1)

    files = list(find_text_files(args.corpus_dir))
    if not files:
        print(f"WARNING: No .md/.txt files in {args.corpus_dir}", file=sys.stderr)
        # Create empty output for pipeline continuity
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text("[]")
        return

    print(f"Ingesting {len(files)} files from {args.corpus_dir}...")
    records = []
    for path in files:
        text_sha = sha256_of_file(path)
        text_len = path.stat().st_size
        print(f"  → {path.name} ({text_len} bytes, sha={text_sha[:8]}...)")
        ner = run_rust_cli(args.rust_cli, path)
        records.append({
            "source": path.name,
            "text_sha256": text_sha,
            "text_length": text_len,
            "entities": ner.get("entities", []),
            "stats": ner.get("stats", {}),
            "model": ner.get("model", "unknown"),
            "version": ner.get("version", "unknown"),
        })

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(records, f, ensure_ascii=False, indent=2)

    total_entities = sum(len(r["entities"]) for r in records)
    print(f"\n✓ Wrote {args.output}: {len(records)} files, {total_entities} total entities")


if __name__ == "__main__":
    main()
