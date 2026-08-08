#!/usr/bin/env python3
"""
LitGraph conflict_graph.py — единый Python-пайплайн для Tauri-команды
get_conflict_graph:

    текст → NER → SVO-триплеты → J-матрица (антисимметричная) → JSON

Возвращает структуру ConflictGraph:
{
  "nodes":   [ {character, outgoing, incoming, balance, role} ],
  "edges":   [ {from, to, weight, verbCount, verbs[], polarity, negated,
                 pronounResolved, sentence} ],
  "matrix":  [[float]],
  "stats":   {nodeCount, edgeCount, rawTripletCount,
              aggressors[], victims[], neutral[]},
  "model":   "ru_core_news_sm",
  "version": "0.1.0"
}

Запуск (для отладки):
    python3 conflict_graph.py input_text.txt
"""

import sys
import os
import json
import re
from collections import defaultdict

# Добавляем директорию скрипта в sys.path, чтобы import ner_extract
# и import svo_extract работали даже из /tmp/litgraph_scripts_PID/
_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)

# Грейсфул-импорт — даём понятное сообщение для пользователя Tauri-приложения
try:
    from svo_extract import extract_svo  # noqa: E402
except ImportError as e:
    print(json.dumps({
        "error": (
            "Не удалось импортировать svo_extract. Убедитесь, что "
            "svo_extract.py лежит рядом с conflict_graph.py. " +
            f"Детали: {e}"
        ),
    }, ensure_ascii=False))
    sys.exit(1)


# ────────────────────────────────────────────────────────────────────
# J-matrix builder (перенесено из build_j_matrix.py v0.1.0)
# ────────────────────────────────────────────────────────────────────

def polarity_weight(polarity: str, negated: bool) -> float:
    """Вес направления для POLER J-матрицы."""
    if polarity == "negative":
        w = 2.0
    elif polarity == "positive":
        w = 1.0
    else:
        w = 1.0
    if negated:
        # Отрицание: действие не совершено, но намерение было.
        # Ослабляем вес, но сохраняем знак.
        w *= 0.3
    return w


def build_j_matrix(triplets: list) -> dict:
    """Построить J-матрицу и агрегированные рёбра из SVO-триплетов."""
    nodes_set = set()
    for t in triplets:
        nodes_set.add(t["subjectLemma"])
        nodes_set.add(t["objectLemma"])
    nodes = sorted(nodes_set)
    node_idx = {n: i for i, n in enumerate(nodes)}

    n = len(nodes)
    matrix = [[0.0] * n for _ in range(n)]
    raw_edges = []

    for t in triplets:
        s = t["subjectLemma"]
        o = t["objectLemma"]
        if s == o:
            continue
        w = polarity_weight(t["polarity"], t.get("negated", False))
        i, j = node_idx[s], node_idx[o]
        matrix[i][j] += w
        matrix[j][i] -= w
        raw_edges.append({
            "from": s,
            "to": o,
            "weight": w,
            "verb": t["verbLemma"],
            "polarity": t["polarity"],
            "negated": t.get("negated", False),
            "pronounResolved": t.get("pronounResolved", False),
            "pronounResolvedTo": t.get("pronounResolvedTo"),
            "sentence": (t.get("sentence") or "")[:200],
        })

    # Агрегируем рёбра по паре (from, to)
    edge_agg = defaultdict(lambda: {
        "weight": 0.0, "verbs": [], "count": 0,
        "polarity": "neutral", "negated": False,
        "pronounResolved": False, "sentence": "",
    })
    for e in raw_edges:
        key = (e["from"], e["to"])
        edge_agg[key]["weight"] += e["weight"]
        edge_agg[key]["verbs"].append(e["verb"])
        edge_agg[key]["count"] += 1
        # Берём репрезентативный пример (берём первый, но если есть
        # negated — приоритет ему, для визуализации пунктиром)
        if e["negated"] or not edge_agg[key]["sentence"]:
            edge_agg[key]["polarity"] = e["polarity"]
            edge_agg[key]["negated"] = e["negated"]
            edge_agg[key]["pronounResolved"] = e["pronounResolved"]
            edge_agg[key]["sentence"] = e["sentence"]

    aggregated_edges = []
    for (f, t), v in edge_agg.items():
        aggregated_edges.append({
            "from": f,
            "to": t,
            "weight": round(v["weight"], 3),
            "verbCount": v["count"],
            "verbs": sorted(set(v["verbs"])),
            "polarity": v["polarity"],
            "negated": v["negated"],
            "pronounResolved": v["pronounResolved"],
            "sentence": v["sentence"],
        })

    # Net aggression per node
    net_aggression = {}
    for i, node in enumerate(nodes):
        net = sum(matrix[i][j] for j in range(n))
        net_aggression[node] = round(net, 3)

    # Outgoing/incoming per node
    outgoing = {node: 0.0 for node in nodes}
    incoming = {node: 0.0 for node in nodes}
    for e in aggregated_edges:
        outgoing[e["from"]] += e["weight"]
        incoming[e["to"]] += e["weight"]

    return {
        "nodes": nodes,
        "matrix": [[round(v, 3) for v in row] for row in matrix],
        "edges": aggregated_edges,
        "rawEdges": raw_edges,
        "netAggression": net_aggression,
        "outgoing": outgoing,
        "incoming": incoming,
    }


# ────────────────────────────────────────────────────────────────────
# Build ConflictGraph response
# ────────────────────────────────────────────────────────────────────

def build_conflict_graph(text: str) -> dict:
    """Полный пайплайн: текст → SVO → J-матрица → ConflictGraph JSON."""
    # SVO-извлечение (внутри уже делает NER)
    svo_result = extract_svo(text, use_ner=True)
    triplets = svo_result.get("triplets", [])

    # Если SVO упал с ошибкой — прокидываем
    if "error" in svo_result:
        return svo_result

    # J-матрица
    j = build_j_matrix(triplets)

    # Формируем nodes с ролями
    nodes_out = []
    for name in j["nodes"]:
        net = j["netAggression"].get(name, 0.0)
        out = j["outgoing"].get(name, 0.0)
        inc = j["incoming"].get(name, 0.0)
        if net > 0.1:
            role = "aggressor"
        elif net < -0.1:
            role = "victim"
        else:
            role = "neutral"
        nodes_out.append({
            "character": name,
            "outgoing": round(out, 3),
            "incoming": round(inc, 3),
            "balance": round(net, 3),
            "role": role,
        })

    # Сортируем nodes по |balance| DESC — главные действующие лица сверху
    nodes_out.sort(key=lambda x: -abs(x["balance"]))

    # Сортируем edges по weight DESC
    edges_out = sorted(j["edges"], key=lambda e: -abs(e["weight"]))

    # Stats
    aggressors = sorted(
        [(n["character"], n["balance"]) for n in nodes_out if n["role"] == "aggressor"],
        key=lambda x: -x[1],
    )
    victims = sorted(
        [(n["character"], n["balance"]) for n in nodes_out if n["role"] == "victim"],
        key=lambda x: x[1],
    )
    neutral = [n["character"] for n in nodes_out if n["role"] == "neutral"]

    return {
        "nodes": nodes_out,
        "edges": edges_out,
        "matrix": j["matrix"],
        "nodeOrder": j["nodes"],  # исходный порядок (для матрицы)
        "stats": {
            "nodeCount": len(nodes_out),
            "edgeCount": len(edges_out),
            "rawTripletCount": len(triplets),
            "aggressors": aggressors,
            "victims": victims,
            "neutral": neutral,
        },
        "model": svo_result.get("model", "ru_core_news_sm"),
        "version": "0.1.0",
        "svoVersion": svo_result.get("version", "unknown"),
        "textLength": len(text),
    }


def main():
    try:
        if len(sys.argv) > 1:
            with open(sys.argv[1], "r", encoding="utf-8") as f:
                text = f.read()
        else:
            text = sys.stdin.read()

        result = build_conflict_graph(text)
        print(json.dumps(result, ensure_ascii=False, indent=2))
    except Exception as e:
        import traceback
        print(json.dumps({
            "error": str(e),
            "type": type(e).__name__,
            "traceback": traceback.format_exc(),
        }, ensure_ascii=False))
        sys.exit(1)


if __name__ == "__main__":
    main()
