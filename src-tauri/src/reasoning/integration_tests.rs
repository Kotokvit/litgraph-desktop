//! Integration test for the Reasoning Engine.
//!
//! Сценарий: «Пётр умирает в Главе 12, но разговаривает в Главе 15».
//!
//! Этот тест проверяет полный цикл reasoning engine:
//!   1. Observe: получаем события из текста (kill в Г12, speak в Г15)
//!   2. BuildState: inference применяет kill→alive=false
//!   3. Reason: contradiction detector находит temporal paradox
//!   4. Hypothesize: generator предлагает «flashback» / «воскрес»
//!   5. Verify: verifier принимает flashback, отвергает «воскрес без события»
//!   6. UpdateState: применяет классификацию (Speak в Г15 → Flashback)
//!
//! Без LLM. Без Python. Чистый Rust reasoning.

#![cfg(test)]

use crate::models::{LitEdge, LitNode, LitNodeData, Position, Project};
use crate::reasoning::{
    Action, Event, EventKind, HypothesisStatus, Provenance, ReasoningCycle, TemporalAnchor,
};

/// Создать минимальный проект: Иван и Пётр как персонажи, без глав в графе
/// (главы обрабатываются через TemporalAnchor напрямую).
fn make_project() -> Project {
    let now = chrono::Utc::now().timestamp_millis() as u64;
    let ivan = LitNode {
        id: "ivan".to_string(),
        node_type: "character".to_string(),
        position: Position { x: 100.0, y: 100.0 },
        data: LitNodeData {
            title: "Иван".to_string(),
            body: "Главный герой".to_string(),
            node_type: "character".to_string(),
            tags: vec![],
            meta: None,
            full_text: None,
            versions: None,
        },
    };
    let peter = LitNode {
        id: "peter".to_string(),
        node_type: "character".to_string(),
        position: Position { x: 200.0, y: 100.0 },
        data: LitNodeData {
            title: "Пётр".to_string(),
            body: "Антагонист".to_string(),
            node_type: "character".to_string(),
            tags: vec![],
            meta: None,
            full_text: None,
            versions: None,
        },
    };
    Project {
        title: "Тестовый роман".to_string(),
        author: "Test".to_string(),
        description: "Integration test fixture".to_string(),
        nodes: vec![ivan, peter],
        edges: vec![],
        created_at: now,
        updated_at: now,
    }
}

/// Событие 1: Иван убивает Петра в Главе 12.
fn make_kill_event() -> Event {
    Event {
        id: 0,
        actor: "ivan".to_string(),
        action: Action::Kill,
        target: Some("peter".to_string()),
        instrument: None,
        time: TemporalAnchor::new(12),
        source_text: "Иван убил Петра.".to_string(),
        confidence: 0.9,
        provenance: Provenance::SvoParser,
    }
}

/// Событие 2: Пётр разговаривает с Иваном в Главе 15 (после смерти).
fn make_speak_event() -> Event {
    Event {
        id: 0,
        actor: "peter".to_string(),
        action: Action::Speak { topic: None },
        target: Some("ivan".to_string()),
        instrument: None,
        time: TemporalAnchor::new(15),
        source_text: "Пётр сказал Ивану: «Здравствуй, старый друг».".to_string(),
        confidence: 0.9,
        provenance: Provenance::SvoParser,
    }
}

#[test]
fn test_full_cycle_peter_dead_in_ch12_speaks_in_ch15() {
    let project = make_project();
    let mut cycle = ReasoningCycle::from_project(&project);

    // До цикла: оба персонажа живы (from_project устанавливает alive=true).
    let peter_alive_before = cycle.world.get("peter", "alive");
    assert!(
        matches!(peter_alive_before, Some(crate::reasoning::FactValue::Bool(true))),
        "Пётр должен быть жив до цикла, got {:?}",
        peter_alive_before
    );

    // Запускаем цикл с двумя событиями.
    let report = cycle.run_cycle(vec![make_kill_event(), make_speak_event()]);

    // 1. События обработаны.
    assert_eq!(report.events_processed, 2, "Должно быть обработано 2 события");

    // 2. Факты установлены (как минимум: peter.alive=false, ivan знает об убийстве).
    assert!(
        report.facts_asserted >= 1,
        "Должен быть установлен хотя бы один факт, got {}",
        report.facts_asserted
    );

    // 3. Состояние мира: Пётр мёртв.
    let peter_alive_after = cycle.world.get("peter", "alive");
    assert!(
        matches!(peter_alive_after, Some(crate::reasoning::FactValue::Bool(false))),
        "Пётр должен быть мёртв после цикла, got {:?}",
        peter_alive_after
    );

    // 4. Обнаружен хотя бы один temporal paradox (Пётр мёртв, но говорит в Г15).
    assert!(
        !report.temporal_paradoxes.is_empty(),
        "Должен быть обнаружен temporal paradox (Пётр говорит после смерти)"
    );
    let paradox = &report.temporal_paradoxes[0];
    assert!(
        paradox.description.contains("Пётр") || paradox.description.contains("peter"),
        "Описание парадокса должно упоминать Петра: {}",
        paradox.description
    );

    // 5. Сгенерированы гипотезы.
    assert!(
        report.hypotheses_generated >= 1,
        "Должна быть сгенерирована хотя бы одна гипотеза"
    );

    // 6. Хотя бы одна гипотеза принята (flashback).
    assert!(
        report.hypotheses_accepted >= 1,
        "Должна быть принята хотя бы одна гипотеза (flashback)"
    );

    // 7. Событие 2 (Speak в Г15) классифицировано как Flashback.
    let speak_event_id = cycle
        .facts
        .all_events()
        .iter()
        .find(|e| matches!(e.action, Action::Speak { .. }))
        .map(|e| e.id)
        .expect("Speak event should be in FactLog");
    let classification = cycle.event_classification(speak_event_id);
    assert!(
        matches!(classification, Some(EventKind::Flashback) | Some(EventKind::Dream) | Some(EventKind::Vision)),
        "Speak в Г15 должен быть классифицирован как Flashback/Dream/Vision, got {:?}",
        classification
    );

    eprintln!("=== Cycle Report ===");
    eprintln!("  events processed: {}", report.events_processed);
    eprintln!("  facts asserted:   {}", report.facts_asserted);
    eprintln!("  paradoxes:        {}", report.temporal_paradoxes.len());
    eprintln!("  hypotheses gen:   {}", report.hypotheses_generated);
    eprintln!("  hypotheses acc:   {}", report.hypotheses_accepted);
    eprintln!("  paradox desc:     {}", paradox.description);
}

#[test]
fn test_no_paradox_when_peter_alive_speaks() {
    let project = make_project();
    let mut cycle = ReasoningCycle::from_project(&project);

    // Только speak-событие, без kill — Пётр жив и говорит, противоречия нет.
    let report = cycle.run_cycle(vec![make_speak_event()]);

    assert_eq!(report.events_processed, 1);
    assert!(
        report.temporal_paradoxes.is_empty(),
        "Не должно быть парадокса: Пётр жив и говорит"
    );
    let peter_alive = cycle.world.get("peter", "alive");
    assert!(
        matches!(peter_alive, Some(crate::reasoning::FactValue::Bool(true))),
        "Пётр должен оставаться живым"
    );
}

#[test]
fn test_constraint_violation_dead_speaking() {
    use crate::reasoning::ConstraintEngine;

    let project = make_project();
    let mut cycle = ReasoningCycle::from_project(&project);

    // Сначала kill, потом speak — но проверяем через ConstraintEngine напрямую.
    let kill = make_kill_event();
    let speak = make_speak_event();

    // Применяем kill — Пётр становится мёртвым.
    cycle.observe(vec![kill.clone()]);
    let _ = cycle.build_state();

    // Теперь проверяем speak-событие — должно быть violation.
    let engine = ConstraintEngine::default_literary();
    // Сначала регистрируем speak-событие в FactLog, чтобы получить ID.
    let speak_id = cycle.facts.record_event(speak);
    let speak_event = cycle
        .facts
        .all_events()
        .iter()
        .find(|e| e.id == speak_id)
        .unwrap();

    let violations = engine.check(&cycle.world, speak_event);
    assert!(
        !violations.is_empty(),
        "Должно быть violation: мёртвый Пётр пытается говорить"
    );
    let v = &violations[0];
    assert!(
        v.reason.contains("мёртв") || v.constraint_name.contains("dead_cannot_speak"),
        "Причина нарушения должна упоминать смерть: {}",
        v.reason
    );
}

#[test]
fn test_resurrect_without_dying_detected() {
    let project = make_project();
    let mut cycle = ReasoningCycle::from_project(&project);

    // Пётр воскрешает, не умирая — должен быть paradox.
    let resurrect = Event {
        id: 0,
        actor: "peter".to_string(),
        action: Action::Resurrect,
        target: None,
        instrument: None,
        time: TemporalAnchor::new(5),
        source_text: "Пётр воскрес.".to_string(),
        confidence: 0.9,
        provenance: Provenance::SvoParser,
    };

    let report = cycle.run_cycle(vec![resurrect]);

    // Парадокс «воскрес без смерти».
    let has_resurrect_paradox = report
        .temporal_paradoxes
        .iter()
        .any(|p| p.description.contains("воскрес") || p.description.contains("resurrect"));
    assert!(
        has_resurrect_paradox,
        "Должен быть парадокс воскрешения без смерти. Paradoxes: {:?}",
        report
            .temporal_paradoxes
            .iter()
            .map(|p| &p.description)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_cycle_idempotent_on_same_events() {
    // Тот же набор событий, запущенный второй раз, не должен удваивать факты.
    let project = make_project();
    let mut cycle = ReasoningCycle::from_project(&project);

    let events = vec![make_kill_event()];
    let _report1 = cycle.run_cycle(events.clone());
    let facts_after_1 = cycle.facts.all_facts().len();

    let report2 = cycle.run_cycle(events);
    let facts_after_2 = cycle.facts.all_facts().len();

    assert_eq!(
        report2.events_processed, 0,
        "Второй запуск не должен обрабатывать уже обработанные события"
    );
    assert_eq!(
        facts_after_1, facts_after_2,
        "Количество фактов не должно увеличиться при повторном запуске"
    );
}

#[test]
fn test_eval_sfera_predela_full() {
    use std::fs;
    let path = "/home/vitalij/Музика/Нова тека (2)/Нова тека/1-Сфера Предела.md";
    if !std::path::Path::new(path).exists() {
        println!("File 1-Сфера Предела.md not found at {}, skipping eval", path);
        return;
    }
    let text = fs::read_to_string(path).expect("failed to read file");
    let parse_res = crate::parser::build_graph(&text, "Сфера Предела", "Виталий Коток").unwrap();

    let now = chrono::Utc::now().timestamp_millis() as u64;
    let project = Project {
        title: "Сфера Предела".to_string(),
        author: "Виталий Коток".to_string(),
        description: "Eval".to_string(),
        nodes: parse_res.nodes,
        edges: parse_res.edges,
        created_at: now,
        updated_at: now,
    };

    let (chapters, _) = crate::parser::chapters::detect(&text);
    let resolver = crate::reasoning::semantic_parser::EntityResolver::from_nodes(&project.nodes);
    
    // 1. Classic Fallback Pipeline
    let legacy_events = crate::reasoning::semantic_parser::parse_text_fallback(&text, &resolver, &chapters);
    let mut legacy_cycle = ReasoningCycle::from_project(&project);
    let legacy_report = legacy_cycle.run_cycle(legacy_events.clone());

    // 2. New Semantic IR (L1.5) Pipeline
    let instructions = crate::reasoning::semantic_parser::parse_text_to_instructions(&text, &resolver, &chapters);
    let mut ir_cycle = ReasoningCycle::from_project(&project);
    let full_ir_report = ir_cycle.run_cycle_with_instructions(instructions.clone());
    let ir_obs = &full_ir_report.ir_report;

    println!("\n=======================================================");
    println!("   E2E EVALUATION REPORT: Сфера Предела (Semantic IR Pipeline)");
    println!("=======================================================");
    println!("SEMANTIC IR (L1.5) METRICS:");
    println!("Instructions Extracted:  {}", ir_obs.total_instructions);
    println!("Valid Instructions:      {}", ir_obs.valid_instructions);
    println!("Validation Errors:       {}", ir_obs.validation_errors.len());
    println!("IR Conflicts Detected:   {}", ir_obs.conflicts_detected);
    println!("Events Processed (L2):   {}", full_ir_report.events_processed);
    println!("-------------------------------------------------------");
    println!("REASONING CYCLE METRICS:");
    println!("Facts Derived:           {}", full_ir_report.facts_asserted);
    println!("Constraint Violations:   {}", full_ir_report.violations.len());
    println!("Temporal Paradoxes:      {}", full_ir_report.temporal_paradoxes.len());
    println!("Hypotheses Generated:    {}", full_ir_report.hypotheses_generated);
    println!("Hypotheses Accepted:     {}", full_ir_report.hypotheses_accepted);
    println!("-------------------------------------------------------");
    println!("LEGACY FALLBACK COMPARISON:");
    println!("Legacy Events Extracted: {}", legacy_events.len());
    println!("Legacy Violations:       {}", legacy_report.violations.len());
    println!("Legacy Paradoxes:        {}", legacy_report.temporal_paradoxes.len());
    println!("-------------------------------------------------------");

    if !ir_obs.validation_errors.is_empty() {
        println!("\nIR VALIDATION ERRORS (showing first 15 of {}):", ir_obs.validation_errors.len());
        for (idx, err) in ir_obs.validation_errors.iter().take(15).enumerate() {
            println!("#{:2}: {}", idx + 1, err);
        }
    }

    if !ir_obs.conflicts.is_empty() {
        println!("\nIR CONFLICTS DETECTED (showing first 15 of {}):", ir_obs.conflicts.len());
        for (idx, (a, b)) in ir_obs.conflicts.iter().take(15).enumerate() {
            println!("#{:2}: conflict between:\n     A: {}\n     B: {}", idx + 1, a, b);
        }
    }

    println!("\nTOP 25 EXTRACTED INSTRUCTIONS:");
    for (idx, inst) in instructions.iter().take(25).enumerate() {
        println!("#{:2}: {}\n     source: \"{}\"", idx + 1, inst.summary(), inst.source_text);
    }

    if !full_ir_report.violations.is_empty() {
        println!("\nCONSTRAINT VIOLATIONS ({}):", full_ir_report.violations.len());
        for (idx, v) in full_ir_report.violations.iter().take(15).enumerate() {
            println!("#{:2}: [{}] actor='{}', action={:?}, reason='{}'",
                idx + 1, v.constraint_name, v.actor, v.attempted_action, v.reason);
        }
    }

    if !full_ir_report.temporal_paradoxes.is_empty() {
        println!("\nTEMPORAL PARADOXES ({}):", full_ir_report.temporal_paradoxes.len());
        for (idx, p) in full_ir_report.temporal_paradoxes.iter().take(15).enumerate() {
            println!("#{:2}: desc='{}'", idx + 1, p.description);
        }
    }
    println!("=======================================================\n");
}
