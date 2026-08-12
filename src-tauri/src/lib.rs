//! LitGraph Desktop — backend entry point.
//!
//! Здесь регистрируются Tauri commands и плагины.
//! См. docs/PROMPT_PLAN.md для полного плана разработки.

mod commands;
mod parser;
mod models;
mod storage;
mod ai;
mod reasoning;
pub mod languagetool_weights;
pub mod linguistic_entities;
pub mod ukrainian_semantic_categories;
pub mod dict;
pub mod linguistic;
mod poler; // Layer F: bridge to litgraph-core Layer E (NarrativeGraph, ParadoxDetector)


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|_app| {
            // Создаём директорию для проектов, если её нет
            if let Some(home) = dirs::home_dir() {
                let litgraph_dir = home.join(".local/share/litgraph");
                let projects_dir = litgraph_dir.join("projects");
                let backups_dir = litgraph_dir.join("backups");
                std::fs::create_dir_all(&projects_dir).ok();
                std::fs::create_dir_all(&backups_dir).ok();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // parse-md
            commands::parse_md::parse_md,
            commands::parse_md_full::parse_md_full, // v0.4.0: авто-пайплайн
            // project
            commands::project::list_projects,
            commands::project::load_project,
            commands::project::save_project,
            commands::project::delete_project,
            // versions
            commands::versions::save_version,
            commands::versions::restore_version,
            commands::versions::delete_version,
            commands::versions::list_versions,
            // export
            commands::export::export_project,
            // ai
            commands::ai::ai_assistant,
            commands::ai::ai_continue_chapter,
            commands::ai::ai_analyze_plot,
            commands::ai::ai_test_connection,
            commands::ai::ai_list_ollama_models,
            // ner — NER-извлечение через spaCy
            commands::ner::extract_entities,
            commands::ner::analyze_characters,
            commands::ner::extract_svo,
            // conflict — конфликт-граф (SVO → J-матрица → directed graph)
            commands::conflict::get_conflict_graph,
            // reasoning — Wave 5: интеллектуальный движок рассуждения
            commands::reasoning::reasoning_extract_events,
            commands::reasoning::reasoning_extract_instructions,
            commands::reasoning::reasoning_run_cycle,
            commands::reasoning::reasoning_run_cycle_with_ir,
            commands::reasoning::reasoning_get_world_state,
            commands::reasoning::reasoning_validate_text,
            // reasoning v0.7+: full 7-stage pipeline (Burn weights + case validation + diagnostics)
            commands::reasoning::reasoning_run_full_pipeline,
            // poler — Layer F: POLER v7.5-LEM (Rust-native, no Python dep)
            commands::poler::cmd_compute_epsilon_climax,
            commands::poler::cmd_extract_svo,
            commands::poler::cmd_detect_paradoxes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
