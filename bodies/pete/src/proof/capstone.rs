//! Byte-identical portable observation-processing-motion capstone specimen.

use crate::{
    live_create_drive_advertisement, live_create_observation_advertisement, CreateDriveObservation,
    CreateObservationChannel, CreateObservationEvidence, CREATE_DEVICE_RESOURCE,
    CREATE_DRIVE_AUTHORITY, CREATE_DRIVE_OPERATION, CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY,
    CREATE_UART_BASE_RESOURCE,
};
use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, AuthorityContractId, AuthorityGrant,
    AuthorityGrantId, BaseImplementationId, CapabilityId, ExecutionProfileId, HostAdvertisement,
    HostOperationContractId, ImplementationId, ResourceHealth, ResourceObservation, SignId,
    SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};
use conduit_planner::{
    plan_selected_realizations_with_characteristics_and_authority, PlannerError,
    SelectedRealizationPlanning,
};
use std::collections::BTreeMap;

pub const PETE_CAPSTONE_FORM_NAME: &str = "pete-capstone";
pub const CAPSTONE_SERIALIZED_CLIENT_RESOURCE: &str =
    "pete.resource/create1-serialized-operation-client@1";
pub const CAPSTONE_WATCHDOG_RESOURCE: &str = "pete.resource/independent-watchdog@1";
pub const CAPSTONE_TRANSLATOR_RESOURCE: &str = "pete.resource/level-translator@1";
pub const CAPSTONE_STD_PROFILE: &str = "pete/std-create1-capstone@1";
pub const CAPSTONE_PICO_PROFILE: &str = "pete/pico-create1-capstone@1";
pub const CAPSTONE_STD_VELOCITY_IMPLEMENTATION: &str = "pete/std-velocity-intent@1";
pub const CAPSTONE_PICO_VELOCITY_IMPLEMENTATION: &str = "pete/pico-velocity-intent@1";
pub const CAPSTONE_STD_SELECT_IMPLEMENTATION: &str = "pete/std-state-select@1";
pub const CAPSTONE_PICO_SELECT_IMPLEMENTATION: &str = "pete/pico-state-select@1";
pub const CAPSTONE_ARTIFACT: &str = "conduit-pete/capstone-kernel@1";
pub const CAPSTONE_DRIVE_GRANT: &str = "grant/pete-capstone-drive";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapstoneHostClass {
    Std,
    PicoW,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapstoneHostEvidence {
    pub class: CapstoneHostClass,
    pub observation: CreateObservationEvidence,
    pub drive: CreateDriveObservation,
    pub serialized_client_pool_id: String,
    pub watchdog_pool_id: Option<String>,
    pub translator_pool_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapstoneAdvertisementRefusal {
    IdentityMismatch,
    MissingSerializedProvider,
    StdInventedEmbeddedResource,
    PicoMissingEmbeddedResource,
    ObservationUnavailable,
    DriveUnavailable,
}

pub fn capstone_advertisement(
    evidence: &CapstoneHostEvidence,
    now_tick: u64,
) -> Result<HostAdvertisement, CapstoneAdvertisementRefusal> {
    if evidence.observation.host_id != evidence.drive.host_id
        || evidence.observation.boot_id != evidence.drive.boot_id
        || evidence.observation.offer_generation != evidence.drive.offer_generation
        || evidence.observation.serial_base_id != evidence.drive.serial_base_id
        || evidence.observation.robot_identity != evidence.drive.robot_identity
    {
        return Err(CapstoneAdvertisementRefusal::IdentityMismatch);
    }
    if evidence.serialized_client_pool_id.is_empty() {
        return Err(CapstoneAdvertisementRefusal::MissingSerializedProvider);
    }
    match evidence.class {
        CapstoneHostClass::Std
            if evidence.watchdog_pool_id.is_some() || evidence.translator_pool_id.is_some() =>
        {
            return Err(CapstoneAdvertisementRefusal::StdInventedEmbeddedResource);
        }
        CapstoneHostClass::PicoW
            if evidence
                .watchdog_pool_id
                .as_deref()
                .is_none_or(str::is_empty)
                || evidence
                    .translator_pool_id
                    .as_deref()
                    .is_none_or(str::is_empty) =>
        {
            return Err(CapstoneAdvertisementRefusal::PicoMissingEmbeddedResource);
        }
        _ => {}
    }

    let observation = live_create_observation_advertisement(&evidence.observation, now_tick)
        .map_err(|_| CapstoneAdvertisementRefusal::ObservationUnavailable)?;
    let drive = live_create_drive_advertisement(&evidence.drive, now_tick)
        .map_err(|_| CapstoneAdvertisementRefusal::DriveUnavailable)?;
    let mut bump = observation
        .capabilities
        .into_iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str()
                == CreateObservationChannel::BumpAggregate.implementation_id()
        })
        .expect("live observation advertisement contains aggregate bump");
    let mut drive_offer = drive.capabilities[0].clone();

    bump.resource_requirements.retain(|requirement| {
        !matches!(
            requirement.class_id.as_str(),
            CREATE_UART_BASE_RESOURCE | CREATE_DEVICE_RESOURCE
        )
    });
    drive_offer.resource_requirements.retain(|requirement| {
        !matches!(
            requirement.class_id.as_str(),
            CREATE_UART_BASE_RESOURCE | CREATE_DEVICE_RESOURCE
        )
    });
    bump.resource_requirements
        .push(resource_requirement(CAPSTONE_SERIALIZED_CLIENT_RESOURCE, 1));
    // The planner currently seals one byte capacity across every Cord in a
    // fragment. The Boolean payload remains exactly one byte; this admits the
    // shared scalar-sized queue slot without changing its Info contract.
    bump.limits.max_queue_bytes = SCALAR_ENCODED_LEN as u32;
    drive_offer
        .resource_requirements
        .push(resource_requirement(CAPSTONE_SERIALIZED_CLIENT_RESOURCE, 1));
    if evidence.class == CapstoneHostClass::PicoW {
        drive_offer
            .resource_requirements
            .push(resource_requirement(CAPSTONE_WATCHDOG_RESOURCE, 1));
        drive_offer
            .resource_requirements
            .push(resource_requirement(CAPSTONE_TRANSLATOR_RESOURCE, 1));
    }
    bump.resource_requirements
        .sort_by(|left, right| left.class_id.cmp(&right.class_id));
    drive_offer
        .resource_requirements
        .sort_by(|left, right| left.class_id.cmp(&right.class_id));

    let (profile, velocity_implementation, select_implementation) = match evidence.class {
        CapstoneHostClass::Std => (
            CAPSTONE_STD_PROFILE,
            CAPSTONE_STD_VELOCITY_IMPLEMENTATION,
            CAPSTONE_STD_SELECT_IMPLEMENTATION,
        ),
        CapstoneHostClass::PicoW => (
            CAPSTONE_PICO_PROFILE,
            CAPSTONE_PICO_VELOCITY_IMPLEMENTATION,
            CAPSTONE_PICO_SELECT_IMPLEMENTATION,
        ),
    };
    let mut velocity = conduit_std_offers::robotics_velocity_intent_offer();
    velocity.capability_id = CapabilityId::from(format!("{profile}/velocity"));
    velocity.implementation.execution_profile_id = ExecutionProfileId::from(profile);
    velocity.implementation.implementation_id = ImplementationId::from(velocity_implementation);
    velocity.implementation.artifact_id = ArtifactId::from(CAPSTONE_ARTIFACT);
    let select = conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::state_select_scalar_contract(),
        conduit_semantic_catalog::STATE_SELECT_SCALAR_CONTRACT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: &format!("{profile}/state-select"),
            execution_profile: profile,
            implementation: select_implementation,
            artifact: CAPSTONE_ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let mut resources = observation.resources;
    for resource in drive.resources {
        if !resources.iter().any(|existing| {
            existing.pool_id == resource.pool_id && existing.class_id == resource.class_id
        }) {
            resources.push(resource);
        }
    }
    resources.push(resource_offer(
        &evidence.serialized_client_pool_id,
        CAPSTONE_SERIALIZED_CLIENT_RESOURCE,
        2,
    ));
    if let Some(pool) = &evidence.watchdog_pool_id {
        resources.push(resource_offer(pool, CAPSTONE_WATCHDOG_RESOURCE, 1));
    }
    if let Some(pool) = &evidence.translator_pool_id {
        resources.push(resource_offer(pool, CAPSTONE_TRANSLATOR_RESOURCE, 1));
    }
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));

    Ok(HostAdvertisement {
        protocol_version: observation.protocol_version,
        host_id: observation.host_id,
        boot_id: observation.boot_id,
        offer_generation: observation.offer_generation,
        profile: profile.into(),
        resources,
        capabilities: vec![bump, velocity, select, drive_offer],
        planner_capabilities: Vec::new(),
    })
}

pub fn capstone_plan(
    evidence: &CapstoneHostEvidence,
    now_tick: u64,
) -> Result<conduit_core::Plan, PlannerError> {
    let (_, profile) = crate::catalogs().expect("fixed Pete catalogs are valid");
    let checked = conduit_form::parse(PETE_CAPSTONE_FORM, &profile)
        .expect("capstone source is canonical and mechanism-free");
    let host = capstone_advertisement(evidence, now_tick).map_err(|error| {
        PlannerError::InvalidPlanningObservation(format!("capstone advertisement: {error:?}"))
    })?;
    let observations = ready_resources(&host);
    let authority_contract = if evidence.drive.safety.has_complete_independent_envelope() {
        CREATE_DRIVE_AUTHORITY
    } else {
        CREATE_DRIVE_REDUCED_SAFETY_AUTHORITY
    };
    let grant = AuthorityGrant {
        grant_id: AuthorityGrantId::from(CAPSTONE_DRIVE_GRANT),
        contract_id: AuthorityContractId::from(authority_contract),
        host_operation_contract_id: HostOperationContractId::from(CREATE_DRIVE_OPERATION),
        subject_kind: kind_id(SCALAR_INFO_ID),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: CapabilityId::from(crate::CREATE_DRIVE_CAPABILITY),
    };
    plan_selected_realizations_with_characteristics_and_authority(
        &checked,
        SelectedRealizationPlanning {
            hosts: &[host],
            bases: &[BaseImplementationId::from("conduit.base/local@1")],
            requirements: &BTreeMap::new(),
            advertisements: &[],
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: SCALAR_ENCODED_LEN as u32,
            authority_grants: &[grant],
        },
    )
}

fn ready_resources(host: &HostAdvertisement) -> Vec<ResourceObservation> {
    host.resources
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
            sign_id: SignId::from(format!("capstone-resource-{index}")),
        })
        .collect()
}

pub const PETE_CAPSTONE_FORM: &str = r#"form pete-capstone {
    bump: robotics/observe-bump
    requested: robotics/velocity-intent(linear-microunits = 100000, angular-microunits = 0)
    stopped: robotics/velocity-intent(linear-microunits = 0, angular-microunits = 0)
    safe_linear: state/select
    drive: robotics/drive-differential(ttl-ms = 250)

    bump.observation > safe_linear.selector
    requested.linear > safe_linear.when-false
    stopped.linear > safe_linear.when-true
    safe_linear.out > drive.linear
    requested.angular > drive.angular
}
"#;

#[cfg(test)]
mod tests;
