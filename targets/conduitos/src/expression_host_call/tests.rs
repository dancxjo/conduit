use super::*;
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_plot::*;

fn program() -> PortableExpressionProgram {
    let syntax = parse_syntax_document(
        "plot add (\n input: U8 >> output: U8\n) {\n input >> (. + 1) >> output\n}\n",
    );
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let BackStatement::Cord(cord) = &syntax.plots[0].back[0] else {
        panic!("cord")
    };
    let CordStage::PureExpression(expression) = &cord.stages[1] else {
        panic!("expression")
    };
    let input = CheckedExpressionType::semantic("value/u8");
    let values = BTreeMap::new();
    let types = BTreeMap::new();
    let numeric = BTreeSet::new();
    let kinds = BTreeMap::new();
    PortableExpressionProgram::from_checked(
        &check_expression(
            &expression.syntax,
            &ExpressionTypeContext {
                glyph_values: None,
                input: &input,
                immutable_values: &values,
                structured_types: &types,
                literal_types: &values,
                numeric_types: &numeric,
                semantic_kinds: &kinds,
            },
        )
        .unwrap(),
    )
    .unwrap()
}

fn selected() -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    selected_program(program())
}

fn selected_program(
    program: PortableExpressionProgram,
) -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    let contract =
        conduit_semantic_catalog::pure_expression_contract(&program, PortTemporal::Value).unwrap();
    let scope = BaseCapabilityScope {
        host_id: HostId::from("host/native"),
        boot_id: BootId::from("boot/native"),
        base_instance_id: BaseInstanceId::from("fixture"),
        base_provider_generation: 1,
        plan_id: PlanId::from("fixture"),
        active_play_id: ActivePlayId::from("fixture"),
        authority_grant_id: AuthorityGrantId::from("fixture"),
        authority_contract_id: AuthorityContractId::from("fixture"),
        capability_id: CapabilityId::from("fixture"),
        implementation_id: ImplementationId::from(IMPLEMENTATION),
        operation_contract_id: HostCallContractId::from(CALL),
        subject_kind: contract.kind_id.clone(),
        resource_pool_id: ResourcePoolId::from("fixture"),
        resource_generation_id: ResourceGenerationId("fixture".into()),
        envelope_id: CapabilityEnvelopeId::from("fixture"),
        maximum_parameter_bytes: 262144,
        maximum_result_bytes: 262144,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1000,
    };
    let (mut fragment, _, _, placement) = crate::machine_membrane::selection_fixture::selected(
        &scope,
        &contract,
        CALL,
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        "fixture",
        "expression",
    );
    let expected = offer(&program, PortTemporal::Value).unwrap();
    let gear = &mut fragment.placements[0];
    gear.limits = expected.limits;
    gear.host_calls = expected.host_calls;
    gear.capability_id = expected.capability_id;
    gear.execution_profile_id = expected.implementation.execution_profile_id;
    gear.artifact_id = expected.implementation.artifact_id;
    gear.configuration = vec![ConfigurationEntry {
        key: "program".into(),
        value: ConfigurationValue::Text(program.canonical_hex().unwrap()),
    }];
    gear.base = None;
    gear.resources.clear();
    gear.authority.clear();
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    let lowered = lower_plan_fragment(&fragment).unwrap();
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    (fragment, lowered, active, placement)
}

#[test]
fn native_checked_expression_is_exact_repeatable_and_cancelled_without_retry() {
    let (fragment, lowered, active, placement) = selected();
    let mut host = ExpressionHostCall::prepare(&fragment, &lowered, &active, &placement).unwrap();
    let node = host.node;
    assert_eq!(
        host.invoke(NodeId(node.0 + 1), HostCallId(0), RequestId(0), &[41]),
        Err(ExpressionCallRefusal::WrongBinding)
    );
    for request in 0..1000 {
        assert_eq!(
            host.invoke(node, HostCallId(0), RequestId(request), &[41])
                .unwrap(),
            &[42]
        );
    }
    assert_eq!(
        host.invoke(node, HostCallId(0), RequestId(999), &[41]),
        Err(ExpressionCallRefusal::StaleRequest)
    );
    assert!(matches!(
        host.invoke(node, HostCallId(0), RequestId(1000), &[255]),
        Err(ExpressionCallRefusal::Evaluation(_))
    ));
    assert_eq!(
        host.invoke(node, HostCallId(0), RequestId(1000), &[41]),
        Err(ExpressionCallRefusal::StaleRequest)
    );
    host.cancel(node, HostCallId(0)).unwrap();
    assert_eq!(
        host.invoke(node, HostCallId(0), RequestId(1001), &[41]),
        Err(ExpressionCallRefusal::Cancelled)
    );
}

#[test]
fn native_expression_refuses_stale_play_and_substituted_lowering() {
    let (fragment, mut lowered, mut active, placement) = selected();
    active.play_sequence += 1;
    assert!(matches!(
        ExpressionHostCall::prepare(&fragment, &lowered, &active, &placement),
        Err(ExpressionCallRefusal::WrongBinding)
    ));
    active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    lowered.identity.placements[0].0 = NodeId(7);
    assert!(matches!(
        ExpressionHostCall::prepare(&fragment, &lowered, &active, &placement),
        Err(ExpressionCallRefusal::WrongBinding)
    ));
}

mod kernel;

mod planned;

#[test]
fn empty_unit_frames_keep_one_admitted_queue_cell_and_execute() {
    let unit = StructuredInfoType::leaf(kind_id(UNIT_INFO_ID)).unwrap();
    let program = PortableExpressionProgram {
        input_type: unit.clone(),
        output_type: unit.clone(),
        root: PortableExpressionNode {
            value_type: unit,
            operation: PortableExpressionOperation::Input,
        },
    };
    let (fragment, lowered, active, placement) = selected_program(program);
    assert_eq!(fragment.placements[0].limits.max_queue_bytes, 1);
    let mut call = ExpressionHostCall::prepare(&fragment, &lowered, &active, &placement).unwrap();
    assert!(
        call.invoke(call.node, HostCallId(0), RequestId(0), &[])
            .unwrap()
            .is_empty()
    );
}
