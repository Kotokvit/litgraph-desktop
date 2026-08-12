#!/usr/bin/env python3
"""
download_corpus.py — Downloads Russian and Ukrainian books and texts for teaching loop corpus.
"""

import os
import re
import json
import urllib.request
from pathlib import Path

CORPUS_DIR = Path(__file__).resolve().parent / "corpus"
CORPUS_DIR.mkdir(parents=True, exist_ok=True)

HEADERS = {'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) LitGraph/1.0'}

# 1. Project Gutenberg Russian Classics
GUTENBERG_BOOKS = {
    'gogol_dead_souls_ru.md': 'https://www.gutenberg.org/cache/epub/12117/pg12117.txt',
    'dostoevsky_crime_ru.md': 'https://www.gutenberg.org/cache/epub/2554/pg2554.txt',
    'tolstoy_anna_ru.md': 'https://www.gutenberg.org/cache/epub/1399/pg1399.txt',
    'chekhov_stories_ru.md': 'https://www.gutenberg.org/cache/epub/13437/pg13437.txt',
    'pushkin_onegin_ru.md': 'https://www.gutenberg.org/cache/epub/23681/pg23681.txt',
    'turgenev_fathers_ru.md': 'https://www.gutenberg.org/cache/epub/53336/pg53336.txt',
    'dostoevsky_idiot_ru.md': 'https://www.gutenberg.org/cache/epub/26203/pg26203.txt',
    'lermontov_hero_ru.md': 'https://www.gutenberg.org/cache/epub/40049/pg40049.txt',
}

def clean_gutenberg_text(text: str) -> str:
    """Strip Gutenberg header and footer."""
    start_match = re.search(r'\*\*\* START OF TH(IS|E) PROJECT GUTENBERG EBOOK.*?\*\*\*', text, re.IGNORECASE)
    if start_match:
        text = text[start_match.end():]
    end_match = re.search(r'\*\*\* END OF TH(IS|E) PROJECT GUTENBERG EBOOK', text, re.IGNORECASE)
    if end_match:
        text = text[:end_match.start()]
    return text.strip()

def download_gutenberg():
    print("Downloading Russian classic books from Project Gutenberg...")
    for filename, url in GUTENBERG_BOOKS.items():
        dest = CORPUS_DIR / filename
        if dest.exists() and dest.stat().st_size > 10000:
            print(f"  [skip] {filename} already exists ({dest.stat().st_size} bytes)")
            continue
        try:
            req = urllib.request.Request(url, headers=HEADERS)
            with urllib.request.urlopen(req, timeout=20) as resp:
                raw_text = resp.read().decode('utf-8', errors='ignore')
                clean_text = clean_gutenberg_text(raw_text)
                dest.write_text(clean_text, encoding='utf-8')
                print(f"  [saved] {filename} ({len(clean_text)} chars)")
        except Exception as e:
            print(f"  [failed] {filename}: {e}")

def download_ukrainian_ner_corpus():
    print("\nDownloading Ukrainian literature and gold texts from lang-uk/ner-uk...")
    tree_url = 'https://api.github.com/repos/lang-uk/ner-uk/git/trees/master?recursive=1'
    try:
        req = urllib.request.Request(tree_url, headers=HEADERS)
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read().decode('utf-8'))
            tok_files = [item['path'] for item in data.get('tree', []) if item['path'].endswith('.tok.txt')]
            
            # Select 25 diverse Ukrainian literary, historical, and narrative text files
            selected = [p for p in tok_files if 'v1.0/data/' in p][:25]
            print(f"  Found {len(tok_files)} Ukrainian files, selecting {len(selected)} texts...")
            
            for path in selected:
                file_name = Path(path).name.replace('.tok.txt', '.md')
                dest = CORPUS_DIR / f"ua_{file_name}"
                if dest.exists() and dest.stat().st_size > 1000:
                    print(f"  [skip] ua_{file_name} already exists")
                    continue
                raw_url = f"https://raw.githubusercontent.com/lang-uk/ner-uk/master/{path}"
                try:
                    r = urllib.request.Request(raw_url, headers=HEADERS)
                    with urllib.request.urlopen(r, timeout=15) as uresp:
                        text = uresp.read().decode('utf-8', errors='ignore')
                        dest.write_text(text, encoding='utf-8')
                        print(f"  [saved] ua_{file_name} ({len(text)} chars)")
                except Exception as e:
                    print(f"  [failed] {file_name}: {e}")
    except Exception as e:
        print(f"  [failed to fetch tree]: {e}")

def main():
    download_gutenberg()
    download_ukrainian_ner_corpus()
    
    files = list(CORPUS_DIR.glob("*.md")) + list(CORPUS_DIR.glob("*.txt"))
    total_size = sum(f.stat().st_size for f in files)
    print(f"\nCorpus ready in {CORPUS_DIR}: {len(files)} files, {total_size / (1024*1024):.2f} MB total.")

if __name__ == "__main__":
    main()
