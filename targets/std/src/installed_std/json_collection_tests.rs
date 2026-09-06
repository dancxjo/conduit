use super::super::test_json_codec::with_source_text;
use crate::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};
use conduit_core::{BootId, HostId, ObservationKind, OfferGeneration, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindSignature,
    ProfileCatalog, StartupCatalog,
};

const TODO: &str = include_str!("../../../../forms/todo/main.conduit");

fn execute(request: &str) -> (String, Result<crate::StdRunReport, String>) {
    execute_entry(request, "todo/command-snapshot")
}

pub(super) fn execute_entry(
    request: &str,
    entry: &str,
) -> (String, Result<crate::StdRunReport, String>) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_web::install_json_catalogs(&mut startup, &mut profile).unwrap();
    super::super::test_json_codec::install_catalog(&mut profile);
    for kind in [
        "conduit-test/json-text-source",
        "conduit-test/json-text-sink",
    ] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
    }
    let restoring = entry == "todo/restore-summary";
    let wiring = if restoring {
        "source.value > application.snapshot\n application.result > sink.value"
    } else {
        "decode: json/decode\n source.value > decode.value\n decode.value > application.request\n application.snapshot > sink.value"
    };
    let source = format!("{TODO}\nform todo-fixture {{\n source: conduit-test/json-text-source\n application: {entry}\n sink: conduit-test/json-text-sink\n {wiring}\n}}\n");
    let parsed = parse_syntax_document(&source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let checked = check_syntax_document(&parsed, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "todo-fixture", &profile).unwrap();
    assert!(expanded.provenance.iter().any(|row| row.source_form
        == if restoring {
            "todo/restore"
        } else {
            "todo/state-step"
        }
        && row.form_path.len() == 3));
    assert!(expanded
        .provenance
        .iter()
        .any(|row| row.source_form == "todo/snapshot" && row.form_path.len() == 3));
    let mut host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("todo-fixture-host"),
            boot_id: BootId::from("todo-fixture-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal()
            .with_text()
            .with_json()
            .with_json_collection(),
    );
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let mut output = Vec::new();
    let report = with_source_text(request.as_bytes(), || {
        host.run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)
    });
    (String::from_utf8(output).unwrap(), report)
}

#[test]
fn todo_add_toggle_remove_execute_composed_forms_through_production_kernel() {
    let cases = [
        (
            r#"{"op":"append","value":{"complete":false,"text":"Buy milk"}}"#,
            r#"[{"complete":false,"text":"Buy milk"}]"#,
        ),
        (
            r#"{"field":"complete","index":0,"op":"toggle"}"#,
            r#"[{"complete":true,"text":"Buy milk"}]"#,
        ),
        (r#"{"index":0,"op":"remove"}"#, "[]"),
    ];
    let mut state = "[]".to_string();
    for (command, expected) in cases {
        let request = format!("{{\"collection\":{state},\"command\":{command}}}");
        let (output, report) = execute(&request);
        let report = report.unwrap();
        state = output
            .lines()
            .find(|line| line.starts_with('['))
            .expect("kernel snapshot")
            .to_string();
        assert_eq!(state, expected, "{output}");
        assert!(matches!(
            report.observations.last().map(|item| &item.kind),
            Some(ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            })
        ));
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
    }
}

#[test]
fn todo_unknown_index_refuses_without_emitting_a_success_snapshot() {
    let (output, report) = execute(r#"{"collection":[],"command":{"index":0,"op":"remove"}}"#);
    assert!(!output.lines().any(|line| line == "[]"), "{output}");
    assert_eq!(
        report.unwrap_err(),
        "installed kernel step: OperationFailed(Failure { code: HostOperationFailed, detail: 105 })"
    );
}
