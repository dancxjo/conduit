//! Proof preparation binds actual native identities to the checked control Source.
//! The grant is explicit proof-appliance authority, never discovered permission.
use super::*;
use alloc::{collections::BTreeMap, vec};
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions, default_expanded_placements,
    plan_expanded_authoring_with_options,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
pub(super) fn plan(
    contract: &ControlContract,
    ids: &BootIdentities,
    base: &[u8; 32],
    device: &UsbDevice,
) -> Result<Plan, &'static str> {
    let (startup, profile) = contract.catalogs();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../../../plots/usb/control.conduit")),
        &startup,
    )
    .map_err(|_| "usb-control-proof-planning")?;
    let authoring = expand_canonical_plot_for_authoring(&checked, "usb-control-call", &profile)
        .map_err(|_| "usb-control-proof-planning")?;
    let kind = contract.kind();
    let offer = conduit_core::capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters: kind.startup_parameters.clone(), shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from(CONTROL_IMPLEMENTATION),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: CONTROL_PROFILE.into(), implementation_id: CONTROL_IMPLEMENTATION.into(), artifact_id: "conduitos/usb-control-kernel-proof@1".into(),
        },
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement { contract_id: CONTROL_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: CONTROL_MAXIMUM_BYTES, maximum_output_bytes: CONTROL_MAXIMUM_BYTES }],
        resource_requirements: vec![resource_requirement(CONTROL_ATTACHMENT, 1)],
        authority_requirements: vec![AuthorityRequirement {contract_id: CONTROL_AUTHORITY.into(), host_call_contract_id: CONTROL_CALL.into(), subject_kind: kind.kind_id.clone()}],
        limits: kind.limits.clone(),
    };
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity::hex(&ids.host).into(),
        boot_id: identity::hex(&ids.boot).into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/usb-control-kernel-proof@1".into(),
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
    .map_err(|_| "usb-control-proof-planning")?;
    registry
        .register(BaseProviderEntry {
            base_id: identity::hex(base).into(),
            provider_instance_id: identity::hex(&identity::derive_usb_device(
                &ids.boot,
                base,
                device.root_port,
                device.slot,
                device.attachment_epoch,
            ))
            .into(),
            provider_generation: u64::from(device.attachment_epoch),
            implementation_id: CONTROL_BASE.into(),
            mechanism_family: CONTROL_ATTACHMENT.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![resource_offer(
                &alloc::format!(
                    "usb-control-dma/{}/{}/{}",
                    device.root_port,
                    device.slot,
                    device.attachment_epoch
                ),
                CONTROL_ATTACHMENT,
                1,
            )],
        })
        .map_err(|_| "usb-control-proof-planning")?;
    registry
        .project_ready_into(&mut host)
        .map_err(|_| "usb-control-proof-planning")?;
    let grants = [AuthorityGrant {
        grant_id: "conduitos.proof/usb-control-explicit-grant@1".into(),
        contract_id: CONTROL_AUTHORITY.into(),
        host_call_contract_id: CONTROL_CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: CONTROL_IMPLEMENTATION.into(),
    }];
    let hosts = [host];
    let placements = default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|_| "usb-control-proof-planning")?;
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
    Ok(plan_expanded_authoring_with_options(
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
    .map_err(|_| "usb-control-proof-planning")?)
}
