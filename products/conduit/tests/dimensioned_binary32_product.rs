//! Actual CLI consumes exact physical data and inspects certified binary32 loss.
use conduit_core::{Observation, ObservationKind, Plan, TerminalDisposition};
use std::process::Command;
const SOURCE: &str = include_str!("fixtures/dimensioned_binary32_chain.conduit");
#[test]
fn actual_cli_projects_the_dimensioned_chain_and_inspects_its_certified_loss() {
    assert_chain(SOURCE);
    assert_chain(&SOURCE.replace("90°", "0.25turn"));
}
fn assert_chain(source: &str) {
    let path = std::env::temp_dir().join(format!(
        "conduit-dimensioned-binary32-{}",
        std::process::id()
    ));
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
            .contains("Dimensioned binary32: certified physical projection"));
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
        let exact: Vec<_> = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .filter(|p| {
                p.execution_profile_id.as_str() == "std/pure-expression-quantity-exact-hosted@1"
            })
            .filter(|p| {
                p.inputs[0].value_kind.as_str() == conduit_core::ANGLE_INFO_ID
                    && p.outputs[0].value_kind.as_str() == conduit_core::ANGLE_INFO_ID
            })
            .collect();
        assert_eq!(
            exact.len(),
            2,
            "both Angle transforms must select exact arithmetic"
        );
        assert!(exact.iter().all(|p| p.source_span.is_some()));
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
                        && p.outputs[0].value_kind.as_str() == conduit_core::QUANTITY_INFO_ID
                )
                .count(),
            1
        );
        for placement in &mathematical {
            assert!(placement.source_span.is_some());
        }
        let qualified = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .filter(|p| {
                p.execution_profile_id.as_str() == "std/pure-expression-near-contract-hosted@1"
            })
            .collect::<Vec<_>>();
        assert_eq!(
            qualified.len(),
            1,
            "authored tolerance must select its prepared consumer"
        );
        assert!(qualified[0].source_span.is_some());
        assert!(qualified[0]
            .semantic_contract
            .laws
            .iter()
            .any(|law| matches!(law, conduit_core::KindSemanticLaw::ValueContracts(_))));
        let boundary = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|p| {
                p.execution_profile_id.as_str() == "std/quantity-near-enclosed-midpoint-binary32@1"
            })
            .expect("selected input-consuming projection");
        assert!(boundary.source_span.is_some());
        assert_eq!(
            boundary.kind_contract_revision.as_str(),
            "numeric/quantity-near@2"
        );
        let active_play: conduit_core::ActivePlayId =
            serde_json::from_value(report["plays"][0]["active_play_id"].clone()).unwrap();
        assert_eq!(
            report["plays"][0]["plan_id"],
            serde_json::to_value(&plan.plan_id).unwrap()
        );
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
            .all(|s| s.plan_id.as_ref() == Some(&plan.plan_id)
                && s.active_play_id.as_ref() == Some(&active_play)));
        println!(
            "DIMENSIONED_BINARY32_PRODUCT source={} plan={} selected_mathematical={}",
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

#[test]
fn actual_cli_refuses_zero_allowance_for_the_symbolic_physical_projection() {
    let path = std::env::temp_dir().join(format!(
        "conduit-dimensioned-binary32-refused-{}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    let result = || {
        std::fs::write(
            path.join("main.conduit"),
            SOURCE.replace("limit=0.000001rad", "limit=0rad"),
        )
        .unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("run")
            .arg(path.join("main.conduit"))
            .arg("--await-terminal")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("certified physical projection"));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains("InvalidInput"), "{error}");
        assert!(error.contains("detail: 3"), "{error}");
    };
    let caught = std::panic::catch_unwind(result);
    std::fs::remove_dir_all(path).unwrap();
    if let Err(panic) = caught {
        std::panic::resume_unwind(panic);
    }
}
