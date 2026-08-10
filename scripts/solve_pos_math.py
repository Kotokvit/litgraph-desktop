#!/usr/bin/env python3
"""
SymPy Calculus & Spectral Analysis for Layer B & C POS-Tagger Integration into POLER Epsilon
"""

import sympy as sp
import numpy as np

def run_pos_math():
    print("=== SYMPY POS DISAMBIGUATION & EPSILON CALCULUS ===")
    
    # 1. Variables
    u = sp.Symbol('u', positive=True, real=True) # |U|
    delta = sp.Symbol('delta_bias', positive=True, real=True) # delta_bias = 15.0
    kappa = sp.Symbol('kappa', positive=True, real=True) # kappa = 1.0 or 1.2
    i_kw = sp.Symbol('I_kw', positive=True, real=True)
    d = sp.Symbol('d', positive=True, real=True) # linear sum of rarity
    e_val = sp.Symbol('E', real=True) # emotion
    c_canon = sp.Symbol('C_canon', real=True) # canon anchors
    a_svo = sp.Symbol('A_svo', real=True) # action verbs
    
    # Precision parameter mu_pos in [0, 1]
    mu_pos = sp.Symbol('mu_pos', positive=True, real=True)
    
    # Epsilon with POS-weighted A_svo
    eps = (kappa * i_kw * d + e_val + c_canon + mu_pos * a_svo) / sp.sqrt(u + delta)
    
    # Partial derivative d(eps) / d(mu_pos)
    deps_dmu = sp.diff(eps, mu_pos)
    print(f"d(eps) / d(mu_pos) = {deps_dmu}")
    
    # Second derivative
    d2eps_dmu2 = sp.diff(deps_dmu, mu_pos)
    print(f"d2(eps) / d(mu_pos)^2 = {d2eps_dmu2}")
    
    # Numerical evaluation for u=15, delta=15, A_svo=4.0, kappa=1.2, i_kw=1.0, d=30.0
    val_deps = deps_dmu.subs({
        u: 15.0,
        delta: 15.0,
        a_svo: 4.0,
        kappa: 1.2
    })
    print(f"Numerical d(eps)/d(mu_pos) at u=15, delta=15, A_svo=4: {float(val_deps):.4f}")
    
    # 2. Spectral Matrix Analysis with POS-Filtered SVO Adjacency
    print("\n--- SPECTRAL MATRIX DECOMPOSITION ---")
    # Character adjacency matrix with 4 characters: [A, B, C, D]
    # SVO action interactions
    A_raw = np.array([
        [0, 12, 5, 1],
        [12, 0, 8, 2],
        [5, 8, 0, 14],
        [1, 2, 14, 0]
    ], dtype=float)
    
    # Homonym Noise Filter Matrix (POS-disambiguated removes 20% false edges)
    POS_filter = np.array([
        [0, 1.0, 0.8, 0.5],
        [1.0, 0, 1.0, 0.8],
        [0.8, 1.0, 0, 1.0],
        [0.5, 0.8, 1.0, 0]
    ])
    
    A_pos = A_raw * POS_filter
    
    eig_raw = np.linalg.eigvals(A_raw)
    eig_pos = np.linalg.eigvals(A_pos)
    
    rho_raw = np.max(np.abs(eig_raw))
    rho_pos = np.max(np.abs(eig_pos))
    
    print(f"Raw Spectral Radius rho(A_raw): {rho_raw:.4f}")
    print(f"POS-Disambiguated Spectral Radius rho(A_pos): {rho_pos:.4f}")
    print(f"Spectral Radius Reduction: {(1 - rho_pos/rho_raw)*100:.2f}%")

if __name__ == "__main__":
    run_pos_math()
