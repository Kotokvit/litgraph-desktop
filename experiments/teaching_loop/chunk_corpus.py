#!/usr/bin/env python3
"""
chunk_corpus.py — Split large corpus files into smaller chunks.

Big Russian books (Anna Karenina 2MB, Karamazov 2MB) are too slow for
rust_ner_cli (60s timeout) and would overload the LLM proposer. We split
each file into chunks of ~50K chars at paragraph boundaries.

Output: corpus_chunks/ directory with smaller .md files
"""

import sys
from pathlib import Path
import hashlib

def split_file(path: Path, out_dir: Path, max_chars: int = 50000) -> list[Path]:
    """Split a text file into chunks of at most max_chars on paragraph boundary."""
    text = path.read_text(encoding='utf-8', errors='ignore')
    if len(text) <= max_chars:
        dest = out_dir / path.name
        if not dest.exists():
            dest.write_text(text, encoding='utf-8')
        return [dest]

    chunks = []
    paragraphs = text.split('\n\n')
    current = ""
    chunk_idx = 0
    for para in paragraphs:
        if len(current) + len(para) + 2 <= max_chars:
            current = (current + '\n\n' + para) if current else para
        else:
            if current:
                chunk_idx += 1
                dest = out_dir / f"{path.stem}_p{chunk_idx:02d}{path.suffix}"
                dest.write_text(current, encoding='utf-8')
                chunks.append(dest)
                current = ""
            # If single paragraph > max_chars, hard-split
            if len(para) > max_chars:
                for i in range(0, len(para), max_chars):
                    chunk_idx += 1
                    dest = out_dir / f"{path.stem}_p{chunk_idx:02d}{path.suffix}"
                    dest.write_text(para[i:i+max_chars], encoding='utf-8')
                    chunks.append(dest)
            else:
                current = para
    if current:
        chunk_idx += 1
        dest = out_dir / f"{path.stem}_p{chunk_idx:02d}{path.suffix}"
        dest.write_text(current, encoding='utf-8')
        chunks.append(dest)
    return chunks


def main():
    in_dir = Path(sys.argv[1])
    out_dir = Path(sys.argv[2])
    max_chars = int(sys.argv[3]) if len(sys.argv) > 3 else 50000
    out_dir.mkdir(parents=True, exist_ok=True)

    files = sorted([p for p in in_dir.iterdir()
                    if p.is_file() and p.suffix.lower() in ('.md', '.txt')])
    print(f"Splitting {len(files)} files from {in_dir} into chunks ≤{max_chars} chars...")

    total_chunks = 0
    for f in files:
        chunks = split_file(f, out_dir, max_chars)
        total_chunks += len(chunks)
        if len(chunks) > 1:
            print(f"  {f.name}: {len(chunks)} chunks")

    print(f"\n✓ Generated {total_chunks} chunks in {out_dir}")
    # Stats
    chunk_sizes = [p.stat().st_size for p in out_dir.iterdir() if p.is_file()]
    print(f"  Total size: {sum(chunk_sizes) / 1024 / 1024:.2f} MB")
    print(f"  Min: {min(chunk_sizes) / 1024:.1f} KB")
    print(f"  Max: {max(chunk_sizes) / 1024:.1f} KB")
    print(f"  Avg: {sum(chunk_sizes) / len(chunk_sizes) / 1024:.1f} KB")


if __name__ == "__main__":
    main()
