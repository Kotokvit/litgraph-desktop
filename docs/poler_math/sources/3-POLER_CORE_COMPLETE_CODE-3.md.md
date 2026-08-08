POLER[Ψ] — Complete Source Code Reference
Version: 0.2.0-unified (core) + 0.3.0-core-delegate (bridge) Date: 2026-04-17 Status: Phase 0 COMPLETE (5/5 tests pass) + Phase 1 TCP Bridge COMPLETE Build: cargo test ✅ (5 passed, 0 failed) | cargo check ✅ (0 errors, 16 warnings)
--------------------------------------------------------------------------------
Table of Contents
 — Cargo Configuration (16 lines)
 — Cargo Configuration (25 lines)
 — Core Library Entry Point (64 lines)
 — Energy Engine (Ур.5–10, 13, 14) (463 lines)
 — Synaptic Constraint Operations (SCTP) (353 lines)
 — Quantum Bridge (Qubits as Archetypes) (279 lines)
 — Vulkan-like Hardware Accelerator (324 lines)
 — Subquantum Bridge — Full Physics Engine (1721 lines)
 — Full Instruction Index — 100 Rules (2094 lines)
 — Phase 0 Convergence Integration Tests (191 lines)
 — Bridge Library Entry Point (35 lines)
 — Bridge Loader (JSON Registry Import) (235 lines)
 — TCP Server (Python ↔ Rust Bridge) (487 lines)
 — TCP Server Binary Entry Point (26 lines)
 — Quick Test Example (78 lines)
 — Self-Learning Demo Example (126 lines)
 — Shared TCP Protocol v2.0 (188 lines)
 — TCP Client Demo (4 modes) (328 lines)
 — Phase 1 Integration Test Runner (150 lines)
Total: 19 files, 7183 lines of source code.
--------------------------------------------------------------------------------
Architecture Overview
--------------------------------------------------------------------------------
Key Equations
--------------------------------------------------------------------------------
1. poler-core/Cargo.toml
Path: poler-core/Cargo.toml | Lines: 16
--------------------------------------------------------------------------------
2. poler-bridge/Cargo.toml
Path: poler-bridge/Cargo.toml | Lines: 25
--------------------------------------------------------------------------------
3. poler-core/src/lib.rs
Path: poler-core/src/lib.rs | Lines: 64
--------------------------------------------------------------------------------
4. poler-core/src/energy_engine.rs
Path: poler-core/src/energy_engine.rs | Lines: 463
--------------------------------------------------------------------------------
5. poler-core/src/synaptic_ops.rs
Path: poler-core/src/synaptic_ops.rs | Lines: 353
--------------------------------------------------------------------------------
6. poler-core/src/quantum_bridge.rs
Path: poler-core/src/quantum_bridge.rs | Lines: 279
--------------------------------------------------------------------------------
7. poler-core/src/vulkan_accelerator.rs
Path: poler-core/src/vulkan_accelerator.rs | Lines: 324
--------------------------------------------------------------------------------
8. poler-core/src/subquantum_bridge.rs
Path: poler-core/src/subquantum_bridge.rs | Lines: 1721
--------------------------------------------------------------------------------
9. poler-core/src/instruction_index.rs
Path: poler-core/src/instruction_index.rs | Lines: 2094
--------------------------------------------------------------------------------
10. poler-core/tests/phase0_convergence.rs
Path: poler-core/tests/phase0_convergence.rs | Lines: 191
--------------------------------------------------------------------------------
11. poler-bridge/src/lib.rs
Path: poler-bridge/src/lib.rs | Lines: 35
--------------------------------------------------------------------------------
12. poler-bridge/src/bridge_loader.rs
Path: poler-bridge/src/bridge_loader.rs | Lines: 235
--------------------------------------------------------------------------------
13. poler-bridge/src/tcp_server.rs
Path: poler-bridge/src/tcp_server.rs | Lines: 487
--------------------------------------------------------------------------------
14. poler-bridge/src/bin/poler_tcp_server.rs
Path: poler-bridge/src/bin/poler_tcp_server.rs | Lines: 26
--------------------------------------------------------------------------------
15. poler-bridge/examples/quick_test.rs
Path: poler-bridge/examples/quick_test.rs | Lines: 78
--------------------------------------------------------------------------------
16. poler-bridge/examples/self_learning_demo.rs
Path: poler-bridge/examples/self_learning_demo.rs | Lines: 126
--------------------------------------------------------------------------------
17. poler-tcp/shared_protocol.py
Path: poler-tcp/shared_protocol.py | Lines: 188
--------------------------------------------------------------------------------
18. poler-tcp/client_demo.py
Path: poler-tcp/client_demo.py | Lines: 328
--------------------------------------------------------------------------------
19. poler-tcp/run_phase1_test.py
Path: poler-tcp/run_phase1_test.py | Lines: 150
--------------------------------------------------------------------------------
Build Verification
--------------------------------------------------------------------------------
Phase 1 TCP Integration Test
--------------------------------------------------------------------------------
Generated automatically from project source files. Total: 7183 lines across 19 files.