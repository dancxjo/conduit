#[path = "common/face.rs"]
mod fixture;
use conduit_core::{ConfigurationValue, HostAdvertisement, PortDirection};
use conduit_face::CheckedSourcePlanInputs;
use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CanonicalBackCatalog, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_todo_face::{
    PreparedTodoSpokenTemplate, TodoFaceFacts, TodoTemplateRefusal, TODO_SPOKEN_TEMPLATE_SOURCE,
};
use std::collections::BTreeMap;
fn producer(name: &str) -> PreparedTodoSpokenTemplate {
    let checked = check_syntax_document(
        &parse_syntax_document(TODO_SPOKEN_TEMPLATE_SOURCE),
        &StartupCatalog::new(),
    )
    .unwrap();
    let profiles = ProfileCatalog::new();
    let expanded = expand_canonical_plot_for_authoring(&checked, name, &profiles).unwrap();
    let gear = &expanded.expanded.gears[0];
    let ConfigurationValue::Text(program) = &gear.configuration[0].value else {
        panic!("program");
    };
    let program = PortableExpressionProgram::from_canonical_hex(program).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "fixture/spoken-template".into(),
        boot_id: "fixture/spoken-template/boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "fixture/spoken-template".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![conduit_std_offers::pure_expression_std_offer(
            &program,
            gear.inputs[0].temporal,
        )
        .unwrap()],
    }];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let boundaries: BTreeMap<_, _> = expanded
        .input_bindings
        .iter()
        .map(|binding| (PortDirection::Input, binding))
        .chain(
            expanded
                .output_bindings
                .iter()
                .map(|binding| (PortDirection::Output, binding)),
        )
        .map(|(direction, binding)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 128 * 1024,
                },
            )
        })
        .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let options = || PlanningOptions {
        connection_bases: &empty,
        line_candidates: &lines,
        connection_item_capacity: 1,
        connection_byte_capacity: 128 * 1024,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        options(),
        &boundaries,
    )
    .unwrap();
    let prepared = conduit_face::prepare_checked_source_plan(
        &checked,
        &expanded,
        &profiles,
        &CanonicalBackCatalog::new(),
        &plan,
        CheckedSourcePlanInputs {
            hosts: &hosts,
            placements: &placements,
            bases: &["conduit.base/local@1".into()],
            options: options(),
            boundary_limits: &boundaries,
        },
    )
    .unwrap();
    PreparedTodoSpokenTemplate::prepare(&prepared).unwrap()
}

#[test]
fn checked_english_styles_preserve_typed_source_counts_unicode_and_actions() {
    let brief = producer("spoken/todo-brief-template");
    let expanded = producer("spoken/todo-expanded-template");
    for (remaining, total) in [(0, 0), (1, 1), (3, 3), (3, 20), (20, 20), (0, 20)] {
        let face = fixture::face(remaining, total, true);
        let facts = TodoFaceFacts::prepare(&face).unwrap();
        let ids: Vec<String> = (0..remaining.min(3))
            .map(|i| format!("todo/item/task-{}", i + 1))
            .collect();
        let page: Vec<_> = ids.iter().map(String::as_str).collect();
        let a = brief.evaluate(&face, &facts, &page, "en").unwrap();
        let b = expanded.evaluate(&face, &facts, &page, "en").unwrap();
        assert_eq!(a.version(), 1);
        assert_eq!(a.locale(), "en");
        assert_eq!(a.source(), TODO_SPOKEN_TEMPLATE_SOURCE);
        assert_eq!(a.source_face(), face.identity.as_str());
        assert_eq!(a.revision(), face.revision);
        assert_eq!(a.remaining(), remaining as u64);
        assert_eq!(a.completed(), (total - remaining) as u64);
        assert_eq!(a.actions(), b.actions());
        assert_eq!(a.remaining(), b.remaining());
        assert_ne!(a.plan(), b.plan());
        assert!(a.wording().contains(&format!(
            "{remaining} {} left.",
            if remaining == 1 { "thing" } else { "things" }
        )));
        assert!(b
            .wording()
            .contains(&format!("{} completed.", total - remaining)));
        assert_ne!(a.wording(), b.wording());
        if remaining > 0 {
            assert!(a.wording().contains("Crème brûlée — 茶 🍵?"));
            assert!(b.wording().contains("Crème brûlée — 茶 🍵?"));
            assert_ne!(a.wording(), b.wording());
            assert_eq!(a.actions().len(), remaining.min(3));
        }
        assert!(matches!(
            brief.evaluate(&face, &facts, &page, "fr"),
            Err(TodoTemplateRefusal::Locale)
        ));
    }
}
#[test]
fn warnings_survive_both_styles_and_stale_or_hidden_fields_refuse() {
    let brief = producer("spoken/todo-brief-template");
    let expanded = producer("spoken/todo-expanded-template");
    let face = fixture::face(1, 1, false);
    let facts = TodoFaceFacts::prepare(&face).unwrap();
    for template in [&brief, &expanded] {
        let words = template
            .evaluate(&face, &facts, &["todo/item/task-1"], "en")
            .unwrap();
        assert!(words.actions().is_empty());
        assert!(words.wording().contains("no admitted action return route"));
    }
    let mut next = face.clone();
    next.revision += 1;
    let next = fixture::rehash(next);
    assert!(matches!(
        brief.evaluate(&next, &facts, &[], "en"),
        Err(TodoTemplateRefusal::Facts(_))
    ));
    let mut hidden = fixture::face(1, 1, true);
    hidden
        .disclosures
        .iter_mut()
        .find(|d| d.subject == "todo/item/task-1")
        .unwrap()
        .level = conduit_face::FaceDisclosureLevel::SelectedDetail;
    let hidden = fixture::rehash(hidden);
    let facts = TodoFaceFacts::prepare(&hidden).unwrap();
    assert!(matches!(
        brief.evaluate(&hidden, &facts, &["todo/item/task-1"], "en"),
        Err(TodoTemplateRefusal::Facts(_))
    ));
}

#[test]
fn every_admitted_count_uses_exact_checked_english_numerals() {
    let template = producer("spoken/todo-brief-template");
    for n in 0..=20 {
        let face = fixture::face(n, 20, true);
        let facts = TodoFaceFacts::prepare(&face).unwrap();
        let words = template.evaluate(&face, &facts, &[], "en").unwrap();
        assert!(words.wording().contains(&format!(
            "{n} {} left.",
            if n == 1 { "thing" } else { "things" }
        )));
        assert!(words.wording().contains(&format!("{} completed.", 20 - n)));
        assert_eq!(words.remaining(), n as u64);
        assert_eq!(words.completed(), (20 - n) as u64);
    }
}

#[test]
fn an_essential_warning_is_preserved_or_refused_when_oversized() {
    let template = producer("spoken/todo-brief-template");
    let mut value = fixture::face(1, 1, true);
    value.subjects.push(conduit_face::FaceSubject {
        identity: "warning/source".into(),
        role: conduit_face::FaceRole::Status,
        name: "Warning".into(),
    });
    value.text.push(conduit_face::FaceText {
        subject: "warning/source".into(),
        text: "Do not submit this item twice.".into(),
    });
    value.disclosures.push(conduit_face::FaceDisclosure {
        subject: "warning/source".into(),
        level: conduit_face::FaceDisclosureLevel::Primary,
    });
    let value = fixture::rehash(value);
    let facts = TodoFaceFacts::prepare(&value).unwrap();
    let words = template.evaluate(&value, &facts, &[], "en").unwrap();
    assert!(words.wording().contains("Do not submit this item twice."));
    let mut large = value.clone();
    large
        .text
        .iter_mut()
        .find(|t| t.subject == "warning/source")
        .unwrap()
        .text = "w".repeat(800);
    large.text.push(conduit_face::FaceText {
        subject: "warning/source".into(),
        text: "x".repeat(800),
    });
    large.text.push(conduit_face::FaceText {
        subject: "warning/source".into(),
        text: "y".repeat(800),
    });
    let large = fixture::rehash(large);
    let facts = TodoFaceFacts::prepare(&large).unwrap();
    assert!(matches!(
        template.evaluate(&large, &facts, &[], "en"),
        Err(TodoTemplateRefusal::Bound)
    ));
}

#[test]
#[ignore = "requires retained actual owner Face through CONDUIT_TODO_TEMPLATE_FACE"]
fn actual_owner_face_binds_checked_todo_wording() {
    let path = std::env::var("CONDUIT_TODO_TEMPLATE_FACE").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let face: conduit_face::Face = serde_json::from_value(value["face"].clone()).unwrap();
    let facts = TodoFaceFacts::prepare(&face).unwrap();
    let ids: Vec<_> = face
        .subjects
        .iter()
        .filter(|s| s.identity.starts_with("todo/item/"))
        .take(3)
        .map(|s| s.identity.as_str())
        .collect();
    for name in [
        "spoken/todo-brief-template",
        "spoken/todo-expanded-template",
    ] {
        let wording = producer(name).evaluate(&face, &facts, &ids, "en").unwrap();
        assert_eq!(wording.source_face(), face.identity.as_str());
        assert_eq!(wording.revision(), face.revision);
        assert_eq!(wording.remaining(), facts.remaining());
        assert_eq!(wording.completed(), facts.completed());
        assert!(wording.wording().contains("Crème brûlée — 茶 🍵?"));
        println!("{name}: {}", wording.wording());
    }
}
