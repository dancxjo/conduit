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
    assert!(plan.fragments.iter().flat_map(|f| &f.placements).any(|g| {
        g.kind_id
            .as_str()
            .starts_with("conduitese/pure-expression/")
            && g.implementation_id.as_str() == "std/kernel-pure-expression@1"
    }));
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
    let source = format!(
        "plot increment (\n >> value: MathematicalValue\n result: Text >>\n) = ((. + 1 == {successor}) ? \"exact\" : \"wrong\")\n\nplot main {{\n seed: math/realize(request={{value:{original},maximum-error:0}})\n check: increment\n shown: face/text(\"Large exact expression\")\n seed.result >> (.source) >> check.value\n check.result >> shown\n}}.\n"
    );
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
fn declared_observation_uncertainty_is_retained_separately_from_numeric_error() {
    // This is authored observation metadata, not physical measurement proof.
    // Exact encoding of the reading does not reduce its declared uncertainty.
    let plan = run(
        include_str!("fixtures/numerical_observation_uncertainty.conduit"),
        "Observation uncertainty",
        "uncertainty retained independently",
    );
    let projection = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .find(|gear| gear.kind_id.as_str() == "math/realize")
        .unwrap();
    let property = projection
        .realization_properties
        .iter()
        .find(|p| *p.profile() == conduit_core::numerical_realization_profile())
        .unwrap();
    let mut work = conduit_core::ExactNumericWork::new(100_000_000);
    let certificate =
        conduit_core::PreparedNumericalRealization::from_property(property, &mut work).unwrap();
    assert_eq!(
        certificate.algorithm(),
        conduit_core::NumericalAlgorithm::Binary64 {
            rounding: conduit_core::Binary64Rounding::NearestTiesEven,
        }
    );
    let uncertainty = conduit_core::ExactRational::parse_decimal("0.05", &mut work).unwrap();
    assert_eq!(
        certificate
            .maximum_error()
            .checked_compare(uncertainty, &mut work)
            .unwrap(),
        std::cmp::Ordering::Less
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

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[test]
fn explicit_ieee_back_executes_source_rounding_and_flags_through_the_product() {
    use conduit_core::{Binary64Rounding as Rounding, IeeeBinary64Operation as Operation};
    let operations = [
        ("add", Operation::Add),
        ("subtract", Operation::Subtract),
        ("multiply", Operation::Multiply),
        ("divide", Operation::Divide),
    ];
    let roundings = [
        ("nearest_ties_even", Rounding::NearestTiesEven),
        ("toward_zero", Rounding::TowardZero),
        ("toward_positive", Rounding::TowardPositive),
        ("toward_negative", Rounding::TowardNegative),
    ];
    let fixture =
        include_str!("../../../architecture/core/tests/fixtures/ieee_binary64_softfloat.tsv");
    let mut count = 0;
    for line in fixture.lines().filter(|line| line.starts_with("a ")) {
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        let op: usize = fields[1].parse().unwrap();
        let mode: usize = fields[2].parse().unwrap();
        let (left, right, result) = (fields[3], fields[4], fields[5]);
        let special = mode == 0
            && matches!(
                (op, left, right),
                (3, "3ff0000000000000", "0000000000000000")
                    | (2, "7fefffffffffffff", "4008000000000000")
                    | (2, "0000000000000001", "0000000000000001")
                    | (0, "fff0000000000069", "3ff0000000000000")
                    | (0, "7ff8000000000042", "3ff0000000000000")
                    | (2, "8000000000000000", "3ff0000000000000")
            );
        if !(left == "3ff0000000000000" && right == "4008000000000000") && !special {
            continue;
        }
        let flags: u8 = fields[6].parse().unwrap();
        let flags = [
            ("inexact", 1),
            ("underflow", 2),
            ("overflow", 4),
            ("divide-by-zero", 8),
            ("invalid", 16),
        ]
        .map(|(name, bit)| format!("{}.flags.{name}", if flags & bit != 0 { "" } else { "!" }))
        .join(" && ");
        let (operation, expected_operation) = operations[op];
        let (rounding, expected_rounding) = roundings[mode];
        let source = ieee_product_source(
            "math/ieee-binary64",
            &format!("operation={operation}(empty),rounding={rounding}(empty)"),
            "IeeeBinary64Result",
            left,
            right,
            &format!(".value == 0x{result} && {flags}"),
        );
        let plan = run(&source, "Explicit IEEE", "explicit rounding and flags");
        let selected = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::IEEE_BINARY64_KIND)
            .unwrap();
        assert_eq!(
            selected.implementation_id.as_str(),
            "conduit.std/ieee-binary64@1"
        );
        assert_eq!(
            selected.execution_profile_id.as_str(),
            "std/ieee-binary64-x86_64-sse2@1"
        );
        assert_eq!(selected.realization_properties.len(), 1);
        assert_eq!(
            conduit_semantic_catalog::ieee_binary64_configuration(
                &selected.kind_id,
                &selected.configuration
            )
            .unwrap(),
            conduit_semantic_catalog::IeeeBinary64Configuration::Arithmetic {
                operation: expected_operation,
                rounding: expected_rounding
            }
        );
        println!(
            "oracle-case={line} selected-ieee={}",
            serde_json::to_string(selected).unwrap()
        );
        count += 1;
    }
    assert_eq!(count, 22);
}

fn ieee_product_source(
    kind: &str,
    configuration: &str,
    result_type: &str,
    left: &str,
    right: &str,
    condition: &str,
) -> String {
    format!(
        r#"plot left (
 >> trigger: MathematicalValue
 result: F64 >>
) = (0x{left})

plot right (
 >> trigger: MathematicalValue
 result: F64 >>
) = (0x{right})

plot explain (
 >> receipt: {result_type}
 result: Text >>
) = (({condition}) ? "explicit rounding and flags" : "wrong")

plot main {{
 seed: math/realize(request={{value:0,maximum-error:0}})
 left: left
 right: right
 operation: {kind}({configuration})
 check: explain
 show: face/text("Explicit IEEE")
 seed.result >> (.source) >> left.trigger
 seed.result >> (.source) >> right.trigger
 left.result >> operation.left
 right.result >> operation.right
 operation.result >> check.receipt
 check.result >> show
}}.
"#
    )
}

#[test]
fn quiet_ieee_predicates_execute_through_the_product_with_separate_invalid_flags() {
    use conduit_core::IeeeBinary64QuietPredicate as Predicate;
    let predicates = [
        ("equal", Predicate::Equal),
        ("less", Predicate::Less),
        ("less_equal", Predicate::LessEqual),
        ("unordered", Predicate::Unordered),
    ];
    let fixture =
        include_str!("../../../architecture/core/tests/fixtures/ieee_binary64_softfloat.tsv");
    let mut count = 0;
    for line in fixture.lines().filter(|line| line.starts_with("c ")) {
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        let (left, right) = (fields[3], fields[4]);
        if !matches!(
            (left, right),
            ("8000000000000000", "0000000000000000") | ("fff0000000000069", "3ff0000000000000")
        ) {
            continue;
        }
        let index: usize = fields[1].parse().unwrap();
        let (predicate, expected) = predicates[index];
        let value = if fields[5] == "0000000000000001" {
            ""
        } else {
            "!"
        };
        let invalid = if fields[6] == "16" { "" } else { "!" };
        let source = ieee_product_source(
            "math/ieee-binary64-compare-quiet",
            &format!("predicate={predicate}(empty)"),
            "IeeeBinary64ComparisonResult",
            left,
            right,
            &format!(
                "{value}.value && {invalid}.flags.invalid && !.flags.inexact && !.flags.divide-by-zero && !.flags.overflow && !.flags.underflow"
            ),
        );
        let plan = run(&source, "Explicit IEEE", "explicit rounding and flags");
        let selected = plan
            .fragments
            .iter()
            .flat_map(|f| &f.placements)
            .find(|gear| {
                gear.kind_id.as_str() == conduit_semantic_catalog::IEEE_BINARY64_QUIET_KIND
            })
            .unwrap();
        assert_eq!(
            selected.execution_profile_id.as_str(),
            "std/ieee-binary64-quiet-integer@1"
        );
        assert_eq!(
            conduit_semantic_catalog::ieee_binary64_configuration(
                &selected.kind_id,
                &selected.configuration
            )
            .unwrap(),
            conduit_semantic_catalog::IeeeBinary64Configuration::Quiet {
                predicate: expected
            }
        );
        println!(
            "oracle-case={line} selected-ieee={}",
            serde_json::to_string(selected).unwrap()
        );
        count += 1;
    }
    assert_eq!(count, 8);
}

#[test]
fn actual_rational_quantity_reaches_the_explicit_near_boundary_in_one_play() {
    let plan = run(
        include_str!("fixtures/numerical_quantity_near.conduit"),
        "Near quantity",
        "certified",
    );
    let near: Vec<_> = plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .filter(|g| g.kind_id.as_str() == "math/near")
        .collect();
    assert_eq!(near.len(), 1);
    assert_eq!(
        near[0].execution_profile_id.as_str(),
        "std/quantity-near-binary32-single-round@2"
    );
}

#[test]
fn qualified_numerical_result_cannot_be_relabelled_as_exact_quantity() {
    let source = include_str!("fixtures/numerical_quantity_near.conduit")
        .replace(">> value: NearQuantity", ">> value: Quantity")
        .replace(
            ".source == 1 / 3 && .maximum-error == 1 / 100663296",
            ". == 1m",
        );
    let files = Files::new(&source);
    let result = Command::new(env!("CARGO_BIN_EXE_conduit"))
        .arg("run")
        .arg(files.0.join("main.conduit"))
        .arg("--await-terminal")
        .arg("--report")
        .arg(files.0.join("report.json"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    let diagnostic = String::from_utf8(result.stderr).unwrap();
    assert!(diagnostic.contains("CND-FRM-045"), "{diagnostic}");
    assert!(
        diagnostic.contains("incompatible payload contracts"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("sink value/quantity@1"), "{diagnostic}");
    assert!(
        !files.0.join("report.json").exists(),
        "must refuse before Play"
    );
}

#[path = "numerical_expression_product/near_fore.rs"]
mod near_fore;
