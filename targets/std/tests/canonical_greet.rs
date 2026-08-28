use conduit_core::{ConfigurationValue, ObservationKind, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, CheckedSyntaxDocument,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{StdHost, ThreadTimer};

const GREET_PROGRAM: &str = include_str!("../../../examples/greet.conduit");

fn checked_and_profile() -> (CheckedSyntaxDocument, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(GREET_PROGRAM);
    assert_eq!(syntax.round_trip(), GREET_PROGRAM);
    let checked = check_syntax_document(&syntax, &startup).expect("greet source checks");
    (checked, profile)
}

fn run(
    root: &str,
) -> (
    conduit_form::ExpandedCanonicalForm,
    String,
    conduit_core::PlanId,
) {
    let (checked, profile) = checked_and_profile();
    let expanded = expand_canonical_form(&checked, root, &profile).expect("greet expands");
    let mut host = StdHost::new();
    let plan = host
        .plan_expanded_local(&expanded)
        .expect("greet plans onto ordinary installed leaves");
    let plan_id = plan.plan_id.clone();
    let mut output = Vec::with_capacity(4_096);
    let mut timer = ThreadTimer;
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect("greet executes through the installed kernel table");
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
    (
        expanded,
        String::from_utf8(output).expect("operator output is UTF-8"),
        plan_id,
    )
}

#[test]
fn explicit_positional_binding_recursively_executes_only_primitive_leaves() {
    let (checked, _) = checked_and_profile();
    let welcome = checked
        .forms
        .iter()
        .find(|form| form.name == "welcome")
        .unwrap();
    assert!(welcome.gears.iter().any(|gear| gear.kind == "greet"
        && gear.startup_bindings[0].value
            == conduit_form::CanonicalStartupValue::Literal("\"Welcome\"".to_string())));

    let (expanded, output, _) = run("welcome");
    assert!(output.contains("WelcomeTravis\n"), "{output}");
    assert_eq!(expanded.gears.len(), 3);
    assert!(!expanded
        .gears
        .iter()
        .any(|operation| operation.kind_id.as_str() == "greet"));
    let join = expanded
        .gears
        .iter()
        .find(|operation| operation.kind_id.as_str() == "text/join")
        .expect("expanded back has one join leaf");
    assert_eq!(
        join.configuration,
        [conduit_core::ConfigurationEntry {
            key: "prefix".to_string(),
            value: ConfigurationValue::Text("Welcome".to_string()),
        }]
    );
    assert!(expanded.provenance.iter().any(|row| {
        row.source_form == "greet"
            && row.form_path == ["welcome", "hello"]
            && row.source_gear == "join"
    }));
}

#[test]
fn omitted_argument_uses_the_checked_face_default_without_mutating_the_form() {
    let (explicit, explicit_output, explicit_plan) = run("welcome");
    let (defaulted, default_output, default_plan) = run("default-welcome");
    assert!(explicit_output.contains("WelcomeTravis\n"));
    assert!(default_output.contains("HelloTravis\n"));
    assert_ne!(explicit.expanded_form_id, defaulted.expanded_form_id);
    assert_ne!(explicit_plan, default_plan);
}

#[test]
fn join_output_bound_and_selected_realization_identity_fail_before_presentation() {
    let oversized_prefix = "x".repeat(conduit_text::MAX_TEXT_BYTES as usize);
    let source = format!(
        "form bad {{\n    join: text/join(\"{oversized_prefix}\")\n    \"y\" > join > presentation/text\n}}\n"
    );
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "bad", &profile).unwrap();
    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    let mut output = Vec::with_capacity(4_096);
    let mut timer = ThreadTimer;
    let error = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
        .expect_err("combined output beyond 256 bytes must fail");
    assert!(error.contains("output exceeds"), "{error}");
    assert!(!String::from_utf8_lossy(&output).contains(&oversized_prefix));

    let (checked, profile) = checked_and_profile();
    let expanded = expand_canonical_form(&checked, "welcome", &profile).unwrap();
    let mut host = StdHost::new();
    let mut mutated = host.plan_expanded_local(&expanded).unwrap();
    let join = mutated.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == "text/join")
        .unwrap();
    join.host_operations[0].contract_id =
        conduit_core::HostOperationContractId::from("wrong/text-join@1");
    let mut output = Vec::with_capacity(4_096);
    assert!(host
        .run_fragment_to(mutated.fragments.remove(0), &mut output, &mut timer)
        .is_err());
    assert!(!String::from_utf8_lossy(&output).contains("WelcomeTravis"));
}
