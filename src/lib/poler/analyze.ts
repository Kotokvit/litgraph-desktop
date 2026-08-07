/**
 * POLER Text Analyzer — высокоуровневый API.
 *
 * Принимает текст → возвращает кластеры слов с метриками.
 */

import {
  tokenize,
  buildVocabulary,
  buildCooccurrence,
  buildDirectedAdjacency,
  buildLaplacian,
  buildModularityMatrix,
  buildProjector,
  sparseSum,
  sparseDegrees,
  type SparseMatrix,
} from "./textGraph";
import {
  DEFAULT_PARAMS,
  initState,
  evolve,
  buildPolarOperator,
  type PolerParams,
} from "./polerCore";
import { smallestEigenvectors, kMeans, silhouette } from "./clustering";

export interface PolerAnalysisOptions {
  windowSize?: number;
  minFreq?: number;
  gamma?: number;
  kModes?: number;
  eta?: number;
  maxIter?: number;
  seed?: number;
}

export interface WordCluster {
  word: string;
  cluster: number;
  modeNorm: number; // ||POLER-мода|| — значимость
  degree: number; // степень в графе
  modes: number[]; // координаты в пространстве k мод
}

export interface PolerAnalysisResult {
  vocabulary: string[];
  clusters: WordCluster[];
  silhouette: number;
  eigenvalues: number[];
  nNodes: number;
  nEdges: number;
  gamma: number;
  kModes: number;
  iterations: number;
  converged: boolean;
  energyStart: number;
  energyFinal: number;
}

/**
 * Полный анализ текста через POLER-динамику.
 */
export function analyzeText(
  text: string,
  options: PolerAnalysisOptions = {}
): PolerAnalysisResult {
  const windowSize = options.windowSize ?? 5;
  const minFreq = options.minFreq ?? 2;
  const gamma = options.gamma ?? 0.05;
  const kModes = options.kModes ?? 4;
  const seed = options.seed ?? 42;

  // 1. Токенизация и словарь
  const tokens = tokenize(text);
  const { word2idx, vocab } = buildVocabulary(tokens, minFreq);
  const n = vocab.length;

  if (n < kModes + 1) {
    throw new Error(
      `Слишком мало уникальных слов (${n}) для кластеризации с k=${kModes}. ` +
        `Нужно минимум ${kModes + 1} слов с частотой ≥ ${minFreq}.`
    );
  }

  // 2. Матрицы
  const A = buildCooccurrence(tokens, word2idx, windowSize);
  const ADir = buildDirectedAdjacency(tokens, word2idx);
  const L = buildLaplacian(A);
  const B = buildModularityMatrix(A);
  const Pi = buildProjector(n);

  // J = (A_dir - A_dir^T) / 2 — антисимметричная
  const ADirDense = sparseToDense(ADir);
  const J: number[][] = Array.from({ length: n }, () => new Array(n).fill(0));
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      J[i][j] = (ADirDense[i][j] - ADirDense[j][i]) / 2;
    }
  }

  const m = sparseSum(A) / 2;
  const degrees = sparseDegrees(A);

  // 3. POLER-динамика (одна мода для диагностики)
  const params: PolerParams = {
    ...DEFAULT_PARAMS,
    eta: options.eta ?? DEFAULT_PARAMS.eta,
    maxIter: options.maxIter ?? DEFAULT_PARAMS.maxIter,
    gamma,
  };
  const state = initState(n, 16, 0.05, seed);
  const energyStart = state.FHistory[0] ?? 0;
  evolve(state, L, J, B, Pi, m, params);
  const energyFinal = state.FHistory[state.FHistory.length - 1] ?? 0;

  // 4. Multi-mode: оператор H и его собственные векторы
  const H = buildPolarOperator(L, J, B, Pi, m, gamma);
  const { values: eigvals, vectors: modes } = smallestEigenvectors(H, kModes);

  // 5. Кластеризация k-means в пространстве мод
  // modes[k] — вектор длины n. Для k-means нужно X[i] = [modes[0][i], modes[1][i], ...]
  const X: number[][] = Array.from({ length: n }, () => new Array(modes.length).fill(0));
  for (let i = 0; i < n; i++) {
    for (let k = 0; k < modes.length; k++) {
      X[i][k] = modes[k][i];
    }
  }
  const { labels } = kMeans(X, kModes, 100, seed);
  const sil = silhouette(X, labels);

  // 6. Сборка результата
  const clusters: WordCluster[] = vocab.map((word, i) => ({
    word,
    cluster: labels[i],
    modeNorm: Math.sqrt(X[i].reduce((s, x) => s + x * x, 0)),
    degree: degrees[i],
    modes: X[i],
  }));

  // Сортируем по убыванию modeNorm (смысловая значимость)
  clusters.sort((a, b) => b.modeNorm - a.modeNorm);

  return {
    vocabulary: vocab,
    clusters,
    silhouette: sil,
    eigenvalues: eigvals,
    nNodes: n,
    nEdges: Math.round(m),
    gamma,
    kModes,
    iterations: state.iter,
    converged: state.converged,
    energyStart,
    energyFinal,
  };
}

function sparseToDense(A: SparseMatrix): number[][] {
  const n = A.n;
  const dense: number[][] = Array.from({ length: n }, () =>
    new Array(n).fill(0)
  );
  for (let r = 0; r < n; r++) {
    for (let idx = A.indptr[r]; idx < A.indptr[r + 1]; idx++) {
      const c = A.indices[idx];
      dense[r][c] = A.values[idx];
    }
  }
  return dense;
}
