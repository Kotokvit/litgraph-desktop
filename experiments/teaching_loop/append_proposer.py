#!/usr/bin/env python3
"""
append_proposer.py — Run proposer.py on a single file, append result to candidate_nodes.json.

Designed to be called from a bash for-loop, so we can robustly iterate over
many files without one timeout killing the whole batch.

Usage:
    python3 append_proposer.py <input.md> <output.json> [--max-chars N]
"""

import sys
import subprocess
import time
from pathlib import Path

def main():
    if len(sys.argv) < 3:
        print("Usage: append_proposer.py <input> <output> [--max-chars N]")
        sys.exit(1)

    input_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2])

    # Parse --max-chars
    max_chars = 50000
    for i, arg in enumerate(sys.argv):
        if arg == '--max-chars' and i + 1 < len(sys.argv):
            max_chars = int(sys.argv[i + 1])

    script_dir = Path(__file__).resolve().parent
    start = time.time()

    try:
        result = subprocess.run(
            [sys.executable, str(script_dir / "proposer.py"),
             str(input_path), str(output_path), '--max-chars', str(max_chars)],
            capture_output=True,
            text=True,
            timeout=120,
            check=False,
        )
        elapsed = time.time() - start
        if result.returncode == 0:
            # Print last meaningful line
            last_line = (result.stdout or "").strip().split('\n')[-1]
            print(f"  OK [{elapsed:.1f}s]: {last_line}")
        else:
            print(f"  ERR [{elapsed:.1f}s] rc={result.returncode}: {result.stderr[:200]}")
    except subprocess.TimeoutExpired:
        elapsed = time.time() - start
        print(f"  TIMEOUT [{elapsed:.1f}s]")
    except Exception as e:
        elapsed = time.time() - start
        print(f"  EXC [{elapsed:.1f}s]: {e}")


if __name__ == "__main__":
    main()
