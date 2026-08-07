//! Экспорт проекта в JSON / текст / Markdown.

use crate::models::Project;
use std::fs;
use std::path::Path;

#[tauri::command]
pub async fn export_project(
    project: Project,
    format: String,
    path: String,
) -> Result<(), String> {
    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?,
        "text" => export_to_text(&project),
        "markdown" | "md" => export_to_markdown(&project),
        _ => return Err(format!("Неизвестный формат: {}", format)),
    };

    fs::write(&path, content).map_err(|e| format!("Не удалось записать файл: {}", e))?;
    Ok(())
}

fn chapter_num(title: &str) -> Option<u32> {
    use fancy_regex::Regex;
    let re = Regex::new(r"(?i)Глава\s+(\d+)").ok()?;
    let caps = re.captures(title).ok()??;
    caps.get(1)?.as_str().parse().ok()
}

fn export_to_text(project: &Project) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(project.title.clone());
    lines.push(format!("Автор: {}", project.author));
    if !project.description.is_empty() {
        lines.push(String::new());
        lines.push(project.description.clone());
    }
    lines.push(String::new());
    lines.push("========================================".to_string());
    lines.push(String::new());

    // Группируем по типам
    let type_order = ["chapter", "scene", "plotpoint", "conflict", "character", "dialogue", "location", "theme", "idea"];
    let type_labels: &[(&str, &str)] = &[
        ("chapter", "ГЛАВЫ"),
        ("scene", "СЦЕНЫ"),
        ("plotpoint", "СЮЖЕТНЫЕ ТОЧКИ"),
        ("conflict", "КОНФЛИКТЫ"),
        ("character", "ПЕРСОНАЖИ"),
        ("dialogue", "ДИАЛОГИ"),
        ("location", "ЛОКАЦИИ"),
        ("theme", "ТЕМЫ И МОТИВЫ"),
        ("idea", "ИДЕИ"),
    ];

    for t in &type_order {
        let nodes: Vec<&_> = project.nodes.iter().filter(|n| &n.node_type == t).collect();
        if nodes.is_empty() {
            continue;
        }
        let label = type_labels.iter().find(|(k, _)| k == t).map(|(_, v)| *v).unwrap_or("ПРОЧЕЕ");
        lines.push(format!("◆ {} ({})", label, nodes.len()));
        lines.push("────────────────────────────────────────".to_string());
        for (i, n) in nodes.iter().enumerate() {
            lines.push(format!("{}. {}", i + 1, n.data.title));
            if !n.data.body.is_empty() {
                lines.push(format!("   {}", n.data.body.replace('\n', "\n   ")));
            }
            if let Some(meta) = &n.data.meta {
                if let Some(obj) = meta.as_object() {
                    let entries: Vec<_> = obj.iter().filter(|(_, v)| !v.is_null()).collect();
                    if !entries.is_empty() {
                        lines.push("   [метаданные]".to_string());
                        for (k, v) in entries {
                            lines.push(format!("   • {}: {}", k, v));
                        }
                    }
                }
            }
            if !n.data.tags.is_empty() {
                let tags_str = n.data.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ");
                lines.push(format!("   {}", tags_str));
            }
            lines.push(String::new());
        }
        lines.push(String::new());
    }

    // Связи
    if !project.edges.is_empty() {
        lines.push("◆ СВЯЗИ".to_string());
        lines.push("────────────────────────────────────────".to_string());
        for (i, e) in project.edges.iter().enumerate() {
            let src = project.nodes.iter().find(|n| n.id == e.source);
            let tgt = project.nodes.iter().find(|n| n.id == e.target);
            if let (Some(s), Some(t)) = (src, tgt) {
                let kind = e.data.as_ref().and_then(|d| d.kind.as_deref()).unwrap_or("связь");
                let kind_label = match kind {
                    "flow" => "Поток сюжета",
                    "cause" => "Причина → следствие",
                    "character" => "Участие персонажа",
                    "location" => "Место действия",
                    "reference" => "Упоминание / ссылка",
                    "conflict" => "Конфликт",
                    "foreshadow" => "Предзнаменование",
                    "alternative" => "Альтернативная ветка",
                    "theme" => "Тема / мотив",
                    _ => "связь",
                };
                lines.push(format!("{}. [{}] {} → {}", i + 1, kind_label, s.data.title, t.data.title));
                if let Some(note) = e.data.as_ref().and_then(|d| d.note.as_deref()) {
                    if !note.is_empty() {
                        lines.push(format!("   {}", note));
                    }
                }
            }
        }
    }

    lines.join("\n")
}

fn export_to_markdown(project: &Project) -> String {
    let mut lines: Vec<String> = Vec::new();

    lines.push(format!("# {}", project.title));
    lines.push(String::new());
    lines.push(format!("**Автор:** {}  ", project.author));
    if !project.description.is_empty() {
        lines.push(String::new());
        lines.push(format!("> {}", project.description));
    }
    lines.push(String::new());

    let type_order = ["chapter", "scene", "plotpoint", "conflict", "character", "dialogue", "location", "theme", "idea"];
    let type_labels: &[(&str, &str)] = &[
        ("chapter", "Главы"),
        ("scene", "Сцены"),
        ("plotpoint", "Сюжетные точки"),
        ("conflict", "Конфликты"),
        ("character", "Персонажи"),
        ("dialogue", "Диалоги"),
        ("location", "Локации"),
        ("theme", "Темы и мотивы"),
        ("idea", "Идеи"),
    ];

    for t in &type_order {
        let nodes: Vec<&_> = project.nodes.iter().filter(|n| &n.node_type == t).collect();
        if nodes.is_empty() {
            continue;
        }
        let label = type_labels.iter().find(|(k, _)| k == t).map(|(_, v)| *v).unwrap_or("Прочее");
        lines.push(format!("## {}", label));
        lines.push(String::new());
        for (i, n) in nodes.iter().enumerate() {
            lines.push(format!("### {}. {}", i + 1, n.data.title));
            lines.push(String::new());
            if !n.data.body.is_empty() {
                lines.push(n.data.body.clone());
                lines.push(String::new());
            }
            if let Some(meta) = &n.data.meta {
                if let Some(obj) = meta.as_object() {
                    for (k, v) in obj.iter().filter(|(_, v)| !v.is_null()) {
                        lines.push(format!("- **{}:** {}", k, v));
                    }
                    if !obj.is_empty() {
                        lines.push(String::new());
                    }
                }
            }
            if !n.data.tags.is_empty() {
                let tags_str = n.data.tags.iter().map(|t| format!("`#{}`", t)).collect::<Vec<_>>().join(" ");
                lines.push(format!("Теги: {}", tags_str));
                lines.push(String::new());
            }
        }
    }

    if !project.edges.is_empty() {
        lines.push("## Связи".to_string());
        lines.push(String::new());
        for e in &project.edges {
            let src = project.nodes.iter().find(|n| n.id == e.source);
            let tgt = project.nodes.iter().find(|n| n.id == e.target);
            if let (Some(s), Some(t)) = (src, tgt) {
                let kind = e.data.as_ref().and_then(|d| d.kind.as_deref()).unwrap_or("связь");
                let kind_label = match kind {
                    "flow" => "Поток сюжета",
                    "cause" => "Причина → следствие",
                    "character" => "Участие персонажа",
                    "location" => "Место действия",
                    "reference" => "Упоминание / ссылка",
                    "conflict" => "Конфликт",
                    "foreshadow" => "Предзнаменование",
                    "alternative" => "Альтернативная ветка",
                    "theme" => "Тема / мотив",
                    _ => "связь",
                };
                lines.push(format!("- **[{}]** {} → {}", kind_label, s.data.title, t.data.title));
            }
        }
    }

    // Suppress unused warning
    let _ = Path::new("");
    let _ = chapter_num("");

    lines.join("\n")
}
