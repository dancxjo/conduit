use conduit_core::{ObservationKind, TerminalDisposition};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{StdHost, ThreadTimer};

const GREET_SOURCE: &str = include_str!("../../../forms/greet/main.conduit");
const EVIDENCE_MARKER: &str = "CONDUIT_FORM_EVIDENCE=";

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

#[test]
fn reusable_greet_runs_twice_with_exact_occurrences_in_one_kernel_play() {
    let (startup, profile) = catalogs();
    let canonical = check_syntax_document(&parse_syntax_document(GREET_SOURCE), &startup).unwrap();
    let canonical_greet_id = canonical
        .forms
        .iter()
        .find(|form| form.name == "greet")
        .unwrap()
        .checked_form_id
        .clone();
    let proof_source = format!(
        "{GREET_SOURCE}\nform greet-double-driver {{\n    first: greet(\"Hi \")\n    second: greet(\"Bye \")\n    \"Ada\" > first > presentation/text\n    \"Bob\" > second > presentation/text\n}}\n"
    );
    let checked = check_syntax_document(&parse_syntax_document(&proof_source), &startup).unwrap();
    assert_eq!(
        checked
            .forms
            .iter()
            .find(|form| form.name == "greet")
            .unwrap()
            .checked_form_id,
        canonical_greet_id
    );

    let expanded = expand_canonical_form(&checked, "greet-double-driver", &profile).unwrap();
    let occurrences = expanded
        .provenance
        .iter()
        .filter(|row| row.source_form == "greet" && row.source_gear == "join")
        .map(|row| row.form_path.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        occurrences,
        [
            vec!["greet-double-driver".to_string(), "first".to_string()],
            vec!["greet-double-driver".to_string(), "second".to_string()],
        ]
    );

    let mut host = StdHost::new();
    let plan = host.plan_expanded_local(&expanded).unwrap();
    assert_eq!(plan.fragments.len(), 1);
    let plan_id = plan.plan_id.clone();
    let mut output = Vec::with_capacity(4_096);
    let report = host
        .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut ThreadTimer)
        .unwrap();
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
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("Hi Ada"), "{output}");
    assert!(output.contains("Bye Bob"), "{output}");
    println!(
        "{EVIDENCE_MARKER}{{\"plan_id\":\"{}\",\"play_id\":\"{}\"}}",
        plan_id.as_str(),
        kernel.active_play_id.as_str()
    );
}
