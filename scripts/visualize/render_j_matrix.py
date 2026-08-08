#!/usr/bin/env python3
"""
LitGraph: визуализация J-матрицы (антисимметричной направленной матрицы
взаимодействий) как интерактивного графа на ECharts.

Роль узлов определяется через netAggression:
  > 0  → aggressor (красный, размер ~ |net|)
  < 0  → victim    (синий,  размер ~ |net|)
  ≈ 0  → neutral   (серый)

Рёбра направлены subject → object.
Толщина ~ weight, цвет ~ полярность, пунктир если negated.

Запуск:
    python3 render_j_matrix.py path/to/01_j_matrix.json > /tmp/j.html
    python3 render_j_matrix.py                    # дефолт: берёт 01_conflict_scene
"""

import json
import os
import sys
import html


# ── Палитра (соответствует правилам charts-skill: low-saturation) ──────────
COLOR = {
    "bg":            "#F8FAFC",
    "text":          "#0F172A",
    "textSub":       "#475569",
    "textMuted":     "#94A3B8",
    "grid":          "#E2E8F0",
    "aggressorFill": "#FEE2E2",   # лёгкий красный фон узла
    "aggressorBd":   "#DC2626",   # насыщенная красная рамка
    "victimFill":    "#DBEAFE",   # лёгкий синий фон
    "victimBd":      "#1D4ED8",
    "neutralFill":   "#F1F5F9",
    "neutralBd":     "#64748B",
    "edgeNeg":       "#DC2626",   # красная стрелка = агрессия
    "edgeNeu":       "#64748B",   # серая = нейтральное действие
    "edgePos":       "#059669",   # зелёная = помощь
    "edgeNegated":   "#475569",   # пунктир = negated (тёмнее, иначе невидим)
    "edgeLabelBg":   "rgba(255,255,255,0.95)",
    "edgeLabelTx":   "#0F172A",
}


def node_color_and_size(net: float):
    """Вернёт (fill, border, symbolSize) по netAggression."""
    abs_net = abs(net)
    size = 38 + min(abs_net * 8, 30)   # 38..68
    if net > 0.1:
        return COLOR["aggressorFill"], COLOR["aggressorBd"], size
    if net < -0.1:
        return COLOR["victimFill"], COLOR["victimBd"], size
    return COLOR["neutralFill"], COLOR["neutralBd"], size


def edge_color_and_width(weight: float, polarity: str, negated: bool):
    """Вернёт (color, width, dash) для ребра.
    Negated-рёбра дополнительно утолщаются, иначе пунктир теряется."""
    width = 1.8 + min(weight * 1.0, 2.2)   # 1.8..4.0
    if negated:
        return COLOR["edgeNegated"], max(width, 2.5), "dashed"
    if polarity == "negative":
        return COLOR["edgeNeg"], width, "solid"
    if polarity == "positive":
        return COLOR["edgePos"], width, "solid"
    return COLOR["edgeNeu"], width, "solid"


def position_nodes(nodes, net_aggression):
    """
    Фиксированные координаты: aggressors сверху, victims снизу.
    Ширина графа подгоняется под контейнер, чтобы заполнить холст.
    Верхний ряд сдвинут ВНИЗ (y=200), чтобы не пересекаться с заголовком.
    """
    sorted_nodes = sorted(nodes, key=lambda n: -net_aggression.get(n, 0))
    n = len(sorted_nodes)
    half = (n + 1) // 2
    top = sorted_nodes[:half]
    bottom = sorted_nodes[half:]
    positions = {}
    canvas_w = 980
    canvas_h = 700
    margin_x = 140
    usable_w = canvas_w - 2 * margin_x
    # Верхняя строка — сдвинута вниз, чтобы не пересекаться с заголовком
    top_y = 260
    bot_y = canvas_h - 160
    for i, name in enumerate(top):
        x = margin_x + usable_w * (i + 0.5) / max(len(top), 1)
        positions[name] = (x, top_y)
    for i, name in enumerate(bottom):
        x = margin_x + usable_w * (i + 0.5) / max(len(bottom), 1)
        positions[name] = (x, bot_y)
    return positions


def render(j_data: dict) -> str:
    nodes = j_data["nodes"]
    matrix = j_data["matrix"]
    edges = j_data.get("edges", [])
    raw_edges = j_data.get("rawEdges", [])
    net = j_data.get("netAggression", {})
    stats = j_data.get("stats", {})

    positions = position_nodes(nodes, net)

    # ── Узлы ───────────────────────────────────────────────────────
    echarts_nodes = []
    for name in nodes:
        fill, border, size = node_color_and_size(net.get(name, 0))
        x, y = positions[name]
        echarts_nodes.append({
            "name": name,
            "x": x, "y": y,
            "symbolSize": size,
            "category": 0 if net.get(name, 0) > 0.1 else (1 if net.get(name, 0) < -0.1 else 2),
            "itemStyle": {
                "color": fill,
                "borderColor": border,
                "borderWidth": 2.5,
            },
            "label": {
                "show": True,
                "position": "inside",
                "formatter": "{b}",
                "fontSize": 13,
                "fontWeight": "bold",
                "color": COLOR["text"],
            },
            "tooltip": {
                "formatter": (
                    f"<b>{html.escape(name)}</b><br/>"
                    f"netAggression: <b>{net.get(name, 0):+.2f}</b><br/>"
                    f"Роль: {'Агрессор' if net.get(name, 0) > 0.1 else ('Жертва' if net.get(name, 0) < -0.1 else 'Нейтрал')}"
                )
            },
        })

    # ── Ребра ──────────────────────────────────────────────────────
    # ECharts graph links используют source/target по name.
    # Для tooltip по ребру — нужен rawEdges, чтобы показать предложение.
    raw_by_pair = {}
    for re_ in raw_edges:
        raw_by_pair.setdefault((re_["from"], re_["to"]), []).append(re_)

    echarts_links = []
    for e in edges:
        f, t, w = e["from"], e["to"], e["weight"]
        # Найдём raw-ребро, чтобы достать polarity/negated/sentence
        raws = raw_by_pair.get((f, t), [])
        polarity = raws[0]["polarity"] if raws else "neutral"
        negated = any(r.get("negated") for r in raws)
        verbs = e.get("verbs", [])

        color, width, dash = edge_color_and_width(w, polarity, negated)

        link = {
            "source": f,
            "target": t,
            "value": w,
            "lineStyle": {
                "color": color,
                "width": width,
                "curveness": 0.25,
                "type": dash,
            },
            "label": {
                "show": True,
                "position": "middle",
                "formatter": ", ".join(verbs),
                "fontSize": 13,
                "fontWeight": "bold",
                "color": COLOR["edgeLabelTx"],
                "backgroundColor": COLOR["edgeLabelBg"],
                "borderColor": color,
                "borderWidth": 1,
                "padding": [4, 8],
                "borderRadius": 4,
            },
            "tooltip": {
                "formatter": _edge_tooltip(f, t, w, polarity, negated, verbs, raws),
            },
        }
        echarts_links.append(link)

    # ── Опции ECharts ──────────────────────────────────────────────
    title = "LitGraph: рентген сцены конфликта"
    subtitle = (
        f"01_conflict_scene.md  •  {len(nodes)} персонажа  •  "
        f"{len(edges)} направленных действий  •  J-матрица v{j_data.get('svoVersion', '?')}"
    )
    # Заголовок занимает левую часть, узлы отодвинуты от верха.

    aggressors = stats.get("aggressors", [])
    victims = stats.get("victims", [])
    summary = (
        f"Главный агрессор: {aggressors[0][0]} (+{aggressors[0][1]:.1f})  •  "
        f"Жертвы: {', '.join(f'{v[0]} ({v[1]:+.1f})' for v in victims)}"
        if aggressors and victims else ""
    )

    option = {
        "backgroundColor": COLOR["bg"],
        "textStyle": {
            "fontFamily": "system-ui, 'Noto Sans SC', 'Segoe UI', sans-serif",
            "color": COLOR["text"],
        },
        "tooltip": {
            "trigger": "item",
            "backgroundColor": "#1E293B",
            "borderColor": "#475569",
            "borderWidth": 1,
            "textStyle": {"color": "#F1F5F9", "fontSize": 12},
            "extraCssText": "max-width: 480px; white-space: normal;",
        },
        "legend": {
            "show": False,   # HTML-легенда сверху достаточно, эта дублирует
            "data": ["Агрессор", "Жертва", "Нейтрал"],
            "top": 20, "right": 32,
            "textStyle": {"color": COLOR["textSub"], "fontSize": 11},
            "itemWidth": 14, "itemHeight": 14,
        },
        "series": [{
            "type": "graph",
            "layout": "none",
            "data": echarts_nodes,
            "links": echarts_links,
            "categories": [
                {"name": "Агрессор",  "itemStyle": {"color": COLOR["aggressorFill"], "borderColor": COLOR["aggressorBd"]}},
                {"name": "Жертва",    "itemStyle": {"color": COLOR["victimFill"],    "borderColor": COLOR["victimBd"]}},
                {"name": "Нейтрал",   "itemStyle": {"color": COLOR["neutralFill"],   "borderColor": COLOR["neutralBd"]}},
            ],
            "roam": True,
            "draggable": False,
            "edgeSymbol": ["none", "arrow"],
            "edgeSymbolSize": [0, 18],
            "edgeLabel": {"show": True},
            "emphasis": {
                "focus": "adjacency",
                "lineStyle": {"width": 4},
                "label": {"fontSize": 13, "fontWeight": "bold"},
            },
            "lineStyle": {"curveness": 0.25},
        }],
        "graphic": [{
            "type": "text",
            "right": 32, "bottom": 24,
            "style": {
                "text": summary,
                "fontSize": 11,
                "fill": COLOR["textSub"],
                "textAlign": "right",
            },
        }],
    }

    option_json = json.dumps(option, ensure_ascii=False, indent=2)

    # ── HTML shell ─────────────────────────────────────────────────
    return f"""<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="UTF-8">
<title>LitGraph: J-матрица конфликта</title>
<script src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    background: {COLOR['bg']};
    font-family: system-ui, 'Noto Sans SC', 'Segoe UI', sans-serif;
    color: {COLOR['text']};
    min-height: 100vh;
  }}
  #chart {{ width: 980px; height: 700px; margin: 40px auto; }}
  .legend-bar {{
    max-width: 980px; margin: 0 auto 16px; padding: 16px 24px;
    background: #FFFFFF; border: 1px solid {COLOR['grid']}; border-radius: 8px;
  }}
  .header-block {{ margin-bottom: 12px; padding-bottom: 12px; border-bottom: 1px solid {COLOR['grid']}; }}
  .title {{ font-size: 19px; font-weight: bold; color: {COLOR['text']}; }}
  .subtitle {{ font-size: 12px; color: {COLOR['textSub']}; margin-top: 4px; }}
  .legend-items {{ display: flex; gap: 20px; align-items: center; flex-wrap: wrap; font-size: 12px; color: {COLOR['textSub']}; }}
  .legend-item {{ display: flex; align-items: center; gap: 8px; }}
  .swatch {{ width: 14px; height: 14px; border-radius: 3px; border: 2px solid; }}
  .edge-line {{ width: 28px; height: 0; border-top: 3px solid; }}
  .edge-line.dashed {{ border-top-style: dashed; }}
</style>
</head>
<body>
  <div class="legend-bar">
    <div class="header-block">
      <div class="title">LitGraph: рентген сцены конфликта</div>
      <div class="subtitle">{subtitle}</div>
    </div>
    <div class="legend-items">
      <div class="legend-item">
        <span class="swatch" style="background:{COLOR['aggressorFill']};border-color:{COLOR['aggressorBd']}"></span>
        Агрессор (net J &gt; 0)
      </div>
      <div class="legend-item">
        <span class="swatch" style="background:{COLOR['victimFill']};border-color:{COLOR['victimBd']}"></span>
        Жертва (net J &lt; 0)
      </div>
      <div class="legend-item">
        <span class="edge-line" style="border-color:{COLOR['edgeNeg']}"></span>
        Агрессия (neg polarity)
      </div>
      <div class="legend-item">
        <span class="edge-line" style="border-color:{COLOR['edgeNeu']}"></span>
        Нейтральное действие
      </div>
      <div class="legend-item">
        <span class="edge-line dashed" style="border-color:{COLOR['edgeNegated']}"></span>
        Negated (не сделал)
      </div>
      <div class="legend-item">
        Толщина линии ~ вес действия
      </div>
    </div>
  </div>
  <div id="chart"></div>
  <script>
    const chart = echarts.init(document.getElementById('chart'));
    const option = {option_json};
    chart.setOption(option);
    window.addEventListener('resize', () => chart.resize());
  </script>
</body>
</html>
"""


def _edge_tooltip(f, t, w, polarity, negated, verbs, raws):
    parts = [f"<b>{html.escape(f)} → {html.escape(t)}</b>"]
    parts.append(f"Вес: <b>{w:+.2f}</b>  •  Полярность: {polarity}" +
                 ("  •  <b>negated</b>" if negated else ""))
    parts.append(f"Глагол(ы): <i>{', '.join(verbs)}</i>")
    if raws:
        parts.append("<hr style='border-color:#475569;margin:6px 0'/>"
                     "<b>Контекст:</b>")
        for r in raws[:3]:
            sent = html.escape(r.get("sentence", "")[:200])
            neg = " <i>[negated]</i>" if r.get("negated") else ""
            pron = " <i>[pron→" + html.escape(str(r.get("pronounResolvedTo", ""))) + "]</i>" if r.get("pronounResolved") else ""
            parts.append(f"<div style='margin-top:4px;color:#CBD5E1'>«{sent}…»{neg}{pron}</div>")
    return "<br/>".join(parts)


def main():
    if len(sys.argv) > 1:
        path = sys.argv[1]
    else:
        # Дефолт: ищем 01_j_matrix.json в репо
        here = os.path.dirname(os.path.abspath(__file__))
        repo = os.path.dirname(os.path.dirname(here))
        path = os.path.join(repo, "tests/corpus/results/svo/01_j_matrix.json")
    with open(path, "r", encoding="utf-8") as f:
        j_data = json.load(f)
    print(render(j_data))


if __name__ == "__main__":
    main()
