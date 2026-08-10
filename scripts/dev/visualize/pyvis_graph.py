"""
Визуализация J-матрицы и графа персонажей через pyvis.

Создаёт интерактивный HTML-граф, который можно открыть в браузере.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path
import numpy as np

# Setup path
_HERE = os.path.dirname(os.path.abspath(__file__))
_PROJECT_ROOT = os.path.abspath(os.path.join(_HERE, '..', '..'))
if _PROJECT_ROOT not in sys.path:
    sys.path.insert(0, _PROJECT_ROOT)

from pyvis.network import Network


def visualize_j_matrix(
    j_matrix: np.ndarray,
    nodes: list[str],
    output_path: str = 'j_matrix_graph.html',
    title: str = 'J-Matrix Character Graph',
) -> str:
    """Создать интерактивный HTML-граф из J-матрицы.

    Args:
        j_matrix: антисимметричная матрица взаимодействий (n x n)
        nodes: список имён персонажей
        output_path: куда сохранить HTML
        title: заголовок графа

    Returns:
        Абсолютный путь к созданному файлу
    """
    net = Network(
        height='800px',
        width='100%',
        directed=True,
        notebook=False,
        heading=title,
    )

    # Настройка физики — пружинная укладка
    net.barnes_hut(
        gravity=-3000,
        central_gravity=0.3,
        spring_length=200,
        spring_strength=0.05,
        damping=0.4,
    )

    # Цвета по сумме J-строки (агрессивность персонажа)
    aggression_scores = np.maximum(j_matrix.sum(axis=1), 0)  # только положительные = агрессия
    victim_scores = np.maximum(-j_matrix.sum(axis=1), 0)  # отрицательные = жертва

    max_agg = max(aggression_scores.max() if len(aggression_scores) else 0, 1.0)
    max_vic = max(victim_scores.max() if len(victim_scores) else 0, 1.0)

    # Добавляем узлы
    for i, name in enumerate(nodes):
        agg = float(aggression_scores[i]) / max_agg
        vic = float(victim_scores[i]) / max_vic

        # Цвет: красный для агрессоров, синий для жертв
        if agg > vic:
            # Красный, насыщенность по agg
            r = int(255 * min(agg, 1.0))
            color = f'rgb({r}, 50, 50)'
        else:
            # Синий
            b = int(255 * min(vic, 1.0))
            color = f'rgb(50, 50, {b})'

        # Размер узла по общей активности
        total_activity = abs(j_matrix[i]).sum()
        size = 20 + 5 * min(total_activity / 3.0, 5)

        net.add_node(
            name,
            label=name,
            color=color,
            size=size,
            title=f'{name}\nAggression: {agg:.2f}\nVictim: {vic:.2f}',
        )

    # Добавляем рёбра (только значимые)
    n = len(nodes)
    for i in range(n):
        for j in range(n):
            if i == j:
                continue
            w = j_matrix[i, j]
            if abs(w) < 0.5:
                continue  # фильтр слабых рёбер

            # Толщина ребра по весу
            width = min(abs(w), 5.0)

            # Цвет: тёмно-красный для агрессии, зелёный для помощи
            if w > 0:
                color = '#cc3333'  # агрессия
            else:
                color = '#33aa33'  # помощь (J[i,j] < 0 означает, что j помог i)

            net.add_edge(
                nodes[i], nodes[j],
                value=float(abs(w)),
                title=f'{nodes[i]} → {nodes[j]}: weight={w:.2f}',
                color=color,
                width=width,
                arrows='to',
            )

    # Сохраняем
    output_path = Path(output_path).resolve()
    net.save_graph(str(output_path))
    return str(output_path)


if __name__ == '__main__':
    # Демо с синтетической J-матрицей
    if len(sys.argv) >= 2:
        import json
        with open(sys.argv[1], encoding='utf-8') as f:
            data = json.load(f)
        nodes = data['nodes']
        matrix = np.array(data['matrix'])
        out = sys.argv[2] if len(sys.argv) >= 3 else 'j_matrix_graph.html'
    else:
        # Демо-данные
        nodes = ['Алексей', 'Сорокин', 'Фёдор', 'Марина']
        matrix = np.array([
            [0.0, 2.0, 0.5, 1.0],
            [-2.0, 0.0, 1.5, 0.0],
            [-0.5, -1.5, 0.0, 2.0],
            [-1.0, 0.0, -2.0, 0.0],
        ])
        out = '/home/z/my-project/litgraph-desktop/scripts/dev/visualize/demo_j_matrix.html'

    path = visualize_j_matrix(matrix, nodes, out)
    print(f"Graph saved to: {path}")
    print(f"Open with: file://{path}")
