//! Authored intrinsic differences through the actual default CLI and kernel.
use conduit_core::{Observation, ObservationKind, Plan, TerminalDisposition};
use std::{path::PathBuf, process::Command};

const SOURCE: &str = include_str!("fixtures/atan2_temperature_differences.conduit");
struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn authored_fahrenheit_and_kelvin_differences_reach_the_exact_turn_bearing() {
    let files = Files(
        std::env::temp_dir().join(format!("conduit-atan2-difference-{}", std::process::id())),
    );
    std::fs::create_dir(&files.0).unwrap();
    let source = files.0.join("main.conduit");
    let report = files.0.join("report.json");
    std::fs::write(&source, SOURCE).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("run")
        .arg(source)
        .arg("--await-terminal")
        .arg("--report")
        .arg(&report)
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
        stdout.contains("\nDifference bearing: exact difference bearing\n"),
        "{stdout}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(report["plans"].as_array().unwrap().len(), 1);
    assert_eq!(report["plays"].as_array().unwrap().len(), 1);
    let plan: Plan = serde_json::from_value(report["plans"][0].clone()).unwrap();
    assert!(conduit_core::verify_plan(&plan));
    assert_eq!(
        plan.source_document_id,
        conduit_plot::parse_syntax_document(SOURCE).source_document_id()
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
    assert!(plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .any(|g| g.kind_id.as_str() == "units/compare-temperature-differences"));
    assert!(plan.fragments.iter().flat_map(|f| &f.placements).any(|g| {
        g.implementation_id.as_str() == "std/kernel-pure-expression@1"
            && g.configuration.iter().any(|entry| {
                let conduit_core::ConfigurationValue::Text(encoded) = &entry.value else {
                    return false;
                };
                conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).is_ok_and(
                    |p| {
                        matches!(p.root.operation,
                        conduit_plot::PortableExpressionOperation::SemanticCall { kind, .. }
                            if kind == "math/atan2")
                    },
                )
            })
    }));
    println!(
        "ATAN2_DIFFERENCE_CLI source={} checked={} expanded={} plan={} play={}\n{stdout}",
        plan.source_document_id.as_str(),
        plan.checked_plot_id.as_str(),
        plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str(),
        report["plays"][0]["active_play_id"]
    );
}
