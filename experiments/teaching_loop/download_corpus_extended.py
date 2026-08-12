#!/usr/bin/env python3
"""
download_corpus_extended.py — Расширенный корпус для обучения Burn scorer.

Добавляет:
- Больше книг Project Gutenberg на русском
- Ukrainian тексты через raw.githubusercontent (обход rate limit)
- Несколько текстов на украинской классике
"""

import os
import re
import time
import json
import urllib.request
from pathlib import Path

CORPUS_DIR = Path(__file__).resolve().parent / "corpus"
CORPUS_DIR.mkdir(parents=True, exist_ok=True)

HEADERS = {'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) LitGraph/1.0'}

# Расширенный список русских книг Gutenberg
GUTENBERG_BOOKS_RU = {
    # Оригинальные
    'gogol_dead_souls_ru.md': 'https://www.gutenberg.org/cache/epub/12117/pg12117.txt',
    'dostoevsky_crime_ru.md': 'https://www.gutenberg.org/cache/epub/2554/pg2554.txt',
    'tolstoy_anna_ru.md': 'https://www.gutenberg.org/cache/epub/1399/pg1399.txt',
    'chekhov_stories_ru.md': 'https://www.gutenberg.org/cache/epub/13437/pg13437.txt',
    'pushkin_onegin_ru.md': 'https://www.gutenberg.org/cache/epub/23681/pg23681.txt',
    'turgenev_fathers_ru.md': 'https://www.gutenberg.org/cache/epub/53336/pg53336.txt',
    'lermontov_hero_ru.md': 'https://www.gutenberg.org/cache/epub/40049/pg40049.txt',
    # Новые
    'gogol_inspector_ru.md': 'https://www.gutenberg.org/cache/epub/66035/pg66035.txt',
    'gogol_taras_bulba_ru.md': 'https://www.gutenberg.org/cache/epub/60481/pg60481.txt',
    'tolstoy_war_peace_ru.md': 'https://www.gutenberg.org/cache/epub/65582/pg65582.txt',
    'tolstoy_resurrection_ru.md': 'https://www.gutenberg.org/cache/epub/66022/pg66022.txt',
    'dostoevsky_karamazov_ru.md': 'https://www.gutenberg.org/cache/epub/28054/pg28054.txt',
    'dostoevsky_teenager_ru.md': 'https://www.gutenberg.org/cache/epub/61479/pg61479.txt',
    'dostoevsky_poor_people_ru.md': 'https://www.gutenberg.org/cache/epub/61481/pg61481.txt',
    'dostoevsky_gambler_ru.md': 'https://www.gutenberg.org/cache/epub/21941/pg21941.txt',
    'turgenev_nest_ru.md': 'https://www.gutenberg.org/cache/epub/61482/pg61482.txt',
    'turgenev_thunderstorm_ru.md': 'https://www.gutenberg.org/cache/epub/61569/pg61569.txt',
    'chekhov_ward6_ru.md': 'https://www.gutenberg.org/cache/epub/13415/pg13415.txt',
    'chekhov_seagull_ru.md': 'https://www.gutenberg.org/cache/epub/17536/pg17536.txt',
    'chekhov_cherry_orchard_ru.md': 'https://www.gutenberg.org/cache/epub/17537/pg17537.txt',
    'goncharov_oblov_ru.md': 'https://www.gutenberg.org/cache/epub/61475/pg61475.txt',
    'leskov_lady_macbeth_ru.md': 'https://www.gutenberg.org/cache/epub/61483/pg61483.txt',
    'saltykov_golovlevov_ru.md': 'https://www.gutenberg.org/cache/epub/66034/pg66034.txt',
    'karamzin_poor_liza_ru.md': 'https://www.gutenberg.org/cache/epub/67605/pg67605.txt',
    'herzen_who_is_guilty_ru.md': 'https://www.gutenberg.org/cache/epub/68138/pg68138.txt',
    'chernyshevsky_what_to_do_ru.md': 'https://www.gutenberg.org/cache/epub/66033/pg66033.txt',
}

# Ukrainian texts — direct raw URLs (без API GitHub)
UKRAINIAN_TEXTS = {
    'ua_kobzar.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/kobzar.tok.txt',
    'ua_kamenyar.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/kamenyar.tok.txt',
    'ua_tygrolov.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/tygrolov.tok.txt',
    'ua_misto.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/misto.tok.txt',
    'ua_zaklyat.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/zakliat.tok.txt',
    'ua_concordia.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/concordia.tok.txt',
    'ua_fiction_1.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/0043.tok.txt',
    'ua_fiction_2.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/0049.tok.txt',
    'ua_fiction_3.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/0067.tok.txt',
    'ua_fiction_4.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/0078.tok.txt',
    'ua_fiction_5.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/0091.tok.txt',
    'ua_fiction_6.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/fiction/0093.tok.txt',
    'ua_news_1.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0000.tok.txt',
    'ua_news_2.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0001.tok.txt',
    'ua_news_3.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0002.tok.txt',
    'ua_news_4.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0003.tok.txt',
    'ua_news_5.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0004.tok.txt',
    'ua_news_6.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0005.tok.txt',
    'ua_news_7.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0006.tok.txt',
    'ua_news_8.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0007.tok.txt',
    'ua_news_9.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0008.tok.txt',
    'ua_news_10.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0009.tok.txt',
    'ua_news_11.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0010.tok.txt',
    'ua_news_12.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0011.tok.txt',
    'ua_news_13.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0012.tok.txt',
    'ua_news_14.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0013.tok.txt',
    'ua_news_15.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0014.tok.txt',
    'ua_news_16.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0015.tok.txt',
    'ua_news_17.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0016.tok.txt',
    'ua_news_18.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0017.tok.txt',
    'ua_news_19.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0018.tok.txt',
    'ua_news_20.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/news/0019.tok.txt',
    'ua_wiki_1.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0000.tok.txt',
    'ua_wiki_2.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0001.tok.txt',
    'ua_wiki_3.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0002.tok.txt',
    'ua_wiki_4.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0003.tok.txt',
    'ua_wiki_5.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0004.tok.txt',
    'ua_wiki_6.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0005.tok.txt',
    'ua_wiki_7.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0006.tok.txt',
    'ua_wiki_8.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0007.tok.txt',
    'ua_wiki_9.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0008.tok.txt',
    'ua_wiki_10.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0009.tok.txt',
    'ua_wiki_11.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0010.tok.txt',
    'ua_wiki_12.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0011.tok.txt',
    'ua_wiki_13.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0012.tok.txt',
    'ua_wiki_14.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0013.tok.txt',
    'ua_wiki_15.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0014.tok.txt',
    'ua_wiki_16.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0015.tok.txt',
    'ua_wiki_17.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0016.tok.txt',
    'ua_wiki_18.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0017.tok.txt',
    'ua_wiki_19.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0018.tok.txt',
    'ua_wiki_20.md': 'https://raw.githubusercontent.com/lang-uk/ner-uk/master/v1.0/data/wikipedia/0019.tok.txt',
}

def clean_gutenberg_text(text: str) -> str:
    start_match = re.search(r'\*\*\* START OF TH(IS|E) PROJECT GUTENBERG EBOOK.*?\*\*\*', text, re.IGNORECASE)
    if start_match:
        text = text[start_match.end():]
    end_match = re.search(r'\*\*\* END OF TH(IS|E) PROJECT GUTENBERG EBOOK', text, re.IGNORECASE)
    if end_match:
        text = text[:end_match.start()]
    return text.strip()


def download_file(url: str, dest: Path, cleaner=None) -> bool:
    if dest.exists() and dest.stat().st_size > 10000:
        print(f"  [skip] {dest.name} already exists")
        return True
    try:
        req = urllib.request.Request(url, headers=HEADERS)
        with urllib.request.urlopen(req, timeout=20) as resp:
            raw = resp.read().decode('utf-8', errors='ignore')
            text = cleaner(raw) if cleaner else raw
            if len(text) < 500:
                print(f"  [skip] {dest.name} too small ({len(text)} chars)")
                return False
            dest.write_text(text, encoding='utf-8')
            print(f"  [saved] {dest.name} ({len(text)} chars)")
            return True
    except Exception as e:
        print(f"  [failed] {dest.name}: {e}")
        return False


def main():
    print("=" * 60)
    print("STEP 1: Project Gutenberg Russian classics")
    print("=" * 60)
    for filename, url in GUTENBERG_BOOKS_RU.items():
        download_file(url, CORPUS_DIR / filename, cleaner=clean_gutenberg_text)
        time.sleep(0.5)  # be nice

    print("\n" + "=" * 60)
    print("STEP 2: Ukrainian texts (lang-uk/ner-uk via raw URLs)")
    print("=" * 60)
    for filename, url in UKRAINIAN_TEXTS.items():
        download_file(url, CORPUS_DIR / filename)
        time.sleep(0.2)

    files = list(CORPUS_DIR.glob("*.md")) + list(CORPUS_DIR.glob("*.txt"))
    total_size = sum(f.stat().st_size for f in files)
    print(f"\n{'=' * 60}")
    print(f"Corpus ready: {len(files)} files, {total_size / (1024*1024):.2f} MB total")
    print(f"Location: {CORPUS_DIR}")


if __name__ == "__main__":
    main()
