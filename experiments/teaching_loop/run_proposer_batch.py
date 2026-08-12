#!/usr/bin/env python3
"""
run_proposer_batch.py — Run proposer on all corpus files sequentially.

Calls proposer.py for each file in corpus_subset/, appending to out/candidate_nodes.json.
Logs progress with timestamps so we can monitor via tail -f.
"""

import sys
import subprocess
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
CORPUS_DIR = SCRIPT_DIR / "corpus_subset"
OUT_FILE = SCRIPT_DIR / "out" / "candidate_nodes.json"
LOG_FILE = SCRIPT_DIR / "out" / "proposer.log"

OUT_FILE.parent.mkdir(exist_ok=True)
if OUT_FILE.exists():
    OUT_FILE.unlink()

files = sorted([p for p in CORPUS_DIR.iterdir() if p.is_file() and p.suffix.lower() in ('.md', '.txt')])
print(f"Starting proposer on {len(files)} files...", flush=True)

ok = 0
err = 0
start = time.time()

with open(LOG_FILE, 'w') as logf:
    logf.write(f"Started at {time.strftime('%Y-%m-%d %H:%M:%S')}\n")
    logf.write(f"Files: {len(files)}\n\n")
    logf.flush()

    for i, f in enumerate(files, 1):
        elapsed = time.time() - start
        rate = i / max(elapsed, 0.1)
        eta = (len(files) - i) / max(rate, 0.01)
        msg = f"[{i:3}/{len(files)}] {f.name} (elapsed {elapsed:.0f}s, eta {eta:.0f}s)"
        print(msg, flush=True)
        logf.write(msg + "\n")
        logf.flush()

        try:
            result = subprocess.run(
                [sys.executable, str(SCRIPT_DIR / "proposer.py"),
                 str(f), str(OUT_FILE), '--max-chars', '50000'],
                capture_output=True,
                text=True,
                timeout=180,
                check=False,
            )
            if result.returncode == 0:
                ok += 1
                # Print last meaningful line of stdout
                last_line = result.stdout.strip().split('\n')[-1] if result.stdout else ""
                logf.write(f"  OK: {last_line}\n")
            else:
                err += 1
                logf.write(f"  ERR rc={result.returncode}: {result.stderr[:200]}\n")
            logf.flush()
        except subprocess.TimeoutExpired:
            err += 1
            logf.write(f"  TIMEOUT after 180s\n")
            logf.flush()
        except Exception as e:
            err += 1
            logf.write(f"  EXC: {e}\n")
            logf.flush()

    summary = f"\nDone at {time.strftime('%Y-%m-%d %H:%M:%S')}: {ok} ok, {err} errors, total {time.time()-start:.0f}s"
    print(summary, flush=True)
    logf.write(summary + "\n")
