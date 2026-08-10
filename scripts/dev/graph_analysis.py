"""
Графовый анализ персонажей и сцен через NetworkX.

Использует J-матрицу (антисимметричную матрицу взаимодействий) из build_j_matrix.py
и расширяет её:

  - Community detection (Louvain) — кластеризация сцен
  - Centrality (betweenness, eigenvector) — кто протагонист
  - Shortest paths — эволюция отношений
  - Subgraph extraction — изоляция сцен

Использование:
    from scripts.dev.graph_analysis import analyze_character_graph
    result = analyze_character_graph(j_matrix, nodes)
"""
from __future__ import annotations

from typing import Optional
import networkx as nx
import numpy as np


# =============================================================================
# ПОСТРОЕНИЕ ГРАФА ИЗ J-МАТРИЦЫ
# =============================================================================

def build_graph_from_j_matrix(j_matrix: np.ndarray, nodes: list[str]) -> nx.DiGraph:
    """Построить направленный граф из антисимметричной J-матрицы.

    J[i,j] > 0 → ребро i → j с весом J[i,j] (i совершил действие над j)
    J[i,j] < 0 → ребро j → i с весом |J[i,j]| (i — жертва)

    Args:
        j_matrix: антисимметричная матрица (n x n)
        nodes: список имён персонажей (n элементов)

    Returns:
        nx.DiGraph с взвешенными рёбрами
    """
    G = nx.DiGraph()
    for node in nodes:
        G.add_node(node)

    n = len(nodes)
    for i in range(n):
        for j in range(n):
            if i == j:
                continue
            w = j_matrix[i, j]
            if w > 0:
                # i → j, вес w (i агрессор/инициатор)
                G.add_edge(nodes[i], nodes[j], weight=float(w), direction='aggressor')
            elif w < 0:
                # j → i, вес |w| (j агрессор, i жертва)
                G.add_edge(nodes[j], nodes[i], weight=float(-w), direction='aggressor')

    return G


# =============================================================================
# CENTRALITY — кто главный персонаж
# =============================================================================

def compute_centrality(G: nx.DiGraph) -> dict[str, dict[str, float]]:
    """Вычислить различные метрики центральности.

    Returns:
        {node_name: {'degree': ..., 'betweenness': ..., 'eigenvector': ..., 'pagerank': ...}}
    """
    result = {}

    # Degree centrality (входящие + исходящие)
    in_deg = nx.in_degree_centrality(G)
    out_deg = nx.out_degree_centrality(G)

    # Betweenness — кто чаще всего "мост" между другими
    try:
        betw = nx.betweenness_centrality(G, weight='weight')
    except Exception:
        betw = {n: 0.0 for n in G.nodes()}

    # Eigenvector centrality — кто связан с важными
    try:
        eig = nx.eigenvector_centrality_numpy(G, weight='weight')
    except Exception:
        eig = {n: 0.0 for n in G.nodes()}

    # PageRank — Google-style centrality
    try:
        pr = nx.pagerank(G, weight='weight')
    except Exception:
        pr = {n: 0.0 for n in G.nodes()}

    for node in G.nodes():
        result[node] = {
            'in_degree': float(in_deg.get(node, 0)),
            'out_degree': float(out_deg.get(node, 0)),
            'total_degree': float(in_deg.get(node, 0) + out_deg.get(node, 0)),
            'betweenness': float(betw.get(node, 0)),
            'eigenvector': float(eig.get(node, 0)),
            'pagerank': float(pr.get(node, 0)),
        }

    return result


def find_protagonist(G: nx.DiGraph) -> tuple[str, dict[str, float]]:
    """Найти протагониста: персонажа с максимальным PageRank.

    Альтернативные критерии:
      - Максимум активности (out-degree)
      - Максимал betweenness (главный мост)
      - Максимум eigenvector (связан с важными)

    Возвращаем: (имя, метрики)
    """
    centralities = compute_centrality(G)
    if not centralities:
        return '', {}

    # Комбинированная метрика: PageRank * 0.5 + out_degree * 0.3 + betweenness * 0.2
    combined = {}
    for node, m in centralities.items():
        combined[node] = (
            m['pagerank'] * 0.5
            + m['out_degree'] * 0.3
            + m['betweenness'] * 0.2
        )

    protagonist = max(combined, key=combined.get)
    return protagonist, centralities[protagonist]


# =============================================================================
# COMMUNITY DETECTION — кластеризация сцен
# =============================================================================

def detect_communities(G: nx.DiGraph, method: str = 'louvain') -> dict[str, int]:
    """Разбить персонажей на сообщества (кластеры сцен).

    Args:
        G: направленный граф взаимодействий
        method: 'louvain' или 'label_propagation'

    Returns:
        {node_name: community_id}
    """
    # Louvain работает на ненаправленных графах
    G_undirected = G.to_undirected()

    if method == 'louvain':
        try:
            communities = nx.community.louvain_communities(G_undirected, weight='weight')
        except Exception:
            # Fallback на label propagation
            communities = nx.community.asyn_lpa_communities(G_undirected, weight='weight')
    elif method == 'label_propagation':
        communities = nx.community.asyn_lpa_communities(G_undirected, weight='weight')
    else:
        raise ValueError(f"Unknown method: {method}")

    result = {}
    for i, comm in enumerate(communities):
        for node in comm:
            result[node] = i

    return result


# =============================================================================
# PATHS — эволюция отношений
# =============================================================================

def find_shortest_paths(G: nx.DiGraph, source: str, target: str) -> list[list[str]]:
    """Найти все кратчайшие пути между двумя персонажами."""
    try:
        return list(nx.all_shortest_paths(G, source, target, weight='weight'))
    except (nx.NetworkXNoPath, nx.NodeNotFound):
        return []


def find_isolated_characters(G: nx.DiGraph) -> list[str]:
    """Найти персонажей без взаимодействий (Observer-Kill candidates)."""
    return [n for n in G.nodes() if G.degree(n) == 0]


# =============================================================================
# ПОЛНЫЙ АНАЛИЗ
# =============================================================================

def analyze_character_graph(j_matrix: np.ndarray, nodes: list[str]) -> dict:
    """Полный графовый анализ персонажей.

    Args:
        j_matrix: антисимметричная матрица взаимодействий
        nodes: список имён персонажей

    Returns:
        dict с ключами:
          - graph: DiGraph (для дальнейшей визуализации)
          - centrality: {node: {metric: value}}
          - protagonist: (name, metrics)
          - communities: {node: community_id}
          - isolated: [node_names]
          - edge_count: int
          - density: float
    """
    G = build_graph_from_j_matrix(j_matrix, nodes)

    centrality = compute_centrality(G)
    protagonist, prot_metrics = find_protagonist(G)
    communities = detect_communities(G)
    isolated = find_isolated_characters(G)

    return {
        'graph': G,
        'centrality': centrality,
        'protagonist': protagonist,
        'protagonist_metrics': prot_metrics,
        'communities': communities,
        'isolated': isolated,
        'edge_count': G.number_of_edges(),
        'node_count': G.number_of_nodes(),
        'density': nx.density(G),
    }


# =============================================================================
# CLI
# =============================================================================

if __name__ == '__main__':
    import sys
    import json

    if len(sys.argv) < 2:
        print("Usage: python -m scripts.dev.graph_analysis <j_matrix.json>")
        sys.exit(1)

    with open(sys.argv[1], encoding='utf-8') as f:
        data = json.load(f)

    nodes = data['nodes']
    matrix = np.array(data['matrix'])

    result = analyze_character_graph(matrix, nodes)

    # Граф не сериализуется — убираем его
    result.pop('graph')

    print(json.dumps(result, ensure_ascii=False, indent=2, default=str))
