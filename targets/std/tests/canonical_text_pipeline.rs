use conduit_core::{ObservationKind, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, ThreadTimer};

const HELLO_PROGRAM: &str = include_str!("../../../forms/hello/main.conduit");

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .expect("the typed text pipeline catalogs are disjoint");
    let syntax = parse_syntax_document(HELLO_PROGRAM);
    assert_eq!(syntax.round_trip(), HELLO_PROGRAM);
    let checked = check_syntax_document(&syntax, &startup).expect("Program 1 checks");
    expand_canonical_form(&checked, "hello", &profile).expect("Program 1 expands")
}

#[test]
fn canonical_program_one_runs_through_the_planner_kernel_and_terminal_sign() {
    let expanded = expanded();
    assert_eq!(expanded.gears.len(), 3);
    assert_eq!(expanded.connections.len(), 2);
    let mut kinds = expanded
        .gears
        .iter()
        .map(|operation| operation.kind_id.as_str())
        .collect::<Vec<_>>();
    kinds.sort_unstable();
    assert_eq!(kinds, ["presentation/text", "text/literal", "text/upper"]);
    assert!(expanded
        .gears
        .iter()
        .find(|operation| operation.kind_id.as_str() == "text/literal")
        .is_some_and(|operation| {
            operation.configuration
                == [conduit_core::ConfigurationEntry {
                    key: "value".to_string(),
                    value: conduit_core::ConfigurationValue::Text("Hello, world.".to_string()),
                }]
        }));

    let mut host = StdHost::new();
    let plan = host
        .plan_expanded_local(&expanded)
        .expect("Program 1 plans onto exact installed offers");
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == host.advertisement().host_id)
        .expect("the local std fragment exists");
    assert_eq!(fragment.placements.len(), 3);

    let mut output = Vec::with_capacity(4_096);
    let mut timer = ThreadTimer;
    let report = host
        .run_fragment_to(fragment, &mut output, &mut timer)
        .expect("Program 1 executes through the installed kernel table");
    let output = String::from_utf8(output).expect("operator output is UTF-8");
    assert!(output.contains("HELLO, WORLD.\n"), "{output}");
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel execution report exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn text_literals_reject_invalid_escape_and_the_exact_byte_bound() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let invalid = parse_syntax_document(
        r#"form bad {
    upper: text/upper
    "bad\q" > upper
}
"#,
    );
    assert!(check_syntax_document(&invalid, &startup).is_err());

    let oversized = "x".repeat(conduit_text::MAX_TEXT_BYTES as usize + 1);
    let source = format!("form bad {{\n    upper: text/upper\n    \"{oversized}\" > upper\n}}\n");
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).expect("syntax remains lossless");
    let error = expand_canonical_form(&checked, "bad", &profile)
        .expect_err("oversized text must fail before planning");
    assert_eq!(error.code, "CND-FRM-040");
}

#[test]
fn selected_upper_realization_and_host_operation_identity_fail_closed() {
    let expanded = expanded();
    let host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let baseline = &plan.fragments[0];
    let upper = baseline
        .placements
        .iter()
        .position(|placement| placement.kind_id.as_str() == "text/upper")
        .unwrap();
    let mutations: [fn(&mut conduit_core::PlannedGear); 2] = [
        |placement| placement.artifact_id = conduit_core::ArtifactId::from("wrong/text-upper@1"),
        |placement| {
            placement.host_operations[0].target_kind =
                Some(conduit_core::KindId::from("wrong/transform"))
        },
    ];
    for mutate in mutations {
        let mut fragment = baseline.clone();
        mutate(&mut fragment.placements[upper]);
        let mut host = StdHost::new();
        let mut output = Vec::with_capacity(4_096);
        let mut timer = ThreadTimer;
        assert!(host
            .run_fragment_to(fragment, &mut output, &mut timer)
            .is_err());
        assert!(!String::from_utf8_lossy(&output).contains("HELLO, WORLD."));
    }
}
