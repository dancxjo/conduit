//! Live physical Create differential-drive offer and authority-gated planning seam.

use crate::{IndependentWatchdogObservation, LocalHazard, OiMode, SafetyObservation};
use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, AuthorityContractId,
    AuthorityRequirement, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FaceStartupParameter, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    OfferGeneration, PROTOCOL_VERSION, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};

pub const CREATE_DRIVE_PROFILE: &str = "pete/create1-differential-drive@1";
pub const CREATE_DRIVE_REDUCED_SAFETY_PROFILE: &str =
    "pete/create1-differential-drive-no-independent-watchdog@1";
pub const CREATE_DRIVE_CAPABILITY: &str = "pete/create1/differential-drive";
pub const CREATE_DRIVE_IMPLEMENTATION: &str = "pete/create1-drive-direct@1";
pub const CREATE_DRIVE_ARTIFACT: &str = "conduit-pete/create1-drive@1";
pub const CREATE_DRIVE_OPERATION: &str = "pete.host/create1-differential-drive@1";
pub const CREATE_DRIVE_AUTHORITY: &str = "pete.authority/create1-motion@1";
pub const CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY: &str =
    "pete.authority/create1-motion-no-independent-watchdog@1";
pub const CREATE_DRIVE_RESOURCE: &str = "pete.resource/create1-drive@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateDriveObservation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub drive_resource_id: String,
    pub mode: OiMode,
    pub safety: SafetyObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateDriveOfferRefusal {
    MissingIdentity,
    UnsupportedMode,
    InvalidFreshness,
    MissingSafetyEnvelope,
    SafetyStaleOrInhibited(LocalHazard),
}

pub fn live_create_drive_advertisement(
    observation: &CreateDriveObservation,
    now_tick: u64,
) -> Result<HostAdvertisement, CreateDriveOfferRefusal> {
    if observation.serial_base_id.is_empty()
        || observation.robot_identity.is_empty()
        || observation.drive_resource_id.is_empty()
    {
        return Err(CreateDriveOfferRefusal::MissingIdentity);
    }
    if !matches!(observation.mode, OiMode::Safe | OiMode::Full) {
        return Err(CreateDriveOfferRefusal::UnsupportedMode);
    }
    if observation.safety.maximum_age_ticks == 0 {
        return Err(CreateDriveOfferRefusal::InvalidFreshness);
    }
    if observation.safety.latch_generation == 0 {
        return Err(CreateDriveOfferRefusal::MissingSafetyEnvelope);
    }
    if let Some(hazard) = observation.safety.first_hazard(now_tick) {
        return Err(CreateDriveOfferRefusal::SafetyStaleOrInhibited(hazard));
    }
    let (profile, authority_contract) = if observation.safety.has_complete_independent_envelope() {
        (CREATE_DRIVE_PROFILE, CREATE_DRIVE_AUTHORITY)
    } else {
        match observation.safety.independent_watchdog {
            IndependentWatchdogObservation::Failed => unreachable!("failed watchdog is a hazard"),
            IndependentWatchdogObservation::Absent | IndependentWatchdogObservation::Healthy => (
                CREATE_DRIVE_REDUCED_SAFETY_PROFILE,
                CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY,
            ),
        }
    };

    let contract = conduit_semantic_catalog::robotics_drive_differential_contract();
    let mut resources = vec![
        resource_offer(
            &observation.serial_base_id,
            crate::CREATE_UART_BASE_RESOURCE,
            1,
        ),
        resource_offer(
            &observation.robot_identity,
            crate::CREATE_DEVICE_RESOURCE,
            1,
        ),
        resource_offer(&observation.drive_resource_id, CREATE_DRIVE_RESOURCE, 1),
    ];
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let mut requirements = vec![
        resource_requirement(crate::CREATE_UART_BASE_RESOURCE, 1),
        resource_requirement(crate::CREATE_DEVICE_RESOURCE, 1),
        resource_requirement(CREATE_DRIVE_RESOURCE, 1),
    ];
    requirements.sort_by(|left, right| left.class_id.cmp(&right.class_id));
    Ok(HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: observation.host_id.clone(),
        boot_id: observation.boot_id.clone(),
        offer_generation: observation.offer_generation,
        profile: conduit_core::HostProfileId::from(profile),
        resources,
        capabilities: vec![CapabilityOffer {
            startup_parameters: vec![FaceStartupParameter {
                name: "ttl-ms".into(),
                value_type: "Count".into(),
                has_default: true,
            }],
            shorthand: None,
            capability_id: CapabilityId::from(CREATE_DRIVE_CAPABILITY),
            kind_id: contract.kind_id,
            kind_contract_revision: KindContractRevision::from(
                conduit_semantic_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_REVISION,
            ),
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(profile),
                implementation_id: ImplementationId::from(CREATE_DRIVE_IMPLEMENTATION),
                artifact_id: ArtifactId::from(CREATE_DRIVE_ARTIFACT),
            },
            inputs: contract.inputs,
            outputs: contract.outputs,
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(CREATE_DRIVE_OPERATION),
                target_kind: Some(kind_id(SCALAR_INFO_ID)),
                maximum_in_flight: 1,
                maximum_input_bytes: (2 * SCALAR_ENCODED_LEN) as u32,
                maximum_output_bytes: 0,
            }],
            resource_requirements: requirements,
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(authority_contract),
                host_operation_contract_id: HostOperationContractId::from(CREATE_DRIVE_OPERATION),
                subject_kind: kind_id(SCALAR_INFO_ID),
            }],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 2,
                max_queue_bytes: (2 * SCALAR_ENCODED_LEN) as u32,
            },
        }],
        planner_capabilities: Vec::new(),
    })
}

#[cfg(test)]
#[path = "create_drive_offer_tests.rs"]
mod tests;
