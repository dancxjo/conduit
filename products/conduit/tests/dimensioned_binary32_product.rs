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
            .arg("--artifacts")
            .arg(path.join("artifacts"))
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
                p.execution_profile_id.as_str() == "std/quantity-near-enclosed-midpoint-binary32@3"
            })
            .expect("selected input-consuming projection");
        assert!(boundary.source_span.is_some());
        assert_eq!(
            boundary.kind_contract_revision.as_str(),
            "numeric/quantity-near@4"
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
        let routed = signs
            .iter()
            .filter_map(|sign| {
                let ObservationKind::RoutedContent { evidence } = &sign.kind else {
                    return None;
                };
                (sign.placement_id.as_ref() == Some(&boundary.placement_id)
                    && evidence.byte_length > 0)
                    .then_some((sign, evidence))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            routed.len(),
            1,
            "actual projection output must reach the product Sign report"
        );
        let (sign, evidence) = routed[0];
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| Some(&connection.connection_id) == sign.connection_id.as_ref())
            .expect("routed projection Sign names its exact planned connection");
        assert_eq!(connection.source_placement_id, boundary.placement_id);
        assert_eq!(connection.source_port_id, evidence.output_port);
        assert!(evidence.byte_length <= connection.byte_capacity);
        assert_eq!(evidence.digest_domain, "conduit/routed-value-content@1");
        assert_eq!(evidence.digest_scheme, "core/semantic-sha256@1");
        let artifact_path = path
            .join("artifacts")
            .join(format!("sign-{}.json", sign.sign_id.as_str()));
        let artifact: conduit_observatory::SignArtifact =
            serde_json::from_slice(&std::fs::read(&artifact_path).unwrap()).unwrap();
        conduit_observatory::validate_sign_artifact(&artifact).unwrap();
        assert_eq!(&artifact.sign, sign);
        assert_eq!(
            conduit_core::bind_sign(
                &artifact.identity.host_id,
                &artifact.identity.boot_id,
                artifact.identity.active_play_id.as_ref(),
                artifact.identity.sequence
            ),
            artifact.identity
        );
        let inspected = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("inspect")
            .arg(&artifact_path)
            .output()
            .unwrap();
        assert!(
            inspected.status.success(),
            "{}",
            String::from_utf8_lossy(&inspected.stderr)
        );
        assert!(String::from_utf8_lossy(&inspected.stdout).contains("RoutedContent"));
        let value_path = path
            .join("artifacts")
            .join(format!("value-{}.json", sign.sign_id.as_str()));
        let value: conduit_observatory::RoutedValueArtifact =
            serde_json::from_slice(&std::fs::read(&value_path).unwrap()).unwrap();
        conduit_observatory::validate_routed_value_artifact(&value).unwrap();
        assert_eq!(value.sign.sign, *sign);
        assert_eq!(value.bytes.len(), evidence.byte_length as usize);
        let frame = conduit_core::validate_canonical_structured_value(&value.bytes).unwrap();
        let report = frame.record_field("projection-evidence").unwrap().unwrap();
        assert_eq!(&report.value_node()[5..9], b"CPE\x01");
        let inspected = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("inspect")
            .arg(&value_path)
            .output()
            .unwrap();
        assert!(
            inspected.status.success(),
            "{}",
            String::from_utf8_lossy(&inspected.stderr)
        );
        assert!(String::from_utf8_lossy(&inspected.stdout).contains("byte correlation verified"));
        assert!(String::from_utf8_lossy(&inspected.stdout).contains("Projection report claims"));
        assert!(String::from_utf8_lossy(&inspected.stdout)
            .contains("Numerical certificate independently replayed"));
        assert!(String::from_utf8_lossy(&inspected.stdout)
            .contains("requested allowance 1/1000000 rad"));
        assert!(String::from_utf8_lossy(&inspected.stdout).contains("maximum absolute error 4989059748945264178730333232019111938743553307553489755494474841856933287739664248916564964485953/114136378384044305483399922558931182958853259523662765806564420563333741593366937500000000000000000000000 rad"));
        // Correlation is separate from report framing: a consistently rehashed
        // unknown report version still refuses ordinary inspection.
        let report_start =
            report.value_node().as_ptr() as usize - value.bytes.as_ptr() as usize + 5;
        let mut malformed_report = value.clone();
        malformed_report.bytes[report_start + 3] = 2;
        let digest = conduit_core::semantic_digest(
            "conduit/routed-value-content@1",
            &malformed_report.bytes,
        );
        if let ObservationKind::RoutedContent { evidence } = &mut malformed_report.sign.sign.kind {
            evidence.digest = digest;
        }
        conduit_observatory::validate_routed_value_artifact(&malformed_report).unwrap();
        let malformed_path = path.join("malformed-report.json");
        std::fs::write(
            &malformed_path,
            serde_json::to_vec(&malformed_report).unwrap(),
        )
        .unwrap();
        let refused = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("inspect")
            .arg(&malformed_path)
            .output()
            .unwrap();
        assert!(!refused.status.success());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("projection evidence framing"));
        // A changed opaque domain field preserves all generic framing and can
        // match its outer hash. Independent numerical replay must still refuse.
        let inventory =
            conduit_core::projection::inspect_projection_evidence(&report.value_node()[5..])
                .unwrap();
        let domain_byte = inventory.source_bytes.as_ptr() as usize - value.bytes.as_ptr() as usize
            + inventory.source_bytes.len()
            - 1;
        let mut forged_domain = value.clone();
        forged_domain.bytes[domain_byte] ^= 1;
        conduit_core::projection::inspect_projection_evidence(
            &forged_domain.bytes[report_start..report_start + report.value_node().len() - 5],
        )
        .unwrap();
        let digest =
            conduit_core::semantic_digest("conduit/routed-value-content@1", &forged_domain.bytes);
        if let ObservationKind::RoutedContent { evidence } = &mut forged_domain.sign.sign.kind {
            evidence.digest = digest;
        }
        conduit_observatory::validate_routed_value_artifact(&forged_domain).unwrap();
        let domain_path = path.join("forged-domain.json");
        std::fs::write(&domain_path, serde_json::to_vec(&forged_domain).unwrap()).unwrap();
        let refused = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("inspect")
            .arg(&domain_path)
            .output()
            .unwrap();
        assert!(!refused.status.success());
        assert!(String::from_utf8_lossy(&refused.stderr)
            .contains("independent projection replay refused"));
        let mut forged = value.clone();
        forged.bytes[0] ^= 1;
        let forged_path = path.join("forged-value.json");
        std::fs::write(&forged_path, serde_json::to_vec(&forged).unwrap()).unwrap();
        assert!(!Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("inspect")
            .arg(&forged_path)
            .output()
            .unwrap()
            .status
            .success());

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
