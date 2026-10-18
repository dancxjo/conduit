//! Actual installed kernel profile and independent factory fact checks.
use super::*;
use crate::host_execution::scalar_tape_fixture::{prepare_with_offer, NoTimer, Outputs};
use crate::ExternalForeInput;
use conduit_core::{ConnectionTrack, ExactNumericWork, ExactRational, Quantity, QuantityValueView};
const SOURCE: &str = "plot divide (\n >> value: Distance\n result: Distance >>\n) = (. / 3)\n";
fn fixture() -> (
    crate::StdHost,
    conduit_core::Plan,
    conduit_plot::PortableExpressionProgram,
) {
    prepare_with_offer(SOURCE, "divide", |program, temporal| {
        conduit_std_offers::pure_exact_quantity_std_offer(program, temporal).unwrap()
    })
}
#[test]
fn exact_quantity_installed_back_emits_a_typed_third_without_kernel_growth_or_allocations() {
    let native = Quantity::parse_plot_literal("1m").unwrap();
    let mut input = native.encode().to_vec();
    let mut expected = ExactRational::from_i128(1);
    for _ in 0..2 {
        let (mut host, plan, program) = fixture();
        assert!(conduit_core::verify_plan(&plan));
        assert_eq!(
            program.maximum_prepared_input_bytes(),
            Ok(conduit_core::QUANTITY_ENCODED_LEN as u32)
        );
        let placement = &plan.fragments[0].placements[0];
        assert_eq!(
            placement.execution_profile_id.as_str(),
            conduit_std_offers::PURE_EXACT_QUANTITY_STD_PROFILE
        );
        let input_bound = placement
            .realization_properties
            .iter()
            .find_map(|property| {
                let value = conduit_core::StructuredInfoValue::from_canonical_bytes(
                    property.canonical_value(),
                )
                .unwrap();
                let conduit_core::StructuredInfoTypeShape::Record { schema, .. } =
                    value.value_type().shape()
                else {
                    return None;
                };
                if schema.as_str() != "std/exact-quantity-expression-bounds@1" {
                    return None;
                }
                let conduit_core::StructuredInfoValueShape::Record(fields) = value.shape() else {
                    return None;
                };
                let field = fields
                    .iter()
                    .find(|f| f.name() == "maximum-input-bytes")
                    .unwrap();
                let conduit_core::StructuredInfoValueShape::Leaf(bytes) = field.value().shape()
                else {
                    return None;
                };
                Some(u64::from_le_bytes(bytes.try_into().unwrap()))
            })
            .unwrap();
        assert_eq!(
            input_bound,
            conduit_core::QUANTITY_VALUE_MAX_ENCODED_BYTES as u64
        );
        let mut outputs = Outputs(Vec::with_capacity(1));
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input,
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(outputs.0.len(), 1);
        assert_eq!(outputs.0[0][0], 2);
        let mut work = ExactNumericWork::new(conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
        expected = expected
            .checked_divide(ExactRational::from_i128(3), &mut work)
            .unwrap();
        let value = QuantityValueView::decode(&outputs.0[0], &mut work).unwrap();
        assert_eq!(value.unit(), native.unit());
        assert_eq!(value.rational(&mut work).unwrap().coordinate(), expected);
        let kernel = report.kernel.as_ref().unwrap();
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        println!(
            "EXACT_QUANTITY_KERNEL plan={} report={report:?}",
            plan.plan_id.as_str()
        );
        input = outputs.0.remove(0);
    }
}
#[test]
fn missing_foreign_or_downgraded_wire_facts_refuse_at_installation() {
    let (_, plan, _) = fixture();
    let original = &plan.fragments[0].placements[0];
    assert!(PureExpressionHost::from_placement(original).is_ok());
    for index in 0..original.realization_properties.len() {
        let mut missing = original.clone();
        missing.realization_properties.remove(index);
        assert!(PureExpressionHost::from_placement(&missing).is_err());
        let mut foreign = original.clone();
        let property = &foreign.realization_properties[index];
        let mut bytes = property.canonical_value().to_vec();
        *bytes.last_mut().unwrap() ^= 1;
        foreign.realization_properties[index] =
            conduit_core::StructuredConfigurationValue::new(property.profile().clone(), bytes)
                .unwrap();
        assert!(PureExpressionHost::from_placement(&foreign).is_err());
    }
    let mut downgraded = original.clone();
    downgraded.execution_profile_id = conduit_std_offers::PURE_EXPRESSION_STD_PROFILE.into();
    downgraded.artifact_id = conduit_std_offers::PURE_EXPRESSION_STD_ARTIFACT.into();
    assert!(PureExpressionHost::from_placement(&downgraded).is_err());
}

#[test]
fn compatible_alternative_is_selected_and_executes_the_exact_source_chain() {
    const CHAIN: &str = "plot divide (\n >> value: Distance\n result: Distance >>\n) = (. / 3)\nplot restore (\n >> value: Distance\n result: Distance >>\n) = (. * 3)\nplot main (\n >> value: Distance\n result: Distance >>\n) {\n third: divide\n restored: restore\n value >> third.value\n third.result >> restored.value\n restored.result >> result\n}\n";
    let mut index = 0;
    let (mut host, plan, _) = crate::host_execution::scalar_tape_fixture::prepare_with_offers(
        CHAIN,
        "main",
        |program, temporal| {
            index += 1;
            let mut exact =
                conduit_std_offers::pure_exact_quantity_std_offer(program, temporal).unwrap();
            exact.capability_id = format!("z-exact/{index}").into();
            // Only the downstream expression has a preferred legacy alternative.
            // Both are genuine initialized Back implementations, not dummy offers.
            if matches!(
                program.root.operation,
                conduit_plot::PortableExpressionOperation::Binary {
                    operator: conduit_plot::BinaryOperator::Multiply,
                    ..
                }
            ) {
                let mut legacy =
                    conduit_std_offers::pure_expression_std_offer(program, temporal).unwrap();
                legacy.capability_id = "a-legacy/restore".into();
                vec![legacy, exact]
            } else {
                vec![exact]
            }
        },
    );
    assert_eq!(index, 2);
    assert!(conduit_core::verify_plan(&plan));
    assert!(plan
        .fragments
        .iter()
        .flat_map(|f| &f.placements)
        .all(|p| p.execution_profile_id.as_str()
            == conduit_std_offers::PURE_EXACT_QUANTITY_STD_PROFILE));
    let input = Quantity::parse_plot_literal("1m").unwrap();
    let mut output = Outputs(Vec::with_capacity(1));
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: input.encode().to_vec(),
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), 1);
    let mut work = ExactNumericWork::new(conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let result = QuantityValueView::decode(&output.0[0], &mut work).unwrap();
    assert_eq!(result.unit(), input.unit());
    assert_eq!(result.role(), input.role());
    assert_eq!(
        result.rational(&mut work).unwrap().coordinate(),
        ExactRational::ONE
    );
    let kernel = report.kernel.as_ref().unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    println!("QUANTITY_COMPATIBLE_SELECTION plan={plan:?} report={report:?}");
}
