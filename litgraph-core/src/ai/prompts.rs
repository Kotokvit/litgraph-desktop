//! Общие промпты для AI-функций.
//! Перенесено из src/app/api/ai/*/route.ts (TS).

use crate::ai::ChatMessage;
use crate::models::Project;
use fancy_regex::Regex;

/// Извлечь номер главы из заголовка "Глава N: ..."
pub fn chapter_num(title: &str) -> Option<u32> {
    let re = Regex::new(r"(?i)Глава\s+(\d+)").ok()?;
    let caps = re.captures(title).ok()??;
    caps.get(1)?.as_str().parse().ok()
}

/// Сборка контекста проекта для AI-помощника.
/// Возвращает (system_prompt, user_prompt).
pub fn build_assistant_prompt(
    project: &Project,
    user_message: &str,
    selected_node_id: &Option<String>,
) -> (String, String) {
    let chapters: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "chapter")
        .collect();
    let scenes: Vec<&_> = project.nodes.iter().filter(|n| n.node_type == "scene").collect();
    let characters: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "character")
        .collect();
    let locations: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "location")
        .collect();
    let plot_points: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "plotpoint")
        .collect();
    let conflicts: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "conflict")
        .collect();
    let themes: Vec<&_> = project.nodes.iter().filter(|n| n.node_type == "theme").collect();

    // Метрики: объём глав
    let chapter_word_counts: Vec<usize> = chapters
        .iter()
        .map(|c| {
            c.data
                .full_text
                .as_deref()
                .or(Some(c.data.body.as_str()))
                .unwrap_or("")
                .split_whitespace()
                .count()
        })
        .collect();
    let avg_words = if chapter_word_counts.is_empty() {
        0
    } else {
        chapter_word_counts.iter().sum::<usize>() / chapter_word_counts.len()
    };

    // Сводка по темам
    let themes_summary = themes
        .iter()
        .map(|t| {
            let related_count = project
                .edges
                .iter()
                .filter(|e| {
                    (e.source == t.id || e.target == t.id)
                        && e.data.as_ref().and_then(|d| d.kind.as_deref()) == Some("theme")
                })
                .count();
            format!(
                "- {}: {} (проявляется в {} главах/сценах)",
                t.data.title,
                t.data.body.chars().take(150).collect::<String>(),
                related_count
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Сводка по персонажам (топ-10)
    let mut char_counts: Vec<(&_, usize)> = characters
        .iter()
        .map(|c| {
            let count = project
                .edges
                .iter()
                .filter(|e| {
                    e.source == c.id
                        && e.data.as_ref().and_then(|d| d.kind.as_deref()) == Some("character")
                })
                .count();
            (c, count)
        })
        .collect();
    char_counts.sort_by(|a, b| b.1.cmp(&a.1));
    let top_characters = char_counts
        .iter()
        .take(10)
        .map(|(c, count)| {
            format!(
                "- {}: {} ({} глав)",
                c.data.title,
                c.data.body.chars().take(100).collect::<String>(),
                count
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let plot_summary = plot_points
        .iter()
        .map(|p| {
            format!(
                "- {}: {}",
                p.data.title,
                p.data.body.chars().take(120).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let conflicts_summary = conflicts
        .iter()
        .map(|c| {
            format!(
                "- {}: {}",
                c.data.title,
                c.data.body.chars().take(120).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let locs_summary = locations
        .iter()
        .take(10)
        .map(|l| {
            format!(
                "- {}: {}",
                l.data.title,
                l.data.body.chars().take(100).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Конспект глав
    let chapters_outline = chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let num = chapter_num(&c.data.title).unwrap_or((i + 1) as u32);
            let wc = chapter_word_counts.get(i).copied().unwrap_or(0);
            format!(
                "Глава {}: {} — {} слов. Кратко: {}",
                num,
                c.data.title,
                wc,
                c.data.body.chars().take(150).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Если выбрана нода — добавим контекст
    let mut selected_context = String::new();
    if let Some(node_id) = selected_node_id {
        if let Some(sel) = project.nodes.iter().find(|n| &n.id == node_id) {
            let text = sel
                .data
                .full_text
                .as_deref()
                .unwrap_or(&sel.data.body);
            let trimmed = if text.len() > 4000 {
                let mut end = 4000;
                while end > 0 && !text.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}…", &text[..end])
            } else {
                text.to_string()
            };
            selected_context = format!(
                "\n\n=== ВЫБРАННАЯ НОДА (id={}, type={}) ===\n{}\n\n{}",
                sel.id, sel.node_type, sel.data.title, trimmed
            );
        }
    }

    let system_prompt = format!(
        "Ты — литературный редактор, соавтор и аналитик в одном лице. Ты помогаешь писателю работать над произведением «{}». Ты видишь структуру произведения как граф (главы, персонажи, локации, темы, конфликты, сюжетные точки) и можешь отвечать на любые вопросы автора: помочь с сюжетом, подсказать идею, проанализировать структуру, найти противоречия, обсудить персонажа, тему или сцену.\n\nОтвечай на том же языке, на котором задан вопрос автора (русский, украинский или английский). Будь конкретен — ссылайся на номера глав, имена персонажей, названия тем. Если автор спрашивает о конкретной ноде — используй её контекст. Если просишь генерацию текста — пиши в стиле автора.\n\nНе повторяй структуру графа обратно автору — он её и так видит. Сразу переходи к делу.",
        project.title
    );

    let description_line = if project.description.is_empty() {
        String::new()
    } else {
        format!("Описание: {}\n", project.description)
    };

    let user_prompt = format!(
        "Контекст произведения:\n\n{description_line}Структура:\n- {} глав (средний объём: {} слов)\n- {} сцен\n- {} персонажей\n- {} локаций\n- {} сюжетных точек\n- {} конфликтов\n- {} тем/мотивов\n\nТЕМЫ И МОТИВЫ:\n{}\n\nПЕРСОНАЖИ (топ-10 по присутствию):\n{}\n\nСЮЖЕТНЫЕ ТОЧКИ:\n{}\n\nКОНФЛИКТЫ:\n{}\n\nЛОКАЦИИ:\n{}\n\nКОНСПЕКТ ГЛАВ:\n{}{}\n\n=== ВОПРОС АВТОРА ===\n{}",
        chapters.len(),
        avg_words,
        scenes.len(),
        characters.len(),
        locations.len(),
        plot_points.len(),
        conflicts.len(),
        themes.len(),
        if themes_summary.is_empty() { "(пока не заданы — автор может создать их как ноды типа «Тема»)" } else { &themes_summary },
        if top_characters.is_empty() { "—" } else { &top_characters },
        if plot_summary.is_empty() { "—" } else { &plot_summary },
        if conflicts_summary.is_empty() { "—" } else { &conflicts_summary },
        if locs_summary.is_empty() { "—" } else { &locs_summary },
        if chapters_outline.is_empty() { "—" } else { &chapters_outline },
        selected_context,
        user_message
    );

    (system_prompt, user_prompt)
}

/// Промпт для "Дописать главу"
pub fn build_continue_chapter_prompt(
    project: &Project,
    from_chapter_id: &Option<String>,
    custom_prompt: &Option<String>,
) -> (String, String) {
    let mut chapters: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "chapter")
        .collect();
    chapters.sort_by(|a, b| {
        let na = chapter_num(&a.data.title).unwrap_or(0);
        let nb = chapter_num(&b.data.title).unwrap_or(0);
        na.cmp(&nb)
    });

    let last_chapter = if let Some(id) = from_chapter_id {
        chapters.iter().find(|c| c.id == *id).cloned()
    } else {
        chapters.last().cloned()
    };

    let last_chapter = match last_chapter {
        Some(c) => c,
        None => return ("Нет глав".to_string(), String::new()),
    };

    let last_idx = chapters.iter().position(|c| c.id == last_chapter.id).unwrap_or(0);
    let context_start = last_idx.saturating_sub(2);
    let context_chapters: Vec<&_> = chapters
        .iter()
        .skip(context_start)
        .take(last_idx - context_start + 1)
        .collect();

    let context_chapter_ids: std::collections::HashSet<&str> = context_chapters.iter().map(|c| c.id.as_str()).collect();

    // Персонажи в контексте
    let char_ids: std::collections::HashSet<String> = project
        .edges
        .iter()
        .filter(|e| {
            e.data.as_ref().and_then(|d| d.kind.as_deref()) == Some("character")
                && context_chapter_ids.contains(e.target.as_str())
        })
        .map(|e| e.source.clone())
        .collect();
    let characters_in_context: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "character" && char_ids.contains(&n.id))
        .collect();

    // Локации в контексте
    let loc_ids: std::collections::HashSet<String> = project
        .edges
        .iter()
        .filter(|e| {
            e.data.as_ref().and_then(|d| d.kind.as_deref()) == Some("location")
                && context_chapter_ids.contains(e.target.as_str())
        })
        .map(|e| e.source.clone())
        .collect();
    let locations_in_context: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "location" && loc_ids.contains(&n.id))
        .collect();

    // Сюжетные точки
    let plot_points: Vec<&_> = project
        .edges
        .iter()
        .filter(|e| {
            e.target == last_chapter.id
                && e.data.as_ref().and_then(|d| d.kind.as_deref()) == Some("cause")
        })
        .filter_map(|e| project.nodes.iter().find(|n| n.id == e.source))
        .filter(|n| n.node_type == "plotpoint")
        .collect();

    let ctx_summary = context_chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let num = chapter_num(&c.data.title).unwrap_or((i + 1) as u32);
            let text = c.data.full_text.as_deref().unwrap_or(&c.data.body);
            let trimmed = if text.len() > 3000 {
                let mut end = 3000;
                while end > 0 && !text.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}…", &text[..end])
            } else {
                text.to_string()
            };
            format!("=== Глава {}: {} ===\n{}", num, c.data.title, trimmed)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let chars_list = characters_in_context
        .iter()
        .map(|c| format!("- {}: {}", c.data.title, c.data.body))
        .collect::<Vec<_>>()
        .join("\n");

    let locs_list = locations_in_context
        .iter()
        .map(|l| format!("- {}: {}", l.data.title, l.data.body))
        .collect::<Vec<_>>()
        .join("\n");

    let pp_list = plot_points
        .iter()
        .map(|p| format!("- {}: {}", p.data.title, p.data.body))
        .collect::<Vec<_>>()
        .join("\n");

    let last_num = chapter_num(&last_chapter.data.title);
    let next_num = last_num.map(|n| n + 1).unwrap_or(chapters.len() as u32 + 1);

    let system_prompt = "Ты — опытный писатель-соавтор, который помогает дописывать художественные произведения. Ты пишешь в стиле автора, сохраняешь персонажей, атмосферу и тональность. Пишешь на том же языке, что и исходный текст. Не вставляй метаркомментарии — только текст главы.".to_string();

    let mut user_prompt = format!(
        "Я работаю над произведением «{}».{}\n{}\n{}\n{}\nПоследние главы для контекста:\n\n{}\n\n=== ЗАДАНИЕ ===\nНапиши следующую главу (Глава {}). Продолжи сюжет с того места, где остановилась Глава {}. Объём — 1500-2500 слов. Сохрани стиль, атмосферу, персонажей и тональность исходного текста. Не повторяйся, развивай сюжет. Не вставляй заголовок «Глава {}» — только текст.",
        project.title,
        if project.description.is_empty() { String::new() } else { format!("\nОписание: {}", project.description) },
        if chars_list.is_empty() { String::new() } else { format!("\nПерсонажи в последних главах:\n{}\n", chars_list) },
        if locs_list.is_empty() { String::new() } else { format!("Локации:\n{}\n", locs_list) },
        if pp_list.is_empty() { String::new() } else { format!("Активные сюжетные линии:\n{}\n", pp_list) },
        ctx_summary,
        next_num,
        last_num.unwrap_or(chapters.len() as u32),
        next_num
    );

    if let Some(cp) = custom_prompt {
        if !cp.trim().is_empty() {
            user_prompt.push_str(&format!("\n\nДополнительные указания автора:\n{}", cp));
        }
    }

    (system_prompt, user_prompt)
}

/// Промпт для "Анализ сюжета"
pub fn build_analyze_plot_prompt(project: &Project, focus: &str) -> (String, String) {
    let mut chapters: Vec<&_> = project
        .nodes
        .iter()
        .filter(|n| n.node_type == "chapter")
        .collect();
    chapters.sort_by(|a, b| {
        let na = chapter_num(&a.data.title).unwrap_or(0);
        let nb = chapter_num(&b.data.title).unwrap_or(0);
        na.cmp(&nb)
    });
    let characters: Vec<&_> = project.nodes.iter().filter(|n| n.node_type == "character").collect();
    let locations: Vec<&_> = project.nodes.iter().filter(|n| n.node_type == "location").collect();
    let plot_points: Vec<&_> = project.nodes.iter().filter(|n| n.node_type == "plotpoint").collect();
    let conflicts: Vec<&_> = project.nodes.iter().filter(|n| n.node_type == "conflict").collect();

    let chapter_word_counts: Vec<usize> = chapters
        .iter()
        .map(|c| {
            c.data
                .full_text
                .as_deref()
                .or(Some(c.data.body.as_str()))
                .unwrap_or("")
                .split_whitespace()
                .count()
        })
        .collect();
    let avg_words = if chapter_word_counts.is_empty() {
        0
    } else {
        chapter_word_counts.iter().sum::<usize>() / chapter_word_counts.len()
    };
    let min_words = chapter_word_counts.iter().copied().min().unwrap_or(0);
    let max_words = chapter_word_counts.iter().copied().max().unwrap_or(0);

    // Недоразвитые персонажи
    let mut char_counts: Vec<(&_, usize)> = characters
        .iter()
        .map(|c| {
            let count = project
                .edges
                .iter()
                .filter(|e| {
                    e.source == c.id
                        && e.data.as_ref().and_then(|d| d.kind.as_deref()) == Some("character")
                })
                .count();
            (c, count)
        })
        .collect();
    char_counts.sort_by(|a, b| a.1.cmp(&b.1));
    let underused = char_counts
        .iter()
        .take(5)
        .map(|(c, count)| format!("{} (участвует в {} главах)", c.data.title, count))
        .collect::<Vec<_>>()
        .join("\n");

    // Главы без персонажей
    let chapters_with_chars: std::collections::HashSet<String> = project
        .edges
        .iter()
        .filter(|e| e.data.as_ref().and_then(|d| d.kind.as_deref()) == Some("character"))
        .map(|e| e.target.clone())
        .collect();
    let orphan_chapters: Vec<&_> = chapters
        .iter()
        .filter(|c| !chapters_with_chars.contains(&c.id))
        .collect();
    let orphan_list = orphan_chapters
        .iter()
        .map(|c| format!("- {}", c.data.title))
        .collect::<Vec<_>>()
        .join("\n");

    // Слишком короткие/длинные
    let short_threshold = (avg_words as f64 * 0.4) as usize;
    let long_threshold = (avg_words as f64 * 1.8) as usize;
    let short_chapters: Vec<String> = chapters
        .iter()
        .enumerate()
        .filter(|(i, _)| chapter_word_counts.get(*i).copied().unwrap_or(0) < short_threshold)
        .map(|(_, c)| format!("- {}", c.data.title))
        .collect();
    let long_chapters: Vec<String> = chapters
        .iter()
        .enumerate()
        .filter(|(i, _)| chapter_word_counts.get(*i).copied().unwrap_or(0) > long_threshold)
        .map(|(_, c)| format!("- {}", c.data.title))
        .collect();

    let chapters_outline = chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let num = chapter_num(&c.data.title).unwrap_or((i + 1) as u32);
            let wc = chapter_word_counts.get(i).copied().unwrap_or(0);
            format!(
                "Глава {}: {} — {} слов. Кратко: {}",
                num,
                c.data.title,
                wc,
                c.data.body.chars().take(200).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let chars_outline = characters
        .iter()
        .map(|c| format!("- {}: {}", c.data.title, c.data.body.chars().take(150).collect::<String>()))
        .collect::<Vec<_>>()
        .join("\n");

    let plot_outline = plot_points
        .iter()
        .map(|p| format!("- {}: {}", p.data.title, p.data.body.chars().take(150).collect::<String>()))
        .collect::<Vec<_>>()
        .join("\n");

    let conflicts_outline = conflicts
        .iter()
        .map(|c| format!("- {}: {}", c.data.title, c.data.body.chars().take(150).collect::<String>()))
        .collect::<Vec<_>>()
        .join("\n");

    let focus_text = match focus {
        "plot" => "Сосредоточься на сюжете: есть ли логические дыры, натянутые повороты, нераскрытые линии?",
        "characters" => "Сосредоточься на персонажах: у кого слабая дуга? Кто появляется и исчезает? Кто избыточен?",
        "pacing" => "Сосредоточься на темпе и ритме: какие главы провисают? Где слишком быстро/медленно?",
        _ => "Проведи полный анализ: сюжет, персонажи, темп, логика, эмоциональная дуга.",
    };

    let system_prompt = "Ты — опытный литературный редактор и аналитик. Ты анализируешь структуру художественного произведения и находишь слабые места. Ты пишешь на том же языке, что и исходный текст (русский, украинский или английский). Ты конкретен: называешь номера глав, имена персонажей, цитируешь проблемы. Не льстишь — указываешь и сильные стороны, и слабые.".to_string();

    let short_chapters_str = if short_chapters.is_empty() {
        "—".to_string()
    } else {
        short_chapters.join("\n")
    };
    let long_chapters_str = if long_chapters.is_empty() {
        "—".to_string()
    } else {
        long_chapters.join("\n")
    };

    let user_prompt = format!(
        "Я работаю над произведением «{}».{}\n\nСтруктура графа:\n- {} глав (средний объём: {} слов; мин: {}; макс: {})\n- {} персонажей\n- {} локаций\n- {} сюжетных точек\n- {} конфликтов\n\nПерсонажи с наименьшим присутствием (потенциально недоразвитые):\n{}\n\nГлавы без явного участия персонажей (одинокие):\n{}\n\nСлишком короткие главы (меньше 40% от среднего):\n{}\n\nСлишком длинные главы (больше 180% от среднего):\n{}\n\nСтруктура глав:\n{}\n\nПерсонажи:\n{}\n\nСюжетные точки:\n{}\n\nКонфликты:\n{}\n\n=== ЗАДАНИЕ ===\n{}\n\nФормат ответа (на русском):\n## 🟢 Сильные стороны\n(3-5 пунктов)\n\n## 🔴 Слабые места\n(3-7 пунктов, конкретно — с номерами глав и именами)\n\n## 💡 Рекомендации\n(3-5 конкретных действий, что доработать)\n\n## ⚠️ Логические нестыковки\n(если есть — с указанием глав)",
        project.title,
        if project.description.is_empty() { String::new() } else { format!("\nОписание: {}", project.description) },
        chapters.len(),
        avg_words,
        min_words,
        max_words,
        characters.len(),
        locations.len(),
        plot_points.len(),
        conflicts.len(),
        if underused.is_empty() { "—" } else { &underused },
        if orphan_list.is_empty() { "—" } else { &orphan_list },
        &short_chapters_str,
        &long_chapters_str,
        if chapters_outline.is_empty() { "—" } else { &chapters_outline },
        if chars_outline.is_empty() { "—" } else { &chars_outline },
        if plot_outline.is_empty() { "—" } else { &plot_outline },
        if conflicts_outline.is_empty() { "—" } else { &conflicts_outline },
        focus_text
    );

    (system_prompt, user_prompt)
}

/// Собрать сообщения для AI из (system, user) + история
pub fn build_messages(system: &str, user: &str, history: &[ChatMessage]) -> Vec<ChatMessage> {
    let mut messages = vec![
        ChatMessage { role: "system".to_string(), content: system.to_string() },
        ChatMessage { role: "user".to_string(), content: user.to_string() },
    ];
    let recent: Vec<ChatMessage> = history
        .iter()
        .rev()
        .take(6)
        .rev()
        .cloned()
        .collect();
    messages.extend(recent);
    messages
}
