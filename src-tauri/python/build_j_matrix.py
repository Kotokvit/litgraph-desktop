#!/usr/bin/env python3
"""
Построение J-матрицы (antisymmetric directed interaction matrix)
из SVO-триплетов для POLER-физики LitGraph.

J[i,j] = +w  если i → j (субъект i совершил действие над j)
J[j,i] = -w  (антисимметрия)

Вес w зависит от полярности:
  negative (агрессия): w = +2
  positive (помощь):   w = +1
  neutral:             w = +1
  negated (не сделал): w *= 0.3  (ослабленное действие)

J[i,j] > 0 → i агрессор по отношению к j
J[i,j] < 0 → i жертва по отношению к j

Использование:
    python3 build_j_matrix.py svo_result.json > j_matrix.json
"""

import sys
import json
from collections import defaultdict


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
        # Ослабляем вес, но сохраняем знак (намерение агрессии).
        w *= 0.3
    return w


def build_j_matrix(triplets: list) -> dict:
    """Построить J-матрицу из SVO-триплетов."""
    # Собираем всех участников
    nodes = set()
    for t in triplets:
        nodes.add(t["subjectLemma"])
        nodes.add(t["objectLemma"])
    nodes = sorted(nodes)
    node_idx = {n: i for i, n in enumerate(nodes)}
    
    # Инициализируем матрицу нулями
    n = len(nodes)
    matrix = [[0.0] * n for _ in range(n)]
    
    # Заполняем матрицу
    edges = []
    for t in triplets:
        s = t["subjectLemma"]
        o = t["objectLemma"]
        if s == o:
            continue  # рефлексивные пропускаем
        w = polarity_weight(t["polarity"], t.get("negated", False))
        i, j = node_idx[s], node_idx[o]
        matrix[i][j] += w
        matrix[j][i] -= w  # антисимметрия
        
        edges.append({
            "from": s,
            "to": o,
            "weight": w,
            "verb": t["verbLemma"],
            "polarity": t["polarity"],
            "negated": t.get("negated", False),
            "pronounResolved": t.get("pronounResolved", False),
            "sentence": t["sentence"][:150],
        })
    
    # Агрегированные рёбра (сумма весов по парам)
    edge_agg = defaultdict(lambda: {"weight": 0.0, "verbs": [], "count": 0})
    for e in edges:
        key = (e["from"], e["to"])
        edge_agg[key]["weight"] += e["weight"]
        edge_agg[key]["verbs"].append(e["verb"])
        edge_agg[key]["count"] += 1
    
    aggregated_edges = []
    for (f, t), v in edge_agg.items():
        aggregated_edges.append({
            "from": f,
            "to": t,
            "weight": round(v["weight"], 3),
            "verbCount": v["count"],
            "verbs": list(set(v["verbs"])),
        })
    
    # Считаем "net aggression" для каждого узла
    # net[i] = sum_j J[i,j]  (positive = net aggressor, negative = net victim)
    net_aggression = {}
    for i, node in enumerate(nodes):
        net = sum(matrix[i][j] for j in range(n))
        net_aggression[node] = round(net, 3)
    
    return {
        "nodes": nodes,
        "matrix": [[round(v, 3) for v in row] for row in matrix],
        "edges": aggregated_edges,
        "rawEdges": edges,
        "netAggression": net_aggression,
        "stats": {
            "nodeCount": n,
            "edgeCount": len(aggregated_edges),
            "rawTripletCount": len(triplets),
            "aggressors": sorted(
                [(n, v) for n, v in net_aggression.items() if v > 0],
                key=lambda x: -x[1]
            ),
            "victims": sorted(
                [(n, v) for n, v in net_aggression.items() if v < 0],
                key=lambda x: x[1]
            ),
            "neutral": [n for n, v in net_aggression.items() if v == 0],
        },
    }


def main():
    if len(sys.argv) > 1:
        with open(sys.argv[1], "r", encoding="utf-8") as f:
            svo_data = json.load(f)
    else:
        svo_data = json.load(sys.stdin)
    
    triplets = svo_data.get("triplets", [])
    j_matrix = build_j_matrix(triplets)
    j_matrix["sourceFile"] = sys.argv[1] if len(sys.argv) > 1 else "stdin"
    j_matrix["svoVersion"] = svo_data.get("version", "unknown")
    
    print(json.dumps(j_matrix, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
