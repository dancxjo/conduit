use super::*;
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
};
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::{LoweredPlanFragment, lower_plan_fragment};
use conduit_plot::*;
fn program() -> PortableExpressionProgram {
    let syntax = parse_syntax_document(
        "plot add (\n input: U8 >> output: Bool\n) {\n input >> (. < 2) >> output\n}\n",
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

fn selected_program(
    program: PortableExpressionProgram,
) -> (
    PlanFragment,
    LoweredPlanFragment,
    ActivePlayIdentity,
    PlacementId,
) {
    let contract = conduit_semantic_catalog::pure_filter_contract(
        &program,
        PortTemporal::Flow { closes: true },
    )
    .unwrap();
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
    let expected = offer(&program, PortTemporal::Flow { closes: true }).unwrap();
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
fn target_filter_binds_exact_plan_node_request_and_active_play() {
    let (fragment, lowered, active, placement) = selected_program(program());
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let plan = seal_plan(identity.clone(), vec![fragment.clone()]);
    let factory = PureFilterOperationFactory::for_plan(&plan).unwrap();
    let node = lowered.identity.placements[0].0;
    let mut host =
        FilterHostCall::prepare(&factory, &fragment, &lowered, &active, &placement).unwrap();
    assert_eq!(
        host.invoke(NodeId(node.0 + 1), HostCallId(0), RequestId(0), &[1]),
        Err(FilterCallRefusal::WrongBinding)
    );
    assert_eq!(
        host.invoke(node, HostCallId(0), RequestId(0), &[1])
            .unwrap(),
        Some([1].as_slice())
    );
    assert_eq!(
        host.invoke(node, HostCallId(0), RequestId(1), &[2])
            .unwrap(),
        None
    );
    assert_eq!(
        host.invoke(node, HostCallId(0), RequestId(1), &[1]),
        Err(FilterCallRefusal::StaleRequest)
    );
    assert!(matches!(
        host.invoke(node, HostCallId(0), RequestId(2), &[]),
        Err(FilterCallRefusal::Evaluation(_))
    ));
    host.cancel(node, HostCallId(0)).unwrap();
    assert_eq!(
        host.invoke(node, HostCallId(0), RequestId(3), &[1]),
        Err(FilterCallRefusal::Cancelled)
    );
    let mut foreign_active = active.clone();
    foreign_active.boot_id = "foreign".into();
    assert!(
        FilterHostCall::prepare(&factory, &fragment, &lowered, &foreign_active, &placement)
            .is_err()
    );
    let mut other_identity = identity;
    other_identity.source_document_id = "foreign-source".into();
    let other = seal_plan(other_identity, vec![fragment])
        .fragments
        .remove(0);
    let other_lowered = lower_plan_fragment(&other).unwrap();
    let other_active = bind_active_play(&other.plan_id, &other.host_id, &other.boot_id, 0);
    assert!(
        FilterHostCall::prepare(&factory, &other, &other_lowered, &other_active, &placement)
            .is_err()
    );
}

#[test]
fn prepared_value_filter_preserves_optional_presence_and_refuses_current() {
    use conduit_semantic_catalog::operation_owners::pure_expression_host::PreparedPureExpressionHost;
    let program = program();
    let output_type = optional_info_type(program.input_type.clone()).unwrap();
    let mut host =
        PreparedPureExpressionHost::prepare(&program, PortTemporal::Value, true).unwrap();
    let selected =
        StructuredInfoValue::from_canonical_bytes(host.execute_filter(&[1]).unwrap().unwrap())
            .unwrap();
    assert_eq!(selected.value_type(), &output_type);
    let StructuredInfoValueShape::Variant { tag, payload } = selected.shape() else {
        panic!("optional variant")
    };
    assert_eq!(tag, "some");
    assert!(matches!(payload.shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == [1]));
    let dropped =
        StructuredInfoValue::from_canonical_bytes(host.execute_filter(&[2]).unwrap().unwrap())
            .unwrap();
    assert_eq!(dropped.value_type(), &output_type);
    assert!(matches!(
        dropped.shape(),
        StructuredInfoValueShape::Variant { tag: "none", .. }
    ));
    assert!(PreparedPureExpressionHost::prepare(&program, PortTemporal::Current, true).is_err());
}
