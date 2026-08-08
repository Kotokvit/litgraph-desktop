#!/usr/bin/env python3
"""
SVO-debug: дамп dependency-parse ключевых предложений из 01_conflict_scene.md
для диагностики, почему svo_extract.py находит только 1 триплет.

Печатает: verb, dep, all children with their dep_/pos_/text, и помечает,
которые из них удовлетворяют is_person_name.
"""

import re
import sys
import os

# Setup import path — repo root is one level up from scripts/
_HERE = os.path.dirname(os.path.abspath(__file__))
_LITGRAPH = os.path.dirname(_HERE)
sys.path.insert(0, os.path.join(_LITGRAPH, "src-tauri", "python"))

import spacy
try:
    import pymorphy3
    MORPH = pymorphy3.MorphAnalyzer()
except ImportError:
    MORPH = None

NLP = spacy.load("ru_core_news_sm")

from ner_extract import extract_entities, FALSE_POSITIVE_NOUNS


# Тестовые предложения (без HTML-комментариев)
SENTENCES = [
    # 1. ✅ работает
    "Алексей посмотрел на Сорокина с холодным вызовом.",
    # 2. pronoun object: "Сорокин остановил его"
    "Фёдор, стоявший у двери, сделал шаг вперёд, но Сорокин остановил его едва заметным движением ладони.",
    # 3. multi-token subject: "Владимир Петрович поднял голову"
    "Владимир Петрович поднял голову от бумаг.",
    # 4. pro-drop subject in conj chain: "Алексей подошёл к двери, открыл её и увидел в коридоре Фёдора."
    "Алексей подошёл к двери, открыл её и увидел в коридоре Фёдора.",
    # 5. pronoun + pro-drop: "Алексей узнал её по серому пальто"
    "Марина Игоревна ждала у лифта — она стояла спиной, но Алексей узнал её по серому пальто.",
    # 6. "Сорокин не остановил его." (after "Алексей взял папку со стола.")
    "Алексей взял папку со стола. Сорокин не остановил его.",
    # 7. "Фёдор, — сказал Сорокин, не поворачиваясь, — оставь нас."
    "— Фёдор, — сказал Сорокин, не поворачиваясь, — оставь нас.",
    # 8. "Алексей ответил Сорокину"
    "— Я не опоздал, — ответил Алексей Сорокину.",
]


def is_person_name_token(text: str, known_persons: set) -> bool:
    if text in known_persons:
        return True
    if MORPH is not None:
        try:
            for p in MORPH.parse(text):
                tags_str = str(p.tag)
                if "Name" in tags_str or "Surn" in tags_str:
                    if p.normal_form and (p.normal_form[0].upper() + p.normal_form[1:]) in known_persons:
                        return True
        except Exception:
            pass
    if re.match(r"^[А-ЯЁ][а-яё]{2,14}$", text):
        if text.lower() not in FALSE_POSITIVE_NOUNS:
            return True
    return False


def dump_sentence(sent, known_persons):
    print(f"\n┌── SENT: «{sent.text.strip()[:120]}»")
    for t in sent:
        if t.pos_ == "VERB":
            print(f"│ VERB «{t.text}» lemma={t.lemma_} dep={t.dep_} idx={t.idx}")
            for ch in t.children:
                is_per = is_person_name_token(ch.text, known_persons)
                flag = "✓PER" if is_per else "    "
                # multi-token span (head + flat:name children)
                span_text = ch.text
                for sub in ch.children:
                    if sub.dep_ in ("flat:name", "flat", "appos", "amod"):
                        span_text += " " + sub.text
                print(f"│   child «{ch.text}» span=«{span_text}» "
                      f"dep={ch.dep_} pos={ch.pos_} morph={str(ch.morph)} {flag}")
    print("└──")


def main():
    # Извлекаем known_persons так же, как в svo_extract.py
    with open(os.path.join(_LITGRAPH, "tests/corpus/01_conflict_scene.md"), encoding="utf-8") as f:
        full_text = f.read()
    # Strip HTML comments, как в ner_extract v0.2.1
    full_text = re.sub(r"<!--.*?-->", "", full_text, flags=re.DOTALL)

    ner_result = extract_entities(full_text)
    known_persons = set()
    for ent in ner_result.get("entities", []):
        if ent["label"] == "PER":
            known_persons.add(ent["lemma"])
            for form in ent["forms"]:
                known_persons.add(form)
    print(f"=== Known persons ({len(known_persons)}): {sorted(known_persons)}")

    print("\n" + "=" * 80)
    print("DUMP dependency parse for diagnostic sentences")
    print("=" * 80)
    for s in SENTENCES:
        doc = NLP(s)
        for sent in doc.sents:
            dump_sentence(sent, known_persons)


if __name__ == "__main__":
    main()
