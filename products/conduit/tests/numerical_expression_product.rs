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

#[test]
fn explicit_f64_cancellation_keeps_intermediate_rounding_in_the_product() {
    let plan = run(
        include_str!("fixtures/numerical_f64_cancellation.conduit"),
        "IEEE binary64 cancellation",
        "rounded intermediate",
    );
    let computation = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|gear| {
            gear.configuration.iter().any(|entry| {
                let conduit_core::ConfigurationValue::Text(encoded) = &entry.value else {
                    return false;
                };
                conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).is_ok_and(
                    |program| {
                        matches!(program.output_type.shape(),
                    conduit_core::StructuredInfoTypeShape::Leaf(kind)
                        if kind.as_str() == conduit_core::F64_INFO_ID)
                    },
                )
            })
        })
        .expect("the actual binary64 computation remains inspectable");
    assert_eq!(
        computation.implementation_id.as_str(),
        "std/kernel-pure-expression@1"
    );
    assert!(computation.realization_properties.iter().any(|property| {
        let Ok(value) = conduit_core::StructuredInfoValue::from_canonical_bytes(property.canonical_value()) else {
            return false;
        };
        matches!(value.value_type().shape(),
            conduit_core::StructuredInfoTypeShape::Record { schema, .. }
                if schema.as_str() == "std/source-expression-selection@1")
            && matches!(value.shape(), conduit_core::StructuredInfoValueShape::Record(fields)
                if fields.iter().any(|field| field.name() == "tape-profile-refusal"
                    && matches!(field.value().shape(), conduit_core::StructuredInfoValueShape::Leaf(bytes)
                        if std::str::from_utf8(bytes).is_ok_and(|why| why.contains("scalar-tape/fixed-integer-bool@1")))))
    }));
}

#[test]
fn pi_over_180_survives_a_structured_product_transport_without_rounding() {
    // math/radians has independent core denotation tests; this checks that the
    // selected product path preserves that denotation through record transport.
    run(
        include_str!("fixtures/numerical_symbolic_transport.conduit"),
        "Pi over 180",
        "exact symbolic transport",
    );
}

#[test]
fn a_checked_integer_uses_the_sealed_tape_through_the_default_product_host() {
    let plan = run(
        include_str!("fixtures/numerical_checked_machine.conduit"),
        "Checked integer",
        "checked machine",
    );
    let add = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|gear| {
            gear.configuration.iter().any(|entry| {
                let conduit_core::ConfigurationValue::Text(encoded) = &entry.value else {
                    return false;
                };
                let Ok(program) =
                    conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
                else {
                    return false;
                };
                matches!(
                    program.root.operation,
                    conduit_plot::PortableExpressionOperation::Binary {
                        operator: conduit_plot::BinaryOperator::Add,
                        ..
                    }
                ) && matches!(program.output_type.shape(),
                    conduit_core::StructuredInfoTypeShape::Leaf(kind)
                        if kind.as_str() == "value/u64")
            })
        })
        .expect("the actual checked addition remains inspectable");
    assert_eq!(
        add.implementation_id.as_str(),
        "std/kernel-pure-scalar-tape@1"
    );
    assert!(add.realization_properties.iter().any(|property| {
        conduit_core::StructuredInfoValue::from_canonical_bytes(property.canonical_value())
            .ok()
            .is_some_and(|value| {
                matches!(value.value_type().shape(),
                conduit_core::StructuredInfoTypeShape::Record { schema, .. }
                    if schema.as_str() == "std/pure-scalar-tape-bounds@1")
            })
    }));
}
