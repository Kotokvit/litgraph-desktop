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

# Импортируем NER
sys.path.insert(0, ".")
from ner_extract import extract_entities, NLP


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
    """Полный пайплайн: текст → NER → граф персонажей → POLER."""
    
    # 1. NER — извлекаем персонажей
    ner_result = extract_entities(text)
    persons = [e for e in ner_result["entities"] if e["label"] == "PER"]
    
    if len(persons) < 2:
        return {
            "entities": ner_result,
            "graph": {"nodes": [], "edges": [], "nNodes": 0, "nEdges": 0},
            "poler": {"eigenvalues": [], "clusters": [], "silhouette": 0},
            "error": "Недостаточно персонажей для анализа (нужно ≥2)"
        }
    
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
    
    # J = 0 (пока нет направленных связей — это для SVO в Фазе 2)
    gamma = 0.05
    J = np.zeros((n, n))
    
    # POLER-оператор
    H = Pi @ (L + gamma * J - B / m) @ Pi
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
    
    # 8. Рёбра для визуализации
    edges = []
    for i in range(n):
        for j in range(i + 1, n):
            if A[i, j] > 0.5:  # порог
                edges.append({
                    "source": characters[i],
                    "target": characters[j],
                    "weight": float(A[i, j]),
                })
    edges.sort(key=lambda x: -x["weight"])
    
    return {
        "entities": ner_result,
        "graph": {
            "nodes": characters,
            "edges": edges[:100],  # топ-100 рёбер
            "nNodes": n,
            "nEdges": len(edges),
        },
        "poler": {
            "eigenvalues": eigenvalues.tolist(),
            "clusters": clusters,
            "silhouette": float(sil),
            "gamma": gamma,
            "kModes": k_modes,
        },
    }


def main():
    try:
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
