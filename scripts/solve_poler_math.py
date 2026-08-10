import sympy as sp
import numpy as np
import scipy.linalg as la
from scipy.optimize import minimize

print("=======================================================")
print("   PART 1: SYMBOLIC DERIVATIONS & CALCULUS (SymPy)")
print("=======================================================")

# Определяем символьные переменные
u, delta, kappa, I_kw, d, E, C_canon, A_svo = sp.symbols('u delta kappa I_kw d E C_canon A_svo', positive=True, real=True)
d_sq_avg, I_loc, gamma_emo, lambda_conf, Omega_conf = sp.symbols('d_sq_avg I_loc gamma_emo lambda_conf Omega_conf', positive=True, real=True)

# 1. Каноническая формула
eps_canon = (kappa * I_kw * d + E + C_canon + A_svo) / sp.sqrt(u + delta)

# Частные производные
deps_du = sp.diff(eps_canon, u)
deps_ddelta = sp.diff(eps_canon, delta)
deps_dkappa = sp.diff(eps_canon, kappa)

print("1.1 Каноническая формула eps:")
sp.pprint(eps_canon)

print("\n1.2 Частная производная по длине |U| (d_eps / du):")
sp.pprint(deps_du)

print("\n1.3 Частная производная по параметру сглаживания delta_bias (d_eps / ddelta):")
sp.pprint(deps_ddelta)

# 2. Кульминационная формула
eps_climax = (kappa * I_loc * d_sq_avg + gamma_emo * E + lambda_conf * Omega_conf) / sp.log(sp.E + u)
deps_climax_du = sp.diff(eps_climax, u)
deps_climax_dOmega = sp.diff(eps_climax, Omega_conf)

print("\n2.1 Кульминационная формула eps_climax:")
sp.pprint(eps_climax)

print("\n2.2 Частная производная climax по длине (d_eps_climax / du):")
sp.pprint(deps_climax_du)

print("\n2.3 Частная производная climax по оператору конфликта (d_eps_climax / dOmega):")
sp.pprint(deps_climax_dOmega)

# 3. Асимптотический предел при u -> infinity
limit_u_inf = sp.limit(eps_canon, u, sp.oo)
limit_climax_u_inf = sp.limit(eps_climax, u, sp.oo)

print("\n3.1 Предел канонической eps при |U| -> inf:", limit_u_inf)
print("3.2 Предел climax eps при |U| -> inf:", limit_climax_u_inf)


print("\n=======================================================")
print("   PART 2: SPECTRAL MATRIX ALGEBRA & J-MATRIX (NumPy/SciPy)")
print("=======================================================")

# Персонажи «Сферы Предела»: [Паша, Марта, Веня, Красс, Рэй, Аэрон, Фокс, Сёма]
chars_sfera = ["Паша", "Марта", "Веня", "Красс", "Рэй", "Аэрон", "Фокс", "Сёма"]
n_sfera = len(chars_sfera)

# Ненаправленная матрица совместной встречаемости (Adjacency Matrix A)
A_sfera = np.array([
    [0, 5, 8, 2, 1, 0, 0, 1], # Паша
    [5, 0, 12, 4, 15, 2, 3, 2], # Марта
    [8, 12, 0, 18, 6, 1, 4, 10], # Веня
    [2, 4, 18, 0, 9, 7, 2, 1], # Красс
    [1, 15, 6, 9, 0, 14, 8, 3], # Рэй
    [0, 2, 1, 7, 14, 0, 5, 0], # Аэрон
    [0, 3, 4, 2, 8, 5, 0, 2], # Фокс
    [1, 2, 10, 1, 3, 0, 2, 0]  # Сёма
], dtype=float)

# Антисимметричная матрица направленного действия J (J = -J^T)
J_sfera = np.array([
    [ 0,  2,  3,  0,  0,  0,  0,  1],
    [-2,  0,  4, -1, -8,  0,  1,  0],
    [-3, -4,  0,  9, -2,  0,  2,  5],
    [ 0,  1, -9,  0, -5,  4,  0,  0],
    [ 0,  8,  2,  5,  0, 11,  6,  1],
    [ 0,  0,  0, -4,-11,  0,  3,  0],
    [ 0, -1, -2,  0, -6, -3,  0,  0],
    [-1,  0, -5,  0, -1,  0,  0,  0]
], dtype=float)

# 1. Спектральный анализ матрицы A
eigenvalues_A, eigenvectors_A = la.eig(A_sfera)
eigenvalues_A = np.real(eigenvalues_A)
idx_sort = np.argsort(eigenvalues_A)[::-1]
eigenvalues_A = eigenvalues_A[idx_sort]
eigenvectors_A = np.real(eigenvectors_A[:, idx_sort])

# Первичный собственный вектор (Centrality Vector v1)
centrality_v1 = np.abs(eigenvectors_A[:, 0])
centrality_v1 = centrality_v1 / np.sum(centrality_v1)

spectral_radius = np.max(np.abs(eigenvalues_A))

print(f"Спектральный радиус матрицы A: rho(A) = {spectral_radius:.4f}")
print("\nСобственная центральность персонажей (Eigenvector Centrality v1):")
for name, c_val in zip(chars_sfera, centrality_v1):
    print(f"  {name:8s}: {c_val:.4f}")

# 2. Singular Value Decomposition (SVD) матрицы J
U_mat, S_vals, Vt_mat = la.svd(J_sfera)

print("\nСингулярные числа антисимметричной матрицы J (Singular Values S):")
for idx, s in enumerate(S_vals):
    print(f"  sigma_{idx+1} = {s:.4f}")

# 3. Расчёт оператора спрямованного конфлікту Omega_conf(C) = sum_{P != C} |J(C,P)| * A(C,P)
Omega_conf_values = []
for i in range(n_sfera):
    omega_i = np.sum(np.abs(J_sfera[i, :]) * A_sfera[i, :])
    Omega_conf_values.append(omega_i)

print("\nОператор спрямованого конфлікту Omega_conf(C) для кожного персонажа:")
for name, om_val in zip(chars_sfera, Omega_conf_values):
    print(f"  {name:8s}: Omega_conf = {om_val:8.2f}")


print("\n=======================================================")
print("   PART 3: NUMERICAL OPTIMIZATION OF PARAMETERS (SciPy)")
print("=======================================================")

# Оптимизация delta_bias и theta_base для максимального разделения шума и сигнала
# Имитация экспериментальной функции распределения (Noise vs Signal)
np.random.seed(42)
noise_u_lens = np.random.randint(3, 12, 1000)
signal_u_lens = np.random.randint(15, 50, 500)

noise_d_sums = noise_u_lens * np.random.uniform(0.8, 1.5, 1000)
signal_d_sums = signal_u_lens * np.random.uniform(1.8, 3.5, 500)

def loss_function(params):
    delta_val, theta_val = params
    if delta_val <= 0 or theta_val <= 0:
        return 1e6
    
    eps_noise = noise_d_sums / np.sqrt(noise_u_lens + delta_val)
    eps_signal = signal_d_sums / np.sqrt(signal_u_lens + delta_val)
    
    # Хотим минимизировать перекрытие между шумом и сигналом (False Positive + False Negative)
    fp = np.sum(eps_noise >= theta_val) / len(eps_noise)
    fn = np.sum(eps_signal < theta_val) / len(eps_signal)
    
    # Целевая функция: взвешенная сумма ошибок
    return fp + 2.0 * fn

res = minimize(loss_function, x0=[15.0, 3.50], method='Nelder-Mead')

print(f"Результаты Nelder-Mead оптимизации параметров:")
print(f"  Оптимальный delta_bias*: {res.x[0]:.4f}")
print(f"  Оптимальный theta_base*: {res.x[1]:.4f}")
print(f"  Минимальная суммарная ошибка (Loss): {res.fun:.6f}")
print("=======================================================\n")
