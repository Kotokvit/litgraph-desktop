/**
 * Кластеризация через собственные векторы POLER-оператора.
 *
 * 1. Находим k наименьших собственных векторов H
 * 2. Кластеризуем k-means в этом пространстве
 */

/**
 * Собственные векторы с наименьшими собственными значениями симметричной матрицы.
 * Использует Jacobi eigenvalue algorithm (для маленьких матриц до ~200×200).
 */
export function smallestEigenvectors(
  H: number[][],
  k: number
): { values: number[]; vectors: number[][] } {
  const n = H.length;
  // Копируем чтобы не мутировать
  const A = H.map((row) => [...row]);

  // Сначала убеждаемся что симметричная
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      const avg = (A[i][j] + A[j][i]) / 2;
      A[i][j] = avg;
      A[j][i] = avg;
    }
  }

  // V — накопленная матрица вращений
  const V: number[][] = Array.from({ length: n }, (_, i) =>
    Array.from({ length: n }, (_, j) => (i === j ? 1 : 0))
  );

  const maxSweeps = 100;
  const tol = 1e-10;

  for (let sweep = 0; sweep < maxSweeps; sweep++) {
    // Вычисляем off-diagonal норму
    let off = 0;
    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        off += A[i][j] * A[i][j];
      }
    }
    if (off < tol) break;

    // Jacobi sweeps
    for (let p = 0; p < n - 1; p++) {
      for (let q = p + 1; q < n; q++) {
        if (Math.abs(A[p][q]) < 1e-15) continue;
        const app = A[p][p];
        const aqq = A[q][q];
        const apq = A[p][q];
        const phi = 0.5 * Math.atan2(2 * apq, aqq - app);
        const c = Math.cos(phi);
        const s = Math.sin(phi);

        // Вращение A
        for (let i = 0; i < n; i++) {
          const aip = A[i][p];
          const aiq = A[i][q];
          A[i][p] = c * aip - s * aiq;
          A[i][q] = s * aip + c * aiq;
        }
        for (let j = 0; j < n; j++) {
          const apj = A[p][j];
          const aqj = A[q][j];
          A[p][j] = c * apj - s * aqj;
          A[q][j] = s * apj + c * aqj;
        }
        A[p][q] = 0;
        A[q][p] = 0;

        // Вращение V
        for (let i = 0; i < n; i++) {
          const vip = V[i][p];
          const viq = V[i][q];
          V[i][p] = c * vip - s * viq;
          V[i][q] = s * vip + c * viq;
        }
      }
    }
  }

  // Собственные значения = диагональ A
  const eigvals: { val: number; idx: number }[] = [];
  for (let i = 0; i < n; i++) {
    eigvals.push({ val: A[i][i], idx: i });
  }
  // Сортируем по возрастанию
  eigvals.sort((a, b) => a.val - b.val);

  // Берём k наименьших (пропускаем первое тривиальное ≈0)
  const kActual = Math.min(k, n - 1);
  const values: number[] = [];
  const vectors: number[][] = []; // каждый вектор — столбец
  for (let mode = 1; mode <= kActual; mode++) {
    const idx = eigvals[mode].idx;
    values.push(eigvals[mode].val);
    const vec = new Array(n);
    for (let i = 0; i < n; i++) vec[i] = V[i][idx];
    // Нормируем
    const nrm = Math.sqrt(vec.reduce((s, x) => s + x * x, 0));
    for (let i = 0; i < n; i++) vec[i] /= nrm;
    vectors.push(vec);
  }

  return { values, vectors };
}

/** Простой k-means. */
export function kMeans(
  X: number[][], // [n_samples][n_features] — здесь X = modes.T
  k: number,
  maxIter = 100,
  seed = 42
): { labels: number[]; centroids: number[][] } {
  const n = X.length;
  const dim = X[0].length;
  const rng = mulberry32(seed);

  // Инициализация: k случайных точек из X
  const centroids: number[][] = [];
  const usedIndices = new Set<number>();
  while (centroids.length < k && centroids.length < n) {
    const idx = Math.floor(rng() * n);
    if (!usedIndices.has(idx)) {
      usedIndices.add(idx);
      centroids.push([...X[idx]]);
    }
  }

  const labels = new Array(n).fill(0);

  for (let iter = 0; iter < maxIter; iter++) {
    let changed = false;

    // Assign
    for (let i = 0; i < n; i++) {
      let bestK = 0;
      let bestDist = Infinity;
      for (let c = 0; c < k; c++) {
        let dist = 0;
        for (let d = 0; d < dim; d++) {
          const diff = X[i][d] - centroids[c][d];
          dist += diff * diff;
        }
        if (dist < bestDist) {
          bestDist = dist;
          bestK = c;
        }
      }
      if (labels[i] !== bestK) {
        labels[i] = bestK;
        changed = true;
      }
    }

    // Update
    const sums: number[][] = Array.from({ length: k }, () =>
      new Array(dim).fill(0)
    );
    const counts = new Array(k).fill(0);
    for (let i = 0; i < n; i++) {
      const c = labels[i];
      for (let d = 0; d < dim; d++) sums[c][d] += X[i][d];
      counts[c]++;
    }
    for (let c = 0; c < k; c++) {
      if (counts[c] > 0) {
        for (let d = 0; d < dim; d++) centroids[c][d] = sums[c][d] / counts[c];
      }
    }

    if (!changed) break;
  }

  return { labels, centroids };
}

/** Силуэт (упрощённый) — мера качества кластеризации. */
export function silhouette(
  X: number[][],
  labels: number[]
): number {
  const n = X.length;
  const uniqueLabels = Array.from(new Set(labels));
  const k = uniqueLabels.length;
  if (k < 2 || k >= n) return 0;

  let totalSil = 0;
  for (let i = 0; i < n; i++) {
    const li = labels[i];
    // a(i) — среднее расстояние до точек того же кластера
    let a = 0;
    let countSame = 0;
    for (let j = 0; j < n; j++) {
      if (j === i) continue;
      if (labels[j] === li) {
        a += dist(X[i], X[j]);
        countSame++;
      }
    }
    a = countSame > 0 ? a / countSame : 0;

    // b(i) — минимальное среднее расстояние до точек другого кластера
    let b = Infinity;
    for (const otherLabel of uniqueLabels) {
      if (otherLabel === li) continue;
      let sum = 0;
      let count = 0;
      for (let j = 0; j < n; j++) {
        if (labels[j] === otherLabel) {
          sum += dist(X[i], X[j]);
          count++;
        }
      }
      if (count > 0) {
        const avg = sum / count;
        if (avg < b) b = avg;
      }
    }

    const sil = b === Infinity ? 0 : (b - a) / Math.max(a, b);
    totalSil += sil;
  }
  return totalSil / n;
}

function dist(a: number[], b: number[]): number {
  let s = 0;
  for (let i = 0; i < a.length; i++) {
    const d = a[i] - b[i];
    s += d * d;
  }
  return Math.sqrt(s);
}

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
