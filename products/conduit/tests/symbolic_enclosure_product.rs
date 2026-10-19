//! Actual product CLI crosses symbolic coordinate construction and a dynamic consumer.
use conduit_core::{Observation, ObservationKind, Plan, TerminalDisposition};
use std::{path::PathBuf, process::Command};
const SOURCE: &str = include_str!("fixtures/numerical_symbolic_enclosure.conduit");
const ROOT_SOURCE: &str = include_str!("fixtures/numerical_root_enclosure.conduit");
const COMPOSED_SOURCE: &str =
    include_str!("fixtures/numerical_composed_symbolic_enclosure.conduit");
struct Files(PathBuf);
impl Files {
    fn new(name: &str, source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "conduit-symbolic-enclosure-{}-{name}",
            std::process::id()
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
#[test]
fn checked_symbolic_coordinate_reaches_the_selected_input_consumer_in_the_actual_cli() {
    assert_admitted("pi", SOURCE, "Certified Pi over 180");
    assert_admitted("root", ROOT_SOURCE, "Certified square root of two");
    assert_admitted(
        "composed",
        COMPOSED_SOURCE,
        "Certified composed symbolic value",
    );
}
fn assert_admitted(name: &str, source: &str, label: &str) {
    let files = Files::new(name, source);
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
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout)
        .contains(&format!("{label}: certified dynamic enclosure")));
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
    assert!(plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .any(|p| p.kind_id.as_str() == "math/enclose"
            && p.implementation_id.as_str() == "conduit.std/mathematical-enclosure@1"
            && p.execution_profile_id.as_str() == "std/borrowed-exact-interval@1"));
    if source == COMPOSED_SOURCE {
        let stages: Vec<_> = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .filter(|p| {
                p.kind_id
                    .as_str()
                    .starts_with("conduitese/pure-expression/")
                    && p.inputs.len() == 1
                    && p.outputs.len() == 1
                    && p.inputs[0].value_kind.as_str() == conduit_core::MATHEMATICAL_VALUE_ID
                    && p.outputs[0].value_kind.as_str() == conduit_core::MATHEMATICAL_VALUE_ID
            })
            .collect();
        assert_eq!(stages.len(), 3);
        for stage in stages {
            assert_eq!(
                stage.implementation_id.as_str(),
                "std/kernel-pure-expression@1"
            );
            assert!(stage.source_span.is_some());
            println!(
                "SYMBOLIC_EXACT_STAGE gear={} kind={} profile={} span={:?}",
                stage.gear_id.as_str(),
                stage.kind_id.as_str(),
                stage.execution_profile_id.as_str(),
                stage.source_span
            );
        }
    }
    let signs: Vec<Observation> = serde_json::from_value(report["observations"].clone()).unwrap();
    assert!(matches!(
        signs.last().map(|s| &s.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert!(signs
        .iter()
        .all(|s| s.plan_id.as_ref() == Some(&plan.plan_id) && s.active_play_id.is_some()));
    println!(
        "SYMBOLIC_ENCLOSURE_PRODUCT source={} plan={}",
        plan.source_document_id.as_str(),
        plan.plan_id.as_str()
    );
}
#[test]
fn actual_cli_refuses_zero_width_symbolic_projection_without_success_output() {
    assert_refused("pi-refused", SOURCE);
    assert_refused("root-refused", ROOT_SOURCE);
    assert_refused("composed-refused", COMPOSED_SOURCE);
}
fn assert_refused(name: &str, source: &str) {
    let source = source.replace("maximum-width:0.00000000000000001", "maximum-width:0");
    let files = Files::new(name, &source);
    let result = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("run")
        .arg(files.0.join("main.conduit"))
        .arg("--await-terminal")
        .output()
        .unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("BackFailed(Failure { code: InvalidInput, detail: 4 })"),
        "{stderr}"
    );
    assert!(!String::from_utf8_lossy(&result.stdout).contains("certified dynamic enclosure"));
    println!(
        "SYMBOLIC_ENCLOSURE_REFUSAL stderr={}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn actual_cli_refuses_an_unsupported_symbolic_denominator_without_success_output() {
    let source = COMPOSED_SOURCE.replace("((. * 3) / 2)", "((. * 3) / math/sqrt(2))");
    assert_ne!(source, COMPOSED_SOURCE);
    let files = Files::new("symbolic-denominator-refused", &source);
    let result = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("run")
        .arg(files.0.join("main.conduit"))
        .arg("--await-terminal")
        .output()
        .unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("BackFailed(Failure { code: HostCallFailed, detail: 38 })"),
        "{stderr}"
    );
    assert!(!String::from_utf8_lossy(&result.stdout).contains("certified dynamic enclosure"));
    println!("SYMBOLIC_DENOMINATOR_REFUSAL stderr={stderr}");
}
