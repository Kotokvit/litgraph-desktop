//! Tauri commands — функции, вызываемые из фронтенда через invoke().

pub mod parse_md;
pub mod parse_md_full; // v0.4.0: авто-пайплайн (Rust + NER merge)
pub mod project;
pub mod versions;
pub mod export;
pub mod ai;
pub mod ner;
pub mod conflict;
pub mod reasoning; // Wave 5: Tauri-команды для reasoning engine
pub mod poler; // Layer F: Tauri-команды для POLER v7.5-LEM (Layers A–E)
