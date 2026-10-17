//! Numerical meaning through the actual default product Host and CLI entrance.
use conduit_core::{Observation, ObservationKind, Plan, TerminalDisposition};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Files(PathBuf);
impl Files {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "conduit-numerical-expression-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("main.conduit"), source).unwrap();
        Self(path)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn run(source: &str, label: &str, expected: &str) -> Plan {
    let files = Files::new(source);
    let result = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("run")
        .arg(files.0.join("main.conduit"))
        .arg("--await-terminal")
        .arg("--report")
        .arg(files.0.join("report.json"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(
        stdout.contains(&format!("\n{label}: {expected}\n")),
        "{stdout}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(files.0.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["plans"].as_array().unwrap().len(), 1);
    assert_eq!(report["plays"].as_array().unwrap().len(), 1);
    let plan: Plan = serde_json::from_value(report["plans"][0].clone()).unwrap();
    assert!(conduit_core::verify_plan(&plan));
    assert_eq!(
        plan.source_document_id,
        conduit_plot::parse_syntax_document(source).source_document_id()
    );
    let observations: Vec<Observation> =
        serde_json::from_value(report["observations"].clone()).unwrap();
    assert!(matches!(
        observations.last().map(|o| &o.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert!(observations
        .iter()
        .all(|o| o.plan_id.as_ref() == Some(&plan.plan_id) && o.active_play_id.is_some()));
    assert!(plan.fragments.iter().flat_map(|f| &f.placements).any(|g| g
        .kind_id
        .as_str()
        .starts_with("conduitese/pure-expression/")
        && g.implementation_id.as_str() == "std/kernel-pure-expression@1"));
    println!(
        "source={} checked={} expanded={} plan={} play={}",
        plan.source_document_id.as_str(),
        plan.checked_plot_id.as_str(),
        plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str(),
        report["plays"][0]["active_play_id"]
    );
    plan
}

#[test]
fn ordinary_exact_tenths_keep_meaning_across_new_plans_without_float_coercion() {
    let tenths = include_str!("fixtures/numerical_exact_tenths.conduit");
    let first = run(tenths, "Exact tenths", "exact");
    let replanned = run(tenths, "Exact tenths", "exact");
    assert_eq!(first.source_document_id, replanned.source_document_id);
    assert_eq!(first.checked_plot_id, replanned.checked_plot_id);
    assert_eq!(first.expanded_plot_id, replanned.expanded_plot_id);
}

#[test]
fn ordinary_thirds_sum_exactly_through_the_default_product_host() {
    run(
        include_str!("fixtures/numerical_exact_thirds.conduit"),
        "Exact thirds",
        "exact",
    );
}

#[test]
fn a_large_exact_expression_never_wraps_or_silently_rounds_to_a_machine_integer() {
    let original = format!("1{}", "0".repeat(1000));
    let successor = format!("1{}1", "0".repeat(999));
    let source = format!("plot increment (\n >> value: MathematicalValue\n result: Text >>\n) = ((. + 1 == {successor}) ? \"exact\" : \"wrong\")\n\nplot main {{\n seed: math/realize(request={{value:{original},maximum-error:0}})\n check: increment\n shown: face/text(\"Large exact expression\")\n seed.result >> (.source) >> check.value\n check.result >> shown\n}}.\n");
    let plan = run(&source, "Large exact expression", "exact");
    let source = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|g| g.kind_id.as_str() == "math/realize")
        .unwrap();
    assert_eq!(
        source.implementation_id.as_str(),
        "conduit.std/numeric-rational@1"
    );
}

#[test]
fn explicit_f32_cancellation_keeps_intermediate_rounding_in_the_product() {
    run(
        include_str!("fixtures/numerical_f32_cancellation.conduit"),
        "IEEE cancellation",
        "rounded intermediate",
    );
}
