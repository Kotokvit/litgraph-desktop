#!/usr/bin/env python3
"""
POLER-анализ графа персонажей.

Принимает текст → извлекает персонажей через NER → строит граф co-occurrence
→ запускает POLER-динамику → возвращает кластеры персонажей.

Это интеграция Слоя 1 (NER) + Слоя 2 (POLER-физика).

Использование:
    cat book.md | python3 poler_entities.py

Выход (JSON):
{
  "entities": [...],          # NER результат
  "graph": {
    "nodes": ["Алексей", "Веня", ...],
    "edges": [{"source": "Алексей", "target": "Веня", "weight": 15}, ...],
    "nNodes": 8,
    "nEdges": 25
  },
  "poler": {
    "eigenvalues": [...],
    "clusters": [{"cluster": 0, "characters": ["Алексей", "Веня"], "size": 2}, ...],
    "silhouette": 0.42
  }
}
"""

import sys
import json
import numpy as np
from collections import defaultdict
from scipy import sparse
from scipy.sparse.linalg import eigsh
from sklearn.cluster import KMeans
from sklearn.metrics import silhouette_score

# Импортируем NER и SVO.
# V3: вычисляем путь к своей директории (где лежит ner_extract.py).
# Раньше было sys.path.insert(0, ".") — это добавляло текущую рабочую
# директорию Tauri, а не папку со скриптами → ModuleNotFoundError.
import os
_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)
from ner_extract import extract_entities, NLP
from svo_extract import extract_svo


def extract_character_mentions(text: str, characters: list) -> dict:
    """Для каждого персонажа найти все позиции упоминаний в тексте.
    
    characters: [{"lemma": "Алексей", "forms": ["Алексей", "Алексея", ...]}, ...]
    Returns: {"Алексей": [pos1, pos2, ...], "Веня": [pos1, ...]}
    """
    # Разбиваем на чанки для больших текстов
    chunk_size = 50000
    chunks = []
    start = 0
    while start < len(text):
        end = min(start + chunk_size, len(text))
        if end < len(text):
            # Ищем границу предложения
            for i in range(end, max(end - 2000, start), -1):
                if i < len(text) and text[i - 1] in ".!?":
                    end = i
                    break
        chunks.append((start, text[start:end]))
        start = end
    
    # Для каждого персонажа — список позиций
    char_positions = defaultdict(list)
    
    # Build form → lemma mapping
    form_to_lemma = {}
    for char in characters:
        for form in char.get("forms", [char["lemma"]]):
            form_to_lemma[form] = char["lemma"]
        form_to_lemma[char["lemma"]] = char["lemma"]
    
    # Обрабатываем каждый чанк
    for chunk_offset, chunk in chunks:
        doc = NLP(chunk)
        for ent in doc.ents:
            if ent.label_ == "PER" and ent.text in form_to_lemma:
                lemma = form_to_lemma[ent.text]
                char_positions[lemma].append(chunk_offset + ent.start_char)
        # Fallback: PROPN токены
        ent_token_ranges = set()
        for ent in doc.ents:
            for i in range(ent.start, ent.end):
                ent_token_ranges.add(i)
        for token in doc:
            if token.i in ent_token_ranges:
                continue
            if token.pos_ == "PROPN" and token.text in form_to_lemma:
                lemma = form_to_lemma[token.text]
                char_positions[lemma].append(chunk_offset + token.idx)
    
    return dict(char_positions)


def build_character_cooccurrence(char_positions: dict, window: int = 2000) -> tuple:
    """Построить граф co-occurrence персонажей.
    
    A[i,j] = сколько раз персонажи i,j упомянуты в пределах window символов.
    """
    characters = sorted(char_positions.keys())
    n = len(characters)
    char_idx = {c: i for i, c in enumerate(characters)}
    
    if n == 0:
        return characters, np.zeros((0, 0))
    
    # Собираем все упоминания: (position, character_index)
    all_mentions = []
    for char, positions in char_positions.items():
        for pos in positions:
            all_mentions.append((pos, char_idx[char]))
    all_mentions.sort()
    
    # Скользящее окно: для каждого упоминания считаем сколько других персонажей
    # упомянуто в пределах window символов
    A = np.zeros((n, n))
    for i, (pos_i, char_i) in enumerate(all_mentions):
        # Идём вперёд пока в окне
        for j in range(i + 1, len(all_mentions)):
            pos_j, char_j = all_mentions[j]
            if pos_j - pos_i > window:
                break
            if char_i != char_j:
                # Вес обратно пропорционален расстоянию
                dist = abs(pos_j - pos_i)
                weight = 1.0 / (1.0 + dist / 500.0)  # 500 — масштаб
                A[char_i, char_j] += weight
                A[char_j, char_i] += weight
    
    return characters, A


def build_character_graph(text: str) -> dict:
    """Полный пайплайн: текст → NER + SVO → граф персонажей → POLER."""
    
    # 1. NER — извлекаем персонажей
    ner_result = extract_entities(text)
    ner_persons = [e["lemma"] for e in ner_result["entities"] if e["label"] == "PER"]
    
    # 1b. SVO — извлекаем направленные действия (с fallback на всех PROPN)
    svo_result = extract_svo(text, use_ner=True)
    
    # 1c. Объединяем персонажей из NER и из SVO
    # Это решает проблему: NER на коротком тексте может пропустить имена
    svo_persons = set()
    for t in svo_result.get("triplets", []):
        svo_persons.add(t.get("subjectLemma", t.get("subject", "")))
        svo_persons.add(t.get("objectLemma", t.get("object", "")))
    
    all_persons = set(ner_persons) | svo_persons
    all_persons.discard("")
    
    if len(all_persons) < 2:
        return {
            "entities": ner_result,
            "graph": {"nodes": [], "edges": [], "nNodes": 0, "nEdges": 0},
            "poler": {"eigenvalues": [], "clusters": [], "silhouette": 0},
            "svo": {"triplets": [], "stats": {"total": 0}},
            "error": "Недостаточно персонажей для анализа (нужно ≥2)"
        }
    
    # Преобразуем в формат как раньше
    persons = [{"lemma": p, "forms": [p], "count": 0, "mentions": []} for p in sorted(all_persons)]
    
    # 2. Извлекаем позиции упоминаний каждого персонажа
    char_positions = extract_character_mentions(text, persons)
    
    # 3. Строим граф co-occurrence
    characters, A = build_character_cooccurrence(char_positions, window=2000)
    n = len(characters)
    
    if n < 2:
        return {
            "entities": ner_result,
            "graph": {"nodes": characters, "edges": [], "nNodes": n, "nEdges": 0},
            "poler": {"eigenvalues": [], "clusters": [], "silhouette": 0},
        }
    
    # 4. POLER-оператор: H = Π_Λ (L + γJ - B/m) Π_Λ
    k = A.sum(axis=1)
    m = A.sum() / 2
    if m == 0:
        return {
            "entities": ner_result,
            "graph": {"nodes": characters, "edges": [], "nNodes": n, "nEdges": 0},
            "poler": {"eigenvalues": [], "clusters": [], "silhouette": 0},
            "svo": {"triplets": [], "stats": {"total": 0}},
        }
    
    # Нормированный лапласиан
    k_safe = np.where(k > 0, k, 1)
    D_invsqrt = np.diag(1.0 / np.sqrt(k_safe))
    L = np.eye(n) - D_invsqrt @ A @ D_invsqrt
    L = (L + L.T) / 2
    
    # Матрица модулярности
    B = A - np.outer(k, k) / (2 * m)
    
    # Проектор Π_Λ = I - (1/n)·1·1^T
    Pi = np.eye(n) - np.ones((n, n)) / n
    
    # === SVO: направленная матрица A_dir для J оператора ===
    # J = (A_dir - A_dir^T) / 2 — антисимметричная часть
    # A_dir[i,j] = сколько раз персонаж i действовал на j
    # svo_result уже извлечён выше
    A_dir = np.zeros((n, n))
    char_idx = {c: i for i, c in enumerate(characters)}
    
    # Считаем направленные действия (с весом по полярности)
    for t in svo_result.get("triplets", []):
        s_lemma = t.get("subjectLemma", t.get("subject", ""))
        o_lemma = t.get("objectLemma", t.get("object", ""))
        # Ищем совпадение по lemma или по формам
        s_idx = char_idx.get(s_lemma)
        o_idx = char_idx.get(o_lemma)
        if s_idx is None:
            # Попробуем найти частичное совпадение
            for c, i in char_idx.items():
                if c.startswith(s_lemma[:4]) or s_lemma.startswith(c[:4]):
                    s_idx = i
                    break
        if o_idx is None:
            for c, i in char_idx.items():
                if c.startswith(o_lemma[:4]) or o_lemma.startswith(c[:4]):
                    o_idx = i
                    break
        if s_idx is not None and o_idx is not None and s_idx != o_idx:
            # Вес: негативные действия весом 2, позитивные 1.5, нейтральные 1
            pol = t.get("polarity", "neutral")
            weight = {"negative": 2.0, "positive": 1.5, "neutral": 1.0}.get(pol, 1.0)
            A_dir[s_idx, o_idx] += weight
    
    # J = (A_dir - A_dir^T) / 2 — антисимметричная
    J = (A_dir - A_dir.T) / 2.0
    
    # POLER-оператор
    gamma = 0.05  # вес резонанса (J)
    H = Pi @ (L + gamma * J - B / m) @ Pi
    # Симметризуем для вещественных собственных значений
    # (J антисимметричная, но H должна быть симметричной для eigsh)
    H = (H + H.T) / 2
    
    # 5. Собственные значения и векторы
    k_modes = min(4, n - 1)
    if n <= 3:
        eigenvalues, eigenvectors = np.linalg.eigh(H)
        eigenvalues = eigenvalues[1:1 + k_modes]
        eigenvectors = eigenvectors[:, 1:1 + k_modes]
    else:
        eigenvalues, eigenvectors = eigsh(H, k=k_modes, which="SM")
    
    # Нормируем
    for i in range(k_modes):
        norm = np.linalg.norm(eigenvectors[:, i])
        if norm > 1e-12:
            eigenvectors[:, i] /= norm
    
    # 6. K-means кластеризация
    X = eigenvectors  # (n, k_modes)
    n_clusters = min(k_modes, n)
    if n_clusters < 2:
        clusters_labels = np.zeros(n, dtype=int)
        sil = 0
    else:
        kmeans = KMeans(n_clusters=n_clusters, random_state=42, n_init=10)
        clusters_labels = kmeans.fit_predict(X)
        if n > n_clusters:
            sil = silhouette_score(X, clusters_labels)
        else:
            sil = 0
    
    # 7. Формируем результат
    clusters = []
    for c in range(n_clusters):
        mask = clusters_labels == c
        chars_in_cluster = [characters[i] for i in range(n) if mask[i]]
        # Средняя степень узлов в кластере
        avg_deg = k[mask].mean() if mask.sum() > 0 else 0
        clusters.append({
            "cluster": c,
            "characters": chars_in_cluster,
            "size": int(mask.sum()),
            "avgDegree": float(avg_deg),
        })
    # Сортируем по размеру
    clusters.sort(key=lambda x: -x["size"])
    
    # 8. Рёбра co-occurrence (симметричные) для визуализации
    edges = []
    for i in range(n):
        for j in range(i + 1, n):
            if A[i, j] > 0.5:  # порог
                edges.append({
                    "source": characters[i],
                    "target": characters[j],
                    "weight": float(A[i, j]),
                    "type": "cooccurrence",
                })
    edges.sort(key=lambda x: -x["weight"])
    
    # 9. Направленные рёбра SVO (для визуализации агрессоров/жертв)
    directed_edges = []
    for i in range(n):
        for j in range(n):
            if i != j and A_dir[i, j] > 0:
                directed_edges.append({
                    "source": characters[i],
                    "target": characters[j],
                    "weight": float(A_dir[i, j]),
                    "type": "action",
                })
    directed_edges.sort(key=lambda x: -x["weight"])
    
    # 10. Асимметрия J — кто «агрессор», кто «жертва»
    # Положительная сумма строки A_dir = персонаж больше действует
    # Положительная сумма столбца = персонаж больше подвергается действиям
    out_sum = A_dir.sum(axis=1)  # исходящие
    in_sum = A_dir.sum(axis=0)   # входящие
    asymmetry = []
    for i, c in enumerate(characters):
        asymmetry.append({
            "character": c,
            "outgoing": float(out_sum[i]),   # сколько действует на других
            "incoming": float(in_sum[i]),    # сколько подвергается
            "balance": float(out_sum[i] - in_sum[i]),  # +агрессор, -жертва
        })
    asymmetry.sort(key=lambda x: -abs(x["balance"]))
    
    return {
        "entities": ner_result,
        "graph": {
            "nodes": characters,
            "edges": edges[:100],  # топ-100 co-occurrence рёбер
            "directedEdges": directed_edges[:50],  # топ-50 SVO рёбер
            "nNodes": n,
            "nEdges": len(edges),
            "nDirectedEdges": len(directed_edges),
        },
        "poler": {
            "eigenvalues": eigenvalues.tolist(),
            "clusters": clusters,
            "silhouette": float(sil),
            "gamma": gamma,
            "kModes": k_modes,
            "jNorm": float(np.linalg.norm(J)),  # норма антисимметричной части
        },
        "svo": {
            "triplets": svo_result.get("triplets", [])[:100],  # топ-100 триплетов
            "stats": svo_result.get("stats", {}),
            "asymmetry": asymmetry,  # агрессоры vs жертвы
        },
    }


def main():
    try:
        # V2: читаем текст из файла (argv[1]) — надёжнее для больших текстов
        if len(sys.argv) > 1:
            with open(sys.argv[1], "r", encoding="utf-8") as f:
                text = f.read()
        else:
            text = sys.stdin.read()
        result = build_character_graph(text)
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
