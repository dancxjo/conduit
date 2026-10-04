//! Inert planning fixtures never advertise a native controller to production.
use super::*;
use alloc::{collections::BTreeMap, vec};
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions, default_expanded_placements,
    plan_expanded_authoring_with_options,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};

fn planned() -> Plan {
    let contract = ControlContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../../plots/usb/control.conduit")),
        &startup,
    )
    .unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "usb-control-call", &profile).unwrap();
    let kind = contract.kind();
    let offer = conduit_core::capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters: kind.startup_parameters.clone(), shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from(CONTROL_IMPLEMENTATION),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: CONTROL_PROFILE.into(), implementation_id: CONTROL_IMPLEMENTATION.into(), artifact_id: "fixture/usb-control".into(),
        },
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement { contract_id: CONTROL_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: CONTROL_MAXIMUM_BYTES, maximum_output_bytes: CONTROL_MAXIMUM_BYTES }],
        resource_requirements: vec![resource_requirement(CONTROL_ATTACHMENT, 1)],
        authority_requirements: vec![AuthorityRequirement {contract_id: CONTROL_AUTHORITY.into(), host_call_contract_id: CONTROL_CALL.into(), subject_kind: kind.kind_id.clone()}],
        limits: kind.limits.clone(),
    };
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/usb-fixture".into(),
        boot_id: "boot/usb-fixture".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/usb-control".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut registry = BaseRegistry::new(BaseRegistryLimits {
        maximum_bases: 1,
        maximum_capabilities_per_base: 1,
        maximum_resources_per_base: 1,
        maximum_advertised_capabilities: 1,
        maximum_advertised_resources: 1,
    })
    .unwrap();
    registry
        .register(BaseProviderEntry {
            base_id: "fixture/usb-base".into(),
            provider_instance_id: "fixture/usb-provider".into(),
            provider_generation: 1,
            implementation_id: CONTROL_BASE.into(),
            mechanism_family: CONTROL_ATTACHMENT.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![resource_offer(
                "fixture/usb-resource",
                CONTROL_ATTACHMENT,
                1,
            )],
        })
        .unwrap();
    registry.project_ready_into(&mut host).unwrap();
    let grants = [AuthorityGrant {
        grant_id: "fixture/usb-grant".into(),
        contract_id: CONTROL_AUTHORITY.into(),
        host_call_contract_id: CONTROL_CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: CONTROL_IMPLEMENTATION.into(),
    }];
    let hosts = [host];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundaries = authoring
        .input_bindings
        .iter()
        .map(|binding| (PortDirection::Input, binding))
        .chain(
            authoring
                .output_bindings
                .iter()
                .map(|binding| (PortDirection::Output, binding)),
        )
        .map(|(direction, binding)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: CONTROL_MAXIMUM_BYTES,
                },
            )
        })
        .collect();
    plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: CONTROL_MAXIMUM_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap()
}

#[test]
fn checked_control_source_selects_a_finite_kernel_back() {
    let plan = planned();
    assert!(verify_plan(&plan));
    let gear = &plan.fragments[0].placements[0];
    let factory = ControlOperationFactory::prepare_contract().unwrap();
    let budget = factory.budget(gear).unwrap();
    assert_eq!(budget.host_requests, 1);
    assert_eq!(budget.value_items, 4);
    assert_eq!(budget.maximum_value_bytes, CONTROL_MAXIMUM_BYTES);
    let mut values = HostedValueStore::new(
        budget.value_items,
        budget.maximum_value_bytes,
        budget.value_bytes,
    )
    .unwrap();
    assert!(factory.prepare(gear, &mut values).is_ok());
}

#[test]
fn altered_realizations_are_refused_before_kernel_preparation() {
    let plan = planned();
    let original = &plan.fragments[0].placements[0];
    let factory = ControlOperationFactory::prepare_contract().unwrap();
    let changes: &[fn(&mut PlannedGear)] = &[
        |g| g.execution_profile_id = "fixture/wrong-profile".into(),
        |g| g.implementation_id = "fixture/wrong-back".into(),
        |g| g.capability_id = "fixture/wrong-possession".into(),
        |g| g.kind_contract_revision = "fixture/wrong-contract".into(),
        |g| g.inputs.clear(),
        |g| g.outputs.clear(),
        |g| g.host_calls[0].maximum_in_flight = 2,
        |g| g.host_calls[0].maximum_input_bytes += 1,
        |g| g.host_calls[0].maximum_output_bytes += 1,
        |g| g.host_calls[0].target_kind = None,
        |g| g.host_calls.push(g.host_calls[0].clone()),
        |g| g.base = None,
        |g| g.base.as_mut().unwrap().implementation_id = "fixture/wrong-base".into(),
        |g| g.base.as_mut().unwrap().mechanism_family = "fixture/wrong-attachment".into(),
        |g| g.resources.clear(),
        |g| g.resources[0].units = 2,
        |g| g.authority.clear(),
        |g| g.authority[0].boot_id = "boot/stale".into(),
        |g| g.authority[0].host_id = "host/other".into(),
        |g| g.authority[0].capability_id = "fixture/other".into(),
        |g| g.limits.max_queue_bytes += 1,
    ];
    for change in changes {
        let mut gear = original.clone();
        change(&mut gear);
        assert!(factory.budget(&gear).is_err());
        let mut values =
            HostedValueStore::new(4, CONTROL_MAXIMUM_BYTES, CONTROL_MAXIMUM_BYTES * 4).unwrap();
        assert!(factory.prepare(&gear, &mut values).is_err());
    }
}

mod execution;
mod possession;
