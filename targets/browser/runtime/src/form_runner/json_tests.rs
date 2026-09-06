//! Native conformance of the browser envelope using the actual authored Todo.
use super::*;
use crate::installed_browser::test_json;
use conduit_form::{KindDefinition, KindSignature};
use std::collections::BTreeMap;
const TODO: &str = include_str!("../../../../../forms/todo/main.conduit");

pub(super) fn execute(input: &str, entry: &str) -> Result<String, String> {
    let (mut startup, mut profile) = crate::installed_browser::catalogs().unwrap();
    let mut host =
        crate::installed_browser::advertisement("todo-browser".into(), "todo-boot".into());
    for offer in [test_json::source_offer(), test_json::sink_offer()] {
        startup
            .insert(KindSignature {
                kind: offer.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile
            .insert(KindDefinition {
                kind_id: offer.kind_id.clone(),
                kind_contract_revision: offer.kind_contract_revision.clone(),
                inputs: offer.inputs.clone(),
                outputs: offer.outputs.clone(),
                configuration: Vec::new(),
            })
            .unwrap();
        host.capabilities.push(offer);
    }
    let wiring = if entry == "todo/restore-summary" {
        "source.value > application.snapshot\n application.result > sink.value"
    } else {
        "decode: json/decode\n source.value > decode.value\n decode.value > application.request\n application.snapshot > sink.value"
    };
    let source = format!("{TODO}\nform browser-todo-fixture {{\n source: conduit-test/json-source\n application: {entry}\n sink: conduit-test/json-sink\n {wiring}\n}}");
    let syntax = conduit_form::parse_syntax_document(&source);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "browser-todo-fixture", &profile).unwrap();
    assert!(expanded
        .provenance
        .iter()
        .any(|row| row.source_form == "todo/snapshot" && row.form_path.len() == 3));
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let fragment = &plan.fragments[0];
    let lowered = lower_plan_fragment(fragment).unwrap();
    validate_envelope(fragment, &lowered, false).unwrap();
    test_json::input(input.as_bytes());
    let mut scheduler = prepare_scheduler(fragment, &lowered).unwrap();
    let capacity = scheduler.values().allocation_capacities();
    let result = drive(&mut scheduler, fragment);
    assert_eq!(scheduler.values().allocation_capacities(), capacity);
    let DriveStatus::Effect(pending) = result? else {
        panic!("no output fixture effect")
    };
    let BrowserHostEffect::Manifestation(ref output) = pending.effect else {
        panic!("wrong output effect")
    };
    let value = String::from_utf8(output.canonical_value.clone()).unwrap();
    complete_host_effect(&mut scheduler, &pending).unwrap();
    assert!(matches!(
        drive(&mut scheduler, fragment).unwrap(),
        DriveStatus::Complete
    ));
    Ok(value)
}

#[test]
fn browser_todo_composed_edits_summary_and_restore_use_the_production_kernel() {
    let mut state = "[]".to_string();
    for (command, expected, counts) in [
        (
            r#"{"op":"append","value":{"complete":false,"text":"Milk"}}"#,
            r#"[{"complete":false,"text":"Milk"}]"#,
            r#"{"false":1,"total":1,"true":0}"#,
        ),
        (
            r#"{"field":"complete","index":0,"op":"toggle"}"#,
            r#"[{"complete":true,"text":"Milk"}]"#,
            r#"{"false":0,"total":1,"true":1}"#,
        ),
        (
            r#"{"index":0,"op":"remove"}"#,
            "[]",
            r#"{"false":0,"total":0,"true":0}"#,
        ),
    ] {
        let request = format!("{{\"collection\":{state},\"command\":{command}}}");
        state = execute(&request, "todo/command-snapshot").unwrap();
        assert_eq!(state, expected);
        assert_eq!(execute(&state, "todo/restore-summary").unwrap(), counts);
        assert_eq!(execute(&request, "todo/command-summary").unwrap(), counts);
    }
}

#[test]
fn browser_todo_refusals_preserve_kernel_failure_details() {
    assert_eq!(
        execute(
            r#"{"collection":[],"command":{"index":0,"op":"remove"}}"#,
            "todo/command-snapshot"
        )
        .unwrap_err(),
        "OperationFailed(Failure { code: InvalidInput, detail: 105 })"
    );
    assert_eq!(
        execute(r#"[{"text":"missing"}]"#, "todo/restore-summary").unwrap_err(),
        "OperationFailed(Failure { code: InvalidInput, detail: 123 })"
    );
    assert_eq!(
        execute(r#"[{"complete":0}]"#, "todo/restore-summary").unwrap_err(),
        "OperationFailed(Failure { code: InvalidInput, detail: 124 })"
    );
}

#[test]
fn browser_todo_snapshot_record_restores_through_the_same_authored_form() {
    use crate::resource_snapshot::{tests::placement, PreparedSnapshotRecord};
    let snapshot = execute(
        r#"{"collection":[],"command":{"op":"append","value":{"complete":false,"text":"Milk"}}}"#,
        "todo/command-snapshot",
    )
    .unwrap();
    let (write, reference) = placement("boot/one", true, snapshot.len());
    let mut writer = PreparedSnapshotRecord::prepare(&write, &reference).unwrap();
    let record = writer
        .publication(&write.authority[0], snapshot.as_bytes())
        .unwrap()
        .to_vec();
    let (read, _) = placement("boot/two", false, snapshot.len());
    let reader = PreparedSnapshotRecord::prepare(&read, &reference).unwrap();
    let restored = reader.restore(&read.authority[0], &record).unwrap();
    assert_eq!(restored, snapshot.as_bytes());
    assert_eq!(
        execute(
            std::str::from_utf8(restored).unwrap(),
            "todo/restore-summary"
        )
        .unwrap(),
        r#"{"false":1,"total":1,"true":0}"#
    );
}
