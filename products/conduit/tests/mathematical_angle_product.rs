//! Ordinary CLI preparation selects exact Angle transport through typed trig.
use conduit_core::{Observation, ObservationKind, Plan, TerminalDisposition};
use std::process::Command;
const SOURCE: &str = include_str!("fixtures/mathematical_angle_chain.conduit");
#[test]
fn actual_cli_selects_the_symbolic_angle_chain_and_completes_with_exact_sine() {
    assert_chain(SOURCE);
    assert_chain(&SOURCE.replace("90°", "0.25turn"));
}
fn assert_chain(source: &str) {
    let path =
        std::env::temp_dir().join(format!("conduit-mathematical-angle-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let result = || {
        std::fs::write(path.join("main.conduit"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("run")
            .arg(path.join("main.conduit"))
            .arg("--await-terminal")
            .arg("--report")
            .arg(path.join("report.json"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout)
            .contains("Typed mathematical Angle: exact sine one"));
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["plans"].as_array().unwrap().len(), 1);
        assert_eq!(report["plays"].as_array().unwrap().len(), 1);
        let plan: Plan = serde_json::from_value(report["plans"][0].clone()).unwrap();
        assert!(conduit_core::verify_plan(&plan));
        assert_eq!(
            plan.source_document_id,
            conduit_plot::parse_syntax_document(source).source_document_id()
        );
        let mathematical: Vec<_> = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .filter(|p| {
                p.execution_profile_id.as_str() == "std/pure-expression-mathematical-angle-hosted@1"
            })
            .collect();
        assert_eq!(mathematical.len(), 3);
        assert_eq!(
            mathematical
                .iter()
                .filter(
                    |p| p.inputs[0].value_kind.as_str() == conduit_core::ANGLE_INFO_ID
                        && p.outputs[0].value_kind.as_str() == conduit_core::ANGLE_INFO_ID
                )
                .count(),
            2
        );
        assert_eq!(
            mathematical
                .iter()
                .filter(
                    |p| p.inputs[0].value_kind.as_str() == conduit_core::ANGLE_INFO_ID
                        && p.outputs[0].value_kind.as_str() == conduit_core::MATHEMATICAL_VALUE_ID
                )
                .count(),
            1
        );
        for placement in &mathematical {
            assert!(placement.source_span.is_some());
        }
        let signs: Vec<Observation> =
            serde_json::from_value(report["observations"].clone()).unwrap();
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
            "MATHEMATICAL_ANGLE_PRODUCT source={} plan={} selected_mathematical={}",
            plan.source_document_id.as_str(),
            plan.plan_id.as_str(),
            mathematical.len()
        );
    };
    let caught = std::panic::catch_unwind(result);
    std::fs::remove_dir_all(path).unwrap();
    if let Err(panic) = caught {
        std::panic::resume_unwind(panic);
    }
}
