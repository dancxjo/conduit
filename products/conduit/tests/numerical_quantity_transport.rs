//! Required real product gates for exact Quantity wire admission.
use conduit_core::{Observation, ObservationKind, Plan, TerminalDisposition};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "conduit-quantity-wire-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn run(source: &str) -> (Files, Output) {
    let dir = Files::new();
    let path = dir.path().join("main.conduit");
    std::fs::write(&path, source).unwrap();
    let check = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "invalid fixture: {}\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("run")
        .arg(path)
        .arg("--await-terminal")
        .arg("--report")
        .arg(dir.path().join("report.json"))
        .output()
        .unwrap();
    (dir, output)
}
#[test]
fn rational_quantity_survives_separate_source_gears_and_the_actual_kernel() {
    let source = include_str!("fixtures/numerical_quantity_rational_chain.conduit");
    let (dir, output) = run(source);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("\nExact quantity chain: exact\n"));
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("report.json")).unwrap()).unwrap();
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
        .filter(|g| {
            g.execution_profile_id.as_str() == "std/pure-expression-quantity-exact-hosted@1"
        })
        .collect();
    assert_eq!(
        exact.len(),
        3,
        "divide, restore and physical comparison must select the reviewed profile"
    );
    for gear in exact {
        let wire = gear
            .realization_properties
            .iter()
            .find(|p| p.profile() == &conduit_core::QuantityWireContract::property_kind())
            .unwrap();
        let wire = conduit_core::QuantityWireContract::from_property(wire).unwrap();
        assert_eq!(
            wire.input(&"input".into()),
            conduit_core::QuantityValueProfile::Exact
        );
        assert!(gear.realization_properties.len() >= 2);
    }
    let observations: Vec<Observation> =
        serde_json::from_value(report["observations"].clone()).unwrap();
    println!(
        "QUANTITY_SOURCE_PLAN {}",
        serde_json::to_string(&plan).unwrap()
    );
    println!(
        "QUANTITY_SOURCE_OBSERVATIONS {}",
        serde_json::to_string(&observations).unwrap()
    );
    assert!(matches!(
        observations.last().map(|o| &o.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert!(observations
        .iter()
        .all(|o| o.plan_id.as_ref() == Some(&plan.plan_id) && o.active_play_id.is_some()));
    assert!(
        plan.fragments
            .iter()
            .flat_map(|f| &f.placements)
            .filter(|g| g
                .kind_id
                .as_str()
                .starts_with("conduitese/pure-expression/"))
            .count()
            >= 4
    );
}
fn legacy_source(operation: &str) -> String {
    format!("plot quantity (\n >> trigger: MathematicalValue\n result: Quantity >>\n) = (1m)\n\nplot transform (\n >> value: Quantity\n result: Quantity >>\n) = ({operation})\n\nplot main {{\n seed: math/realize(request={{value:0,maximum-error:0}})\n quantity: quantity\n transform: transform\n wrap: structured-info/wrap-quantity\n shown: face/quantity(label=\"Quantity\")\n seed.result >> (.source) >> quantity.trigger\n quantity.result >> transform.value\n transform.result >> wrap.in\n wrap.out >> shown.input\n}}.\n")
}
#[test]
fn decimal_only_consumer_refuses_unproven_exact_wire_profile_before_play() {
    // Control: this is a valid Source graph and an installed native consumer.
    let (_, native) = run(&legacy_source("."));
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    let (dir, exact) = run(&legacy_source(". / 3"));
    assert!(!exact.status.success());
    assert!(
        String::from_utf8_lossy(&exact.stderr).contains("quantity/wire-profile-mismatch@1"),
        "{}",
        String::from_utf8_lossy(&exact.stderr)
    );
    assert!(
        !dir.path().join("report.json").exists(),
        "ineligible wire profile must refuse before Play"
    );
}

#[test]
fn reviewed_native_quantity_catalog_and_generic_literal_execute_in_the_public_product() {
    let source = legacy_source(".");
    let (dir, output) = run(&source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("report.json")).unwrap()).unwrap();
    let plan: Plan = serde_json::from_value(report["plans"][0].clone()).unwrap();
    assert!(conduit_core::verify_plan(&plan));
    assert_eq!(
        plan.source_document_id,
        conduit_plot::parse_syntax_document(&source).source_document_id()
    );
    let placements: Vec<_> = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect();
    let wrapper = placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::QUANTITY_INFO_WRAP_KIND)
        .unwrap();
    assert_eq!(
        wrapper.implementation_id.as_str(),
        "std/kernel-wrap-quantity@1"
    );
    assert_eq!(
        wrapper.execution_profile_id.as_str(),
        "conduit.std/wrap-quantity-kernel@1"
    );
    assert_eq!(
        wrapper.host_calls[0].maximum_input_bytes,
        conduit_core::QUANTITY_ENCODED_LEN as u32
    );
    let expected = conduit_core::Quantity::parse_plot_literal("1m")
        .unwrap()
        .encode();
    assert!(placements.iter().any(|gear| gear.configuration.iter().any(|entry| {
        let conduit_core::ConfigurationValue::Text(encoded) = &entry.value else { return false; };
        let Ok(program) = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded) else { return false; };
        program.output_type == conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::QUANTITY_INFO_ID)).unwrap()
            && matches!(&program.root.operation, conduit_plot::PortableExpressionOperation::CanonicalLiteral(bytes) if bytes.as_slice() == expected)
    })));
    let observations: Vec<Observation> =
        serde_json::from_value(report["observations"].clone()).unwrap();
    assert!(matches!(
        observations.last().map(|o| &o.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    println!(
        "native quantity control Source={} Plan={} Play={}",
        plan.source_document_id.as_str(),
        plan.plan_id.as_str(),
        report["plays"][0]["active_play_id"]
    );
}
