//! Exact authority-gated Create docking realization.

use std::collections::BTreeMap;

use conduit_core::{
    authority_grant, kind_id, resource_offer, resource_requirement, ArtifactId,
    AuthorityContractId, AuthorityRequirement, BaseImplementationId, BootId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, FaceStartupParameter, HostAdvertisement,
    HostId, HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    ImplementationOffer, KindContractRevision, OfferGeneration, ResourceHealth,
    ResourceObservation, SignId, BOOL_ENCODED_LEN, BOOL_INFO_ID, PROTOCOL_VERSION,
    TIMER_RESOURCE_CLASS,
};
use conduit_planner::{
    plan_selected_realizations_with_characteristics_and_authority, PlannerError,
    SelectedRealizationPlanning,
};

use crate::{
    IndependentWatchdogObservation, LocalHazard, OiMode, SafetyObservation, CREATE_DEVICE_RESOURCE,
    CREATE_UART_BASE_RESOURCE,
};

pub const CREATE_DOCK_FORM: &str = r#"form seek_dock {
    request: state/toggle(initial = true)
    dock: robotics/dock(timeout-ms = 30000)
    request.value > dock.request
}
"#;

pub const CREATE_DOCK_CAPABILITY: &str = "pete/create1/dock";
pub const CREATE_DOCK_IMPLEMENTATION: &str = "pete/create1-seek-dock@1";
pub const CREATE_DOCK_ARTIFACT: &str = "conduit-pete/create1-dock@1";
pub const CREATE_DOCK_OPERATION: &str = "pete.host/create1-dock@1";
pub const CREATE_DOCK_AUTHORITY: &str = "pete.authority/create1-dock@1";
pub const CREATE_DOCK_REDUCED_SAFETY_AUTHORITY: &str =
    "pete.authority/create1-dock-no-independent-watchdog@1";
pub const CREATE_DOCK_PROFILE: &str = "pete/create1-dock@1";
pub const CREATE_DOCK_REDUCED_SAFETY_PROFILE: &str = "pete/create1-dock-no-independent-watchdog@1";
pub const CREATE_DOCK_RESOURCE: &str = "pete.resource/create1-dock@1";
pub const CREATE_DOCK_GRANT: &str = "grant/pete-create1-dock";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateDockObservation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub robot_identity_verified: bool,
    pub dock_resource_id: String,
    pub timer_resource_id: String,
    pub mode: OiMode,
    pub safety: SafetyObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateDockOfferRefusal {
    MissingIdentity,
    UnverifiedIdentity,
    UnsupportedMode,
    InvalidFreshness,
    MissingSafetyEnvelope,
    SafetyStaleOrInhibited(LocalHazard),
}

pub fn live_create_dock_advertisement(
    observation: &CreateDockObservation,
    now_tick: u64,
) -> Result<HostAdvertisement, CreateDockOfferRefusal> {
    if observation.serial_base_id.is_empty()
        || observation.robot_identity.is_empty()
        || observation.dock_resource_id.is_empty()
        || observation.timer_resource_id.is_empty()
    {
        return Err(CreateDockOfferRefusal::MissingIdentity);
    }
    if !observation.robot_identity_verified {
        return Err(CreateDockOfferRefusal::UnverifiedIdentity);
    }
    if !matches!(observation.mode, OiMode::Safe | OiMode::Full) {
        return Err(CreateDockOfferRefusal::UnsupportedMode);
    }
    if observation.safety.maximum_age_ticks == 0 {
        return Err(CreateDockOfferRefusal::InvalidFreshness);
    }
    if observation.safety.latch_generation == 0 {
        return Err(CreateDockOfferRefusal::MissingSafetyEnvelope);
    }
    if let Some(hazard) = observation.safety.first_hazard(now_tick) {
        return Err(CreateDockOfferRefusal::SafetyStaleOrInhibited(hazard));
    }

    let (profile, authority) = if observation.safety.has_complete_independent_envelope() {
        (CREATE_DOCK_PROFILE, CREATE_DOCK_AUTHORITY)
    } else {
        match observation.safety.independent_watchdog {
            IndependentWatchdogObservation::Failed => unreachable!("failed watchdog is a hazard"),
            IndependentWatchdogObservation::Absent | IndependentWatchdogObservation::Healthy => (
                CREATE_DOCK_REDUCED_SAFETY_PROFILE,
                CREATE_DOCK_REDUCED_SAFETY_AUTHORITY,
            ),
        }
    };
    let contract = conduit_semantic_catalog::robotics_dock_contract();
    let mut resources = vec![
        resource_offer(&observation.serial_base_id, CREATE_UART_BASE_RESOURCE, 1),
        resource_offer(&observation.robot_identity, CREATE_DEVICE_RESOURCE, 1),
        resource_offer(&observation.dock_resource_id, CREATE_DOCK_RESOURCE, 1),
        resource_offer(&observation.timer_resource_id, TIMER_RESOURCE_CLASS, 1),
    ];
    resources.sort();
    let mut requirements = vec![
        resource_requirement(CREATE_UART_BASE_RESOURCE, 1),
        resource_requirement(CREATE_DEVICE_RESOURCE, 1),
        resource_requirement(CREATE_DOCK_RESOURCE, 1),
        resource_requirement(TIMER_RESOURCE_CLASS, 1),
    ];
    requirements.sort();
    let authority_requirement = AuthorityRequirement {
        contract_id: AuthorityContractId::from(authority),
        host_operation_contract_id: HostOperationContractId::from(CREATE_DOCK_OPERATION),
        subject_kind: kind_id(BOOL_INFO_ID),
    };
    let dock = CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "timeout-ms".into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(CREATE_DOCK_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::ROBOTICS_DOCK_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(CREATE_DOCK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(CREATE_DOCK_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(CREATE_DOCK_OPERATION),
            target_kind: Some(kind_id(BOOL_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: BOOL_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: requirements,
        authority_requirements: vec![authority_requirement],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: BOOL_ENCODED_LEN as u32,
        },
    };
    let mut toggle = conduit_std_offers::state_toggle_offer();
    toggle.capability_id = CapabilityId::from("pete/create1-dock-request@1");
    let mut capabilities = vec![toggle, dock];
    capabilities.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    Ok(HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: observation.host_id.clone(),
        boot_id: observation.boot_id.clone(),
        offer_generation: observation.offer_generation,
        profile: HostProfileId::from(profile),
        resources,
        capabilities,
        planner_capabilities: Vec::new(),
    })
}

pub fn create_dock_plan(
    observation: &CreateDockObservation,
    now_tick: u64,
    authority_granted: bool,
) -> Result<conduit_core::Plan, PlannerError> {
    let (_, profile) = crate::catalogs().expect("fixed Pete catalogs are valid");
    let form = conduit_form::parse(CREATE_DOCK_FORM, &profile)
        .expect("canonical dock Form checks independently of mechanism facts");
    let host = live_create_dock_advertisement(observation, now_tick)
        .expect("caller supplies one fresh usable dock observation");
    let authority_contract = if observation.safety.has_complete_independent_envelope() {
        CREATE_DOCK_AUTHORITY
    } else {
        CREATE_DOCK_REDUCED_SAFETY_AUTHORITY
    };
    let requirement = AuthorityRequirement {
        contract_id: AuthorityContractId::from(authority_contract),
        host_operation_contract_id: HostOperationContractId::from(CREATE_DOCK_OPERATION),
        subject_kind: kind_id(BOOL_INFO_ID),
    };
    let grant = authority_granted.then(|| {
        authority_grant(
            CREATE_DOCK_GRANT,
            &requirement,
            host.host_id.clone(),
            host.boot_id.clone(),
            CapabilityId::from(CREATE_DOCK_CAPABILITY),
        )
    });
    let observations = host
        .resources
        .iter()
        .enumerate()
        .map(|(index, pool)| ResourceObservation {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            pool_id: pool.pool_id.clone(),
            class_id: pool.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: pool.capacity_units,
            utilized_units: 0,
            sign_id: SignId::from(format!("create-dock-resource-{index}")),
        })
        .collect::<Vec<_>>();
    let hosts = [host];
    plan_selected_realizations_with_characteristics_and_authority(
        &form,
        SelectedRealizationPlanning {
            hosts: &hosts,
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &BTreeMap::new(),
            advertisements: &[],
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: BOOL_ENCODED_LEN as u32,
            authority_grants: grant.as_slice(),
        },
    )
}

#[cfg(test)]
#[path = "create_dock_tests.rs"]
mod tests;
