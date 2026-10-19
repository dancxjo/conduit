//! Dynamic exact data crosses checked transport and the installed numeric consumer.
use super::*;
use crate::host_execution::scalar_tape_fixture::{NoTimer, Outputs};
use crate::{ExternalForeInput, StdHost};
use conduit_plot::*;
use std::collections::BTreeMap;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn prepared(width: &str) -> (StdHost, Plan) {
    let source = format!("plot carry (\n >> value: MathematicalValue <= 33059B\n result: MathematicalValue <= 33059B >>\n) = .\n\nplot bounded (\n >> value: MathematicalValue <= 33059B\n result: MathematicalEnclosure <= 65536B >>\n) {{\n exact: carry()\n interval: math/enclose(request={{maximum-width:{width}}})\n value >> exact.value\n exact.result >> interval.value\n interval.result >> result\n}}\n");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_numerical_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "bounded", &profile).unwrap();
    let mut host = StdHost::new();
    host.prepare_expression_realizations(&authored.expanded)
        .unwrap();
    host.prepare_numerical_realizations(&authored.expanded)
        .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: MATHEMATICAL_ENCLOSE_MAX_BYTES,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from(LOCAL_BASE_IMPLEMENTATION_ID)],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MATHEMATICAL_ENCLOSE_MAX_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.source_document_id, syntax.source_document_id());
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|p| p.implementation_id.as_str() == IMPLEMENTATION));
    (host, plan)
}
fn specimen(root: bool) -> Vec<u8> {
    let mut b = MathematicalValueBuilder::new();
    let value = if root {
        let two = b
            .insert(
                MathematicalNode::Rational(ExactRational::from_i128(2)),
                &mut work(),
            )
            .unwrap();
        b.insert(MathematicalNode::PrincipalSquareRoot(two), &mut work())
            .unwrap()
    } else {
        let pi = b.insert(MathematicalNode::Pi, &mut work()).unwrap();
        let n = b
            .insert(
                MathematicalNode::Rational(ExactRational::from_i128(180)),
                &mut work(),
            )
            .unwrap();
        b.insert(MathematicalNode::Divide(pi, n), &mut work())
            .unwrap()
    };
    let value = b.finish(value, &mut work()).unwrap();
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a [u8] {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!()
    };
    let StructuredInfoValueShape::Leaf(bytes) = fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value()
        .shape()
    else {
        panic!()
    };
    bytes
}
fn rational(value: &StructuredInfoValue, name: &str) -> ExactRational {
    MathematicalValue::decode(field(value, name), &mut work())
        .unwrap()
        .as_rational()
        .unwrap()
}
#[test]
fn dynamic_pi_over_180_and_sqrt_two_cross_exact_transport_and_installed_interval_consumer() {
    let (mut host, plan) = prepared("0.00000000000000001");
    for root in [false, true] {
        let input = specimen(root);
        let mut output = Outputs(Vec::with_capacity(1));
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input.clone(),
                }],
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(output.0.len(), 1);
        let result = StructuredInfoValue::from_canonical_bytes(&output.0[0]).unwrap();
        assert_eq!(field(&result, "source"), input);
        let lower = rational(&result, "lower");
        let upper = rational(&result, "upper");
        let width = rational(&result, "width");
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == MATHEMATICAL_ENCLOSE_KIND)
            .unwrap();
        let requirements = mathematical_enclose_requirements(&placement.configuration).unwrap();
        let mut verifier = PreparedMathematicalEnclosureVerifier::new(
            &mathematical_enclosure_type(),
            requirements,
            800_000_000,
            131072,
        )
        .unwrap();
        let guard = crate::allocation_probe::begin();
        let checked = verifier
            .verify(
                &output.0[0],
                requirements.maximum_width,
                Some(&input),
                &mut ExactNumericWork::new(800_000_000),
            )
            .unwrap();
        assert_eq!(checked.width(), width);
        assert_eq!(guard.finish(), 0);
        assert_eq!(
            rational(&result, "maximum-width"),
            q(1, 100_000_000_000_000_000)
        );
        assert_eq!(upper.checked_subtract(lower, &mut work()).unwrap(), width);
        let (lo, hi) = if root {
            (
                q(14142135623730950, 10_000_000_000_000_000),
                q(14142135623730951, 10_000_000_000_000_000),
            )
        } else {
            (
                q(174532925199432, 10_000_000_000_000_000),
                q(174532925199434, 10_000_000_000_000_000),
            )
        };
        assert_ne!(
            lower.checked_compare(lo, &mut work()).unwrap(),
            core::cmp::Ordering::Less
        );
        assert_ne!(
            upper.checked_compare(hi, &mut work()).unwrap(),
            core::cmp::Ordering::Greater
        );
        assert_eq!(
            width
                .checked_compare(q(1, 100_000_000_000_000_000), &mut work())
                .unwrap(),
            core::cmp::Ordering::Less
        );
        let kernel = report.kernel.as_ref().unwrap();
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        println!(
            "SYMBOLIC_ENCLOSURE root={root} plan={} report={report:?}",
            plan.plan_id.as_str()
        );
    }
}
#[test]
fn impossible_dynamic_precision_emits_no_output_and_valid_exact_input_recovers() {
    let (mut host, plan) = prepared("0");
    let mut output = Outputs(Vec::with_capacity(1));
    assert!(host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: specimen(false),
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer
        )
        .is_err());
    assert!(output.0.is_empty());
    let value = MathematicalValue::rational(ExactRational::ONE);
    let mut input = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut input).unwrap();
    input.truncate(n);
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: input,
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), 1);
    let result = StructuredInfoValue::from_canonical_bytes(&output.0[0]).unwrap();
    assert_eq!(rational(&result, "width"), ExactRational::ZERO);
    assert_eq!(report.kernel.unwrap().post_play_start_allocations, 0);
}

#[test]
fn initialized_consumer_recomputes_request_bounds_and_exact_selected_identity() {
    let (_, plan) = prepared("0.00000000000000001");
    let correct = plan.fragments[0]
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == MATHEMATICAL_ENCLOSE_KIND)
        .unwrap();
    assert!(admitted(correct).is_ok());
    let mut invalid = vec![];
    let mut changed = correct.clone();
    changed.realization_properties.clear();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.artifact_id = "foreign/artifact".into();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.outputs.clear();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.semantic_contract.laws.clear();
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.limits.max_queue_bytes = 1;
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.configuration.push(changed.configuration[0].clone());
    invalid.push(changed);
    let mut changed = correct.clone();
    changed.execution_profile_id = "foreign/profile".into();
    invalid.push(changed);
    for changed in invalid {
        assert!(admitted(&changed).is_err());
        assert!(budget(&changed).is_err());
    }
}
