//! Actual product artifacts retain independently replayable native/exact reports.
use conduit_core::*;
use std::process::Command;
#[test]
fn native_exact_and_rational_reports_survive_product_custody_and_inspection() {
    for (index, expression, tolerance, profile, representation) in [
        (
            0,
            "1m",
            "0m",
            "std/quantity-near-exact-rational@2",
            "exact_rational",
        ),
        (
            1,
            "1m",
            "0.000001m",
            "std/quantity-near-binary32-single-round@2",
            "binary32",
        ),
        (
            2,
            "1m / 3",
            "0.000001m",
            "std/quantity-near-binary32-single-round@2",
            "binary32",
        ),
    ] {
        let path = std::env::temp_dir().join(format!(
            "conduit-near-reports-{}-{index}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        let expected = if index == 2 { "1 / 3" } else { "1" };
        let expression = if index == 2 { "1m" } else { expression };
        let division = if index == 2 {
            "plot divide (\n >> value: Quantity\n result: Quantity >>\n) = (. / 3)\n"
        } else {
            ""
        };
        let route = if index == 2 {
            "third: divide\n original.result >> third.value\n third.result >> boundary.value"
        } else {
            "original.result >> boundary.value"
        };
        let source = format!("plot source (\n >> trigger: MathematicalValue\n result: Quantity >>\n) = ({expression})\nplot inspect (\n >> value: Quantity near 0.1m\n result: Text >>\n) = (.source == {expected} ? \"verified report\" : \"wrong\")\n{division}plot main {{\n seed: math/realize(request={{value:0,maximum-error:0}})\n original: source\n boundary: math/near(limit={tolerance})\n check: inspect\n show: face/text(\"Near report\")\n seed.result >> (.source) >> original.trigger\n {route}\n boundary.result >> check.value\n check.result >> show\n}}.\n");
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
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("verified report"));
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path.join("report.json")).unwrap()).unwrap();
        let plan: Plan = serde_json::from_value(report["plans"][0].clone()).unwrap();
        let producer = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|p| p.kind_id.as_str() == "math/near")
            .unwrap();
        assert_eq!(producer.execution_profile_id.as_str(), profile);
        let signs: Vec<Observation> =
            serde_json::from_value(report["observations"].clone()).unwrap();
        let sign = signs
            .iter()
            .find(|s| {
                s.placement_id.as_ref() == Some(&producer.placement_id)
                    && matches!(s.kind, ObservationKind::RoutedContent { .. })
            })
            .unwrap();
        let value_path = path
            .join("artifacts")
            .join(format!("value-{}.json", sign.sign_id.as_str()));
        let value: conduit_observatory::RoutedValueArtifact =
            serde_json::from_slice(&std::fs::read(&value_path).unwrap()).unwrap();
        conduit_observatory::validate_routed_value_artifact(&value).unwrap();
        let frame = validate_canonical_structured_value(&value.bytes).unwrap();
        let tag = frame
            .record_field("representation")
            .unwrap()
            .unwrap()
            .variant_tag()
            .unwrap();
        assert_eq!(tag, representation);
        let field = frame.record_field("projection-evidence").unwrap().unwrap();
        let evidence = &field.value_node()[5..];
        let claims = projection::inspect_projection_evidence(evidence).unwrap();
        assert_eq!(claims.facts().count(), 3);
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
        let text = String::from_utf8_lossy(&inspected.stdout);
        assert!(
            text.contains("Numerical certificate independently replayed"),
            "{text}"
        );
        assert!(text.contains("original Plan request matched"));
        // Mutation remains a canonical typed value and has a matching outer
        // content digest, but independent report replay must still refuse it.
        let offset = evidence.as_ptr() as usize - value.bytes.as_ptr() as usize;
        let mut forged = value.clone();
        forged.bytes[offset + 3] ^= 1;
        if let ObservationKind::RoutedContent { evidence } = &mut forged.sign.sign.kind {
            evidence.digest = semantic_digest("conduit/routed-value-content@1", &forged.bytes);
        }
        conduit_observatory::validate_routed_value_artifact(&forged).unwrap();
        // The Sign identity is contextual rather than a content signature.
        let forged_path = path.join("forged.json");
        std::fs::write(&forged_path, serde_json::to_vec(&forged).unwrap()).unwrap();
        let refused = Command::new(env!("CARGO_BIN_EXE_conduit"))
            .arg("inspect")
            .arg(&forged_path)
            .output()
            .unwrap();
        assert!(!refused.status.success());
        assert!(String::from_utf8_lossy(&refused.stderr).contains("projection evidence framing"));
        std::fs::remove_dir_all(path).unwrap();
    }
}
