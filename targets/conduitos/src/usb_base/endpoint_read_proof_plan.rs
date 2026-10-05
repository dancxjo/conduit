//! Proof preparation binds actual native identities to the checked endpoint Source.
//! The grant is explicit proof-appliance authority, never discovered permission.
use super::{
    endpoint_read_contract::{
        ENDPOINT_READ_CALL, ENDPOINT_READ_MAXIMUM_BYTES, EndpointReadContract,
    },
    endpoint_read_factory::*,
};
use alloc::{collections::BTreeMap, vec};
use conduit_core::*;
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions, default_expanded_placements,
    plan_expanded_authoring_with_options,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
};
/// Finite proof-root workload, with Sign history admitted before Play.
pub const ENDPOINT_READ_PROOF_TRANSFERS: u16 = 128;
pub const ENDPOINT_READ_PROOF_SIGN_STORAGE: conduit_composite::KernelCompositeSignStorage =
    conduit_composite::KernelCompositeSignStorage {
        additional_local_items: 4096,
        additional_remote_items: 4096,
    };

/// Observed identities supplied by the proof root; these data grant no authority.
pub struct EndpointReadProofSubject<'a> {
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub controller_base_id: &'a str,
    pub device_instance_id: &'a str,
    pub root_port: u8,
    pub slot: u8,
    pub attachment_epoch: u32,
    pub endpoint_dci: u8,
    pub endpoint_epoch: u64,
}

/// One explicit proof-appliance recipe shared by guest execution and host verification.
/// Constructing its advertisement or Plan performs no physical effects.
pub fn plan(
    contract: &EndpointReadContract,
    subject: &EndpointReadProofSubject<'_>,
) -> Result<Plan, &'static str> {
    if subject.root_port == 0
        || subject.slot == 0
        || subject.attachment_epoch == 0
        || subject.endpoint_epoch == 0
        || !(3..=31).contains(&subject.endpoint_dci)
        || subject.endpoint_dci & 1 == 0
    {
        return Err("usb-endpoint-read-proof-subject");
    }
    let (startup, profile) = contract.catalogs();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("../../plots/usb/endpoint-read.conduit")),
        &startup,
    )
    .map_err(|_| "usb-endpoint-read-proof-planning")?;
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "usb-endpoint-read-call", &profile)
            .map_err(|_| "usb-endpoint-read-proof-planning")?;
    let (host, grants) = host_and_grants(contract, subject)?;
    let hosts = [host];
    let placements = default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|_| "usb-endpoint-read-proof-planning")?;
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
                    byte_capacity: ENDPOINT_READ_MAXIMUM_BYTES,
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
            connection_byte_capacity: ENDPOINT_READ_MAXIMUM_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|_| "usb-endpoint-read-proof-planning")
}

/// Explicit fixture-root advertisement and grants, shared by checked USB proofs.
/// No discovery or Source metadata is treated as possession.
pub(super) fn host_and_grants(
    contract: &EndpointReadContract,
    subject: &EndpointReadProofSubject<'_>,
) -> Result<(HostAdvertisement, [AuthorityGrant; 1]), &'static str> {
    let kind = contract.kind();
    let offer = conduit_core::capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters: kind.startup_parameters.clone(), shorthand: kind.shorthand.clone(),
        capability_id: CapabilityId::from(ENDPOINT_READ_IMPLEMENTATION),
        kind_id: kind.kind_id.clone(), kind_contract_revision: kind.kind_contract_revision.clone(),
        implementation: ImplementationOffer {
            execution_profile_id: ENDPOINT_READ_PROFILE.into(), implementation_id: ENDPOINT_READ_IMPLEMENTATION.into(), artifact_id: "conduitos/usb-endpoint-read-kernel-proof@1".into(),
        },
        inputs: kind.inputs.clone(), outputs: kind.outputs.clone(),
        host_calls: vec![HostCallRequirement { contract_id: ENDPOINT_READ_CALL.into(), target_kind: Some(kind.kind_id.clone()), maximum_in_flight: 1, maximum_input_bytes: ENDPOINT_READ_MAXIMUM_BYTES, maximum_output_bytes: ENDPOINT_READ_MAXIMUM_BYTES }],
        resource_requirements: vec![resource_requirement(ENDPOINT_READ_ATTACHMENT, 1)],
        authority_requirements: vec![AuthorityRequirement {contract_id: ENDPOINT_READ_AUTHORITY.into(), host_call_contract_id: ENDPOINT_READ_CALL.into(), subject_kind: kind.kind_id.clone()}],
        limits: kind.limits.clone(),
    };
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: subject.host_id.into(),
        boot_id: subject.boot_id.into(),
        offer_generation: OfferGeneration(1),
        profile: "conduitos/usb-endpoint-read-kernel-proof@1".into(),
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
    .map_err(|_| "usb-endpoint-read-proof-planning")?;
    registry
        .register(BaseProviderEntry {
            base_id: subject.controller_base_id.into(),
            provider_instance_id: subject.device_instance_id.into(),
            provider_generation: u64::from(subject.attachment_epoch),
            implementation_id: ENDPOINT_READ_BASE.into(),
            mechanism_family: ENDPOINT_READ_ATTACHMENT.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![resource_offer(
                &alloc::format!(
                    "usb-endpoint-read-dma/{}/{}/{}/{}/{}",
                    subject.root_port,
                    subject.slot,
                    subject.attachment_epoch,
                    subject.endpoint_dci,
                    subject.endpoint_epoch
                ),
                ENDPOINT_READ_ATTACHMENT,
                1,
            )],
        })
        .map_err(|_| "usb-endpoint-read-proof-planning")?;
    registry
        .project_ready_into(&mut host)
        .map_err(|_| "usb-endpoint-read-proof-planning")?;
    let grants = [AuthorityGrant {
        grant_id: "conduitos.proof/usb-endpoint-read-explicit-grant@1".into(),
        contract_id: ENDPOINT_READ_AUTHORITY.into(),
        host_call_contract_id: ENDPOINT_READ_CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: ENDPOINT_READ_IMPLEMENTATION.into(),
    }];
    Ok((host, grants))
}
