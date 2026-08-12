#!/usr/bin/env python3
"""
proposer.py — Phase 2 Step 4

LLM proposer: приймає текст, відправляє в z-ai-web-dev-sdk LLM,
отримує candidate nodes у JSON-форматі.

Використовує z-ai CLI (subprocess) — простіше ніж Node.js SDK з Python.

Usage:
    python3 proposer.py path/to/file.md out/candidate_nodes.json
    python3 proposer.py path/to/file.md out/candidate_nodes.json --max-chars 20000

Output format (matches rust_nodes.json schema):
    [
      {
        "source": "book1.md",
        "text_sha256": "abc123...",
        "text_length": 12345,
        "entities": [
          { "lemma": "...", "label": "PER", "count": 5, "forms": [...], ... }
        ],
        "model": "llm-proposer-glm",
        "version": "0.1"
      }
    ]
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import List, Dict, Any

# System prompt for LLM proposer.
# We ask for JSON output matching our schema.
SYSTEM_PROMPT = """Ты — лингвистический анализатор художественной литературы.

Задача: извлечь из текста всех персонажей (character entities) и вернуть их в виде JSON.

Правила:
1. Персонаж — это одушевлённое существо, которое действует, говорит, думает или упоминается в тексте.
2. Не персонажи: города (Москва), организации (КГБ), абстрактные понятия (Бездна, Смерть, Любовь), если они не одушевлены явно.
3. Один персонаж — одна запись (лемма). Все формы (Борис, Бориса, Борису) → одна entity.
4. lemma — нормальная форма имени (именительный падеж, единственное число).
5. label всегда "PER".
6. count — количество упоминаний в тексте.
7. forms — список всех форм имени, встретившихся в тексте.

Верни ТОЛЬКО валидный JSON, без markdown, без пояснений:
{
  "entities": [
    {"lemma": "Борис", "label": "PER", "count": 5, "forms": ["Борис", "Бориса", "Борису"]},
    ...
  ]
}

Если в тексте нет персонажей, верни: {"entities": []}
"""


def sha256_of_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def split_text_into_chunks(text: str, max_chars: int) -> List[str]:
    """Split text into chunks of at most max_chars, on paragraph boundary if possible."""
    if len(text) <= max_chars:
        return [text]

    chunks = []
    paragraphs = text.split("\n\n")
    current = ""
    for para in paragraphs:
        if len(current) + len(para) + 2 <= max_chars:
            current = (current + "\n\n" + para) if current else para
        else:
            if current:
                chunks.append(current)
            # If single paragraph > max_chars, hard-split
            if len(para) > max_chars:
                for i in range(0, len(para), max_chars):
                    chunks.append(para[i:i + max_chars])
                current = ""
            else:
                current = para
    if current:
        chunks.append(current)
    return chunks


def call_llm(text_chunk: str) -> Dict[str, Any]:
    """Call z-ai CLI with system prompt + text chunk, return parsed JSON."""
    user_prompt = f"""Проанализируй текст и извлеки всех персонажей:

---ТЕКСТ---
{text_chunk}
---КОНЕЦ ТЕКСТА---

Верни JSON в указанном формате."""

    import tempfile
    import os

    try:
        # z-ai CLI mixes diagnostic messages ("🚀 Initializing...") with stdout.
        # Use -o <file> to write JSON output to file, capture only stderr.
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as tmp:
            tmp_path = tmp.name

        try:
            result = subprocess.run(
                ["z-ai", "chat", "--prompt", user_prompt, "--system", SYSTEM_PROMPT,
                 "--output", tmp_path],
                capture_output=True,
                text=True,
                timeout=120,
                check=False,
            )
            if result.returncode != 0:
                print(f"  ERROR: z-ai CLI failed (rc={result.returncode}): {result.stderr[:200]}", file=sys.stderr)
                return {"entities": []}

            # Read JSON output file
            try:
                with open(tmp_path, "r", encoding="utf-8") as f:
                    cli_output = json.load(f)
                content = cli_output.get("choices", [{}])[0].get("message", {}).get("content", "")
            except (json.JSONDecodeError, FileNotFoundError, KeyError) as e:
                print(f"  WARNING: Could not read CLI output file: {e}", file=sys.stderr)
                return {"entities": []}

        finally:
            try:
                os.unlink(tmp_path)
            except OSError:
                pass

        # Strip markdown code fences if present
        content = content.strip()
        if content.startswith("```"):
            # Remove first line (```json) and last line (```)
            lines = content.split("\n")
            if len(lines) >= 3:
                content = "\n".join(lines[1:-1])
            elif len(lines) == 2:
                content = lines[0][3:]  # just ```json stripped
        # Some LLMs add trailing prose after JSON. Find last `}` and truncate.
        if content and not content.endswith("}"):
            last_brace = content.rfind("}")
            if last_brace > 0:
                content = content[:last_brace + 1]

        # Parse the JSON response
        try:
            parsed = json.loads(content)
            if isinstance(parsed, dict) and "entities" in parsed:
                return parsed
            else:
                print(f"  WARNING: LLM returned JSON without 'entities' key: {content[:200]}", file=sys.stderr)
                return {"entities": []}
        except json.JSONDecodeError as e:
            print(f"  WARNING: LLM returned non-JSON (parse error: {e}): {content[:200]}", file=sys.stderr)
            return {"entities": []}

    except subprocess.TimeoutExpired:
        print(f"  ERROR: z-ai CLI timed out", file=sys.stderr)
        return {"entities": []}
    except FileNotFoundError:
        print(f"  ERROR: z-ai CLI not found. Install with: npm install -g z-ai-web-dev-sdk", file=sys.stderr)
        sys.exit(1)


def merge_chunk_results(chunks_results: List[Dict[str, Any]]) -> Dict[str, Any]:
    """Merge entities from multiple chunks. Group by lowercase lemma, sum counts, union forms."""
    by_lemma: Dict[str, Dict[str, Any]] = {}
    for chunk_result in chunks_results:
        for entity in chunk_result.get("entities", []):
            lemma = entity.get("lemma", "").strip()
            if not lemma:
                continue
            key = lemma.lower()
            if key in by_lemma:
                # Merge
                existing = by_lemma[key]
                existing["count"] += entity.get("count", 1)
                existing_forms = set(existing.get("forms", []))
                existing_forms.update(entity.get("forms", []))
                existing["forms"] = sorted(existing_forms)
            else:
                by_lemma[key] = {
                    "lemma": lemma,
                    "label": entity.get("label", "PER"),
                    "count": entity.get("count", 1),
                    "forms": entity.get("forms", [lemma]),
                }
    return {"entities": list(by_lemma.values())}


def main():
    parser = argparse.ArgumentParser(description="LLM proposer for teaching loop")
    parser.add_argument("input", type=Path, help="Input .md/.txt file")
    parser.add_argument("output", type=Path, help="Output JSON path (candidate_nodes.json)")
    parser.add_argument("--max-chars", type=int, default=20000,
                        help="Max chars per LLM chunk (default: 20000)")
    args = parser.parse_args()

    if not args.input.is_file():
        print(f"ERROR: {args.input} not found", file=sys.stderr)
        sys.exit(1)

    text = args.input.read_text(encoding="utf-8")
    text_sha = sha256_of_text(text)
    print(f"  → {args.input.name} ({len(text)} chars, sha={text_sha[:8]}...)")

    chunks = split_text_into_chunks(text, args.max_chars)
    print(f"  Split into {len(chunks)} chunks (max {args.max_chars} chars each)")

    chunk_results = []
    for i, chunk in enumerate(chunks, 1):
        print(f"  Chunk {i}/{len(chunks)}: {len(chunk)} chars → LLM...")
        result = call_llm(chunk)
        entity_count = len(result.get("entities", []))
        print(f"    Got {entity_count} entities")
        chunk_results.append(result)

    merged = merge_chunk_results(chunk_results)
    print(f"  Merged: {len(merged['entities'])} unique entities")

    record = {
        "source": args.input.name,
        "text_sha256": text_sha,
        "text_length": len(text),
        "entities": merged["entities"],
        "stats": {"total": len(merged["entities"])},
        "model": "llm-proposer-glm",
        "version": "0.1",
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    # If output exists, append; else create new list
    if args.output.exists():
        with open(args.output, "r", encoding="utf-8") as f:
            existing = json.load(f)
        if not isinstance(existing, list):
            existing = []
    else:
        existing = []

    existing.append(record)
    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(existing, f, ensure_ascii=False, indent=2)

    print(f"\n✓ Wrote {args.output}: {len(existing)} records ({len(merged['entities'])} entities in last)")


if __name__ == "__main__":
    main()
