/**
 * Ядро POLER-динамики для графа.
 *
 * Каноническое уравнение:
 *     dp/dt = -η · Π_Λ · [D·p + γ·J·p + ∇F]
 *
 * Где для графа текста:
 *     D = L (нормированный лапласиан)
 *     J = (A_dir - A_dir^T) / 2  (антисимметричная часть)
 *     Π_Λ = I - (1/n)·1·1^T   (проектор на ⊥1)
 *     F = -p^T B p / (2m)    (отрицательная модулярность)
 *     ∇F = -B·p / m
 *
 * Стационарность: Π_Λ · (L + γJ - B/m) · p = 0
 */

export interface PolerParams {
  eta: number; // шаг динамики
  gamma: number; // вес резонансной части J
  rho: number; // затухание памяти
  maxIter: number;
  tol: number;
  backtracking: boolean;
}

export const DEFAULT_PARAMS: PolerParams = {
  eta: 0.1,
  gamma: 0.05,
  rho: 0.9,
  maxIter: 500,
  tol: 1e-7,
  backtracking: true,
};

export interface PolerState {
  p: number[];
  memory: number[][]; // FIFO буфер [memorySize][n]
  FHistory: number[];
  pHistory: number[];
  iter: number;
  converged: boolean;
}

/** Начальное состояние: p0 = 1/√n + малый шум. */
export function initState(
  n: number,
  memorySize = 16,
  noiseScale = 0.05,
  seed = 42
): PolerState {
  const rng = mulberry32(seed);
  const p0 = new Array(n);
  for (let i = 0; i < n; i++) {
    p0[i] = 1 / Math.sqrt(n) + noiseScale * (rng() - 0.5) * 2;
  }
  normalize(p0);
  const memory: number[][] = Array.from({ length: memorySize }, () =>
    new Array(n).fill(0)
  );
  return { p: p0, memory, FHistory: [], pHistory: [], iter: 0, converged: false };
}

/** F(p) = -p^T B p / (2m). */
export function energyF(p: number[], B: number[][], m: number): number {
  let pTBp = 0;
  for (let i = 0; i < p.length; i++) {
    for (let j = 0; j < p.length; j++) {
      pTBp += p[i] * B[i][j] * p[j];
    }
  }
  return -pTBp / (2 * m);
}

/** ∇F = -B·p / m. */
export function gradF(p: number[], B: number[][], m: number): number[] {
  const n = p.length;
  const grad = new Array(n).fill(0);
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      grad[i] += B[i][j] * p[j];
    }
    grad[i] = -grad[i] / m;
  }
  return grad;
}

/** Резонансный член: γ · Σ_k ρ^k · J · p_{t-k}. */
function resonanceTerm(
  p: number[],
  J: number[][],
  state: PolerState,
  params: PolerParams
): number[] {
  const n = p.length;
  const memorySize = state.memory.length;
  // Сдвигаем memory (FIFO): новые в начале
  for (let i = memorySize - 1; i > 0; i--) {
    state.memory[i] = state.memory[i - 1];
  }
  state.memory[0] = [...p];

  // Взвешенная сумма: Σ_k ρ^k · memory[k]
  const weightedP = new Array(n).fill(0);
  for (let k = 0; k < memorySize; k++) {
    const w = Math.pow(params.rho, k);
    for (let i = 0; i < n; i++) {
      weightedP[i] += w * state.memory[k][i];
    }
  }

  // J · weightedP
  const result = new Array(n).fill(0);
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      result[i] += J[i][j] * weightedP[j];
    }
    result[i] *= params.gamma;
  }
  return result;
}

/** Один шаг POLER-динамики. */
function polerStep(
  state: PolerState,
  L: number[][],
  J: number[][],
  B: number[][],
  Pi: number[][],
  m: number,
  params: PolerParams
): number[] {
  const n = state.p.length;
  const p = state.p;

  // L · p
  const Lp = matVec(L, p);
  // γ · J · p_mem
  const JpMem = resonanceTerm(p, J, state, params);
  // ∇F = -B·p / m
  const grad = gradF(p, B, m);

  // force = L·p + γ·J·p_mem + ∇F
  const force = new Array(n).fill(0);
  for (let i = 0; i < n; i++) {
    force[i] = Lp[i] + JpMem[i] + grad[i];
  }

  // dp = -η · Π_Λ · force
  const piForce = matVec(Pi, force);
  const dp = new Array(n).fill(0);
  for (let i = 0; i < n; i++) {
    dp[i] = -params.eta * piForce[i];
  }
  return dp;
}

/** Эволюция POLER до сходимости или maxIter. */
export function evolve(
  state: PolerState,
  L: number[][],
  J: number[][],
  B: number[][],
  Pi: number[][],
  m: number,
  params: PolerParams = DEFAULT_PARAMS
): PolerState {
  let etaCurrent = params.eta;
  let FPrev = energyF(state.p, B, m);
  state.FHistory.push(FPrev);

  for (let it = 0; it < params.maxIter; it++) {
    const dp = polerStep(state, L, J, B, Pi, m, params);
    const pNew = new Array(state.p.length);
    for (let i = 0; i < state.p.length; i++) {
      pNew[i] = state.p[i] + dp[i];
    }
    normalize(pNew);

    const FNew = energyF(pNew, B, m);

    if (params.backtracking && FNew > FPrev + 1e-10) {
      // Энергия выросла — откат, уменьшаем η
      etaCurrent *= 0.5;
      if (etaCurrent < 1e-6) {
        state.converged = true;
        break;
      }
      continue;
    } else {
      etaCurrent = params.eta;
      state.p = pNew;
      state.FHistory.push(FNew);
      state.pHistory.push(norm(dp));
      state.iter = it + 1;

      if (norm(dp) < params.tol) {
        state.converged = true;
        break;
      }
      FPrev = FNew;
    }
  }
  return state;
}

/**
 * Multi-mode: построение оператора H = Π_Λ (L + γJ - B/m) Π_Λ
 * и извлечение k наименьших собственных векторов.
 *
 * Использует Jacobi eigenvalue algorithm (для маленьких матриц).
 */
export function buildPolarOperator(
  L: number[][],
  J: number[][],
  B: number[][],
  Pi: number[][],
  m: number,
  gamma: number
): number[][] {
  const n = L.length;
  // H = Pi @ (L + γJ - B/m) @ Pi
  // Шаг 1: M = L + γJ - B/m
  const M: number[][] = Array.from({ length: n }, () => new Array(n).fill(0));
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      M[i][j] = L[i][j] + gamma * J[i][j] - B[i][j] / m;
    }
  }
  // Шаг 2: PiM = Pi @ M
  const PiM = matMul(Pi, M);
  // Шаг 3: H = PiM @ Pi
  const H = matMul(PiM, Pi);
  // Симметризация
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      const avg = (H[i][j] + H[j][i]) / 2;
      H[i][j] = avg;
      H[j][i] = avg;
    }
  }
  return H;
}

// ========================
// Вспомогательные функции
// ========================

function matVec(A: number[][], x: number[]): number[] {
  const n = A.length;
  const result = new Array(n).fill(0);
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      result[i] += A[i][j] * x[j];
    }
  }
  return result;
}

function matMul(A: number[][], B: number[][]): number[][] {
  const n = A.length;
  const C: number[][] = Array.from({ length: n }, () => new Array(n).fill(0));
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      let s = 0;
      for (let k = 0; k < n; k++) s += A[i][k] * B[k][j];
      C[i][j] = s;
    }
  }
  return C;
}

function normalize(v: number[]): void {
  const nrm = norm(v);
  if (nrm > 1e-12) {
    for (let i = 0; i < v.length; i++) v[i] /= nrm;
  }
}

function norm(v: number[]): number {
  return Math.sqrt(v.reduce((s, x) => s + x * x, 0));
}

/** Простой PRNG (mulberry32) — детерминированный по seed. */
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
