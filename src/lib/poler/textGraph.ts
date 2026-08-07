/**
 * Построение графа co-occurrence из текста.
 *
 * Порт lib/text_graph.py из poler-prototype.
 *
 * Матрицы:
 * - A (symmetric, weighted): co-occurrence в окне W
 * - A_dir (directed): соседние токены (i → j если w_t=i, w_{t+1}=j)
 * - L: нормированный лапласиан I - D^{-1/2} A D^{-1/2}
 * - B: матрица модулярности A - kk^T/(2m)
 * - Pi: проектор I - (1/n)·1·1^T
 */

/** Простая токенизация: lowercase + split по не-буквам. НЕ удаляет стоп-слов. */
export function tokenize(text: string): string[] {
  const lower = text.toLowerCase();
  // Кирилица + латиница + цифры
  const tokens = lower.split(/[^a-zа-яё0-9]+/i).filter((t) => t.length >= 2);
  return tokens;
}

/** Словарь: слово → индекс. Удаляет слова с частотой < minFreq. */
export function buildVocabulary(
  tokens: string[],
  minFreq = 2
): { word2idx: Map<string, number>; vocab: string[] } {
  const freq = new Map<string, number>();
  for (const t of tokens) {
    freq.set(t, (freq.get(t) ?? 0) + 1);
  }
  const vocab = Array.from(freq.entries())
    .filter(([, c]) => c >= minFreq)
    .map(([w]) => w)
    .sort();
  const word2idx = new Map<string, number>();
  vocab.forEach((w, i) => word2idx.set(w, i));
  return { word2idx, vocab };
}

/** Симметричная взвешенная матрица co-occurrence.
 * A[i,j] += 1/distance если j в окне ±windowSize от i.
 */
export function buildCooccurrence(
  tokens: string[],
  word2idx: Map<string, number>,
  windowSize = 5
): { values: number[]; indices: number[]; indptr: number[]; n: number } {
  const n = word2idx.size;
  // Используем Map для разреженного накопления
  const entries = new Map<string, number>();

  for (let center = 0; center < tokens.length; center++) {
    const centerWord = tokens[center];
    const centerIdx = word2idx.get(centerWord);
    if (centerIdx === undefined) continue;

    for (let offset = 1; offset <= windowSize; offset++) {
      const weight = 1.0 / offset;
      for (const nbrPos of [center - offset, center + offset]) {
        if (nbrPos < 0 || nbrPos >= tokens.length) continue;
        const nbrWord = tokens[nbrPos];
        const nbrIdx = word2idx.get(nbrWord);
        if (nbrIdx === undefined || nbrIdx === centerIdx) continue;
        // Симметричный ключ
        const key = `${Math.min(centerIdx, nbrIdx)},${Math.max(centerIdx, nbrIdx)}`;
        entries.set(key, (entries.get(key) ?? 0) + weight);
      }
    }
  }

  // Собираем CSR
  const rows: number[] = [];
  const cols: number[] = [];
  const data: number[] = [];
  for (const [key, value] of entries) {
    const [i, j] = key.split(",").map(Number);
    rows.push(i);
    cols.push(j);
    data.push(value);
    // Симметрия
    if (i !== j) {
      rows.push(j);
      cols.push(i);
      data.push(value);
    }
  }

  return toCSR(rows, cols, data, n);
}

/** Направленная матрица: A_dir[i,j] = сколько раз j шёл сразу после i. */
export function buildDirectedAdjacency(
  tokens: string[],
  word2idx: Map<string, number>
): { values: number[]; indices: number[]; indptr: number[]; n: number } {
  const n = word2idx.size;
  const entries = new Map<string, number>();

  for (let t = 0; t < tokens.length - 1; t++) {
    const w1 = tokens[t];
    const w2 = tokens[t + 1];
    const i = word2idx.get(w1);
    const j = word2idx.get(w2);
    if (i === undefined || j === undefined || i === j) continue;
    const key = `${i},${j}`;
    entries.set(key, (entries.get(key) ?? 0) + 1);
  }

  const rows: number[] = [];
  const cols: number[] = [];
  const data: number[] = [];
  for (const [key, value] of entries) {
    const [i, j] = key.split(",").map(Number);
    rows.push(i);
    cols.push(j);
    data.push(value);
  }

  return toCSR(rows, cols, data, n);
}

/** Нормированный лапласиан: L = I - D^{-1/2} A D^{-1/2}. */
export function buildLaplacian(
  A: SparseMatrix
): number[][] {
  const n = A.n;
  const dense = toDense(A);
  const k = new Array(n).fill(0);
  for (let i = 0; i < n; i++) k[i] = dense[i].reduce((s, v) => s + v, 0);

  const L: number[][] = Array.from({ length: n }, () => new Array(n).fill(0));
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      const ki = Math.sqrt(k[i] || 1);
      const kj = Math.sqrt(k[j] || 1);
      L[i][j] = (i === j ? 1 : 0) - dense[i][j] / (ki * kj);
    }
  }
  // Симметризация
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      const avg = (L[i][j] + L[j][i]) / 2;
      L[i][j] = avg;
      L[j][i] = avg;
    }
  }
  return L;
}

/** B = A - k·k^T / (2m) — матрица модулярности Ньюмана. */
export function buildModularityMatrix(A: SparseMatrix): number[][] {
  const n = A.n;
  const dense = toDense(A);
  const k = new Array(n).fill(0);
  for (let i = 0; i < n; i++) k[i] = dense[i].reduce((s, v) => s + v, 0);
  const m = k.reduce((s, v) => s + v, 0) / 2;
  if (m === 0) return dense;

  const B: number[][] = Array.from({ length: n }, () => new Array(n).fill(0));
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      B[i][j] = dense[i][j] - (k[i] * k[j]) / (2 * m);
    }
  }
  return B;
}

/** Π_Λ = I - (1/n)·1·1^T. Проектор на подпространство ⊥ 1. */
export function buildProjector(n: number): number[][] {
  const Pi: number[][] = Array.from({ length: n }, () => new Array(n).fill(0));
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      Pi[i][j] = (i === j ? 1 : 0) - 1 / n;
    }
  }
  return Pi;
}

// ========================
// Разреженные матрицы (CSR)
// ========================

export interface SparseMatrix {
  values: number[];
  indices: number[]; // column indices
  indptr: number[]; // row pointers
  n: number;
}

function toCSR(
  rows: number[],
  cols: number[],
  data: number[],
  n: number
): SparseMatrix {
  // Сортируем по row, затем col
  const triplets = rows.map((r, i) => [r, cols[i], data[i]] as const);
  triplets.sort((a, b) => a[0] - b[0] || a[1] - b[1]);

  const indptr = new Array(n + 1).fill(0);
  const indices: number[] = [];
  const values: number[] = [];

  // Подсчёт элементов в каждой строке
  for (const [r] of triplets) indptr[r + 1]++;
  // Prefix sum
  for (let i = 1; i <= n; i++) indptr[i] += indptr[i - 1];

  // Заполнение
  for (const triplet of triplets) {
    indices.push(triplet[1]);
    values.push(triplet[2]);
  }

  return { values, indices, indptr, n };
}

function toDense(A: SparseMatrix): number[][] {
  const n = A.n;
  const dense: number[][] = Array.from({ length: n }, () => new Array(n).fill(0));
  for (let r = 0; r < n; r++) {
    for (let idx = A.indptr[r]; idx < A.indptr[r + 1]; idx++) {
      const c = A.indices[idx];
      dense[r][c] = A.values[idx];
    }
  }
  return dense;
}

/** Сумма всех элементов (для расчёта m). */
export function sparseSum(A: SparseMatrix): number {
  return A.values.reduce((s, v) => s + v, 0);
}

/** Степени узлов. */
export function sparseDegrees(A: SparseMatrix): number[] {
  const n = A.n;
  const k = new Array(n).fill(0);
  for (let r = 0; r < n; r++) {
    for (let idx = A.indptr[r]; idx < A.indptr[r + 1]; idx++) {
      k[r] += A.values[idx];
    }
  }
  return k;
}
