//! Exact MPU-6050 realization of portable body orientation.

use std::collections::BTreeMap;

use conduit_core::{
    resource_offer, resource_requirement, ArtifactId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConnectionBase, ExecutionProfileId, FaceStartupParameter, HostAdvertisement,
    HostId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, OfferGeneration, OrientationObservation,
    ResourceHealth, ResourceObservation, SignId, PROTOCOL_VERSION,
    ROBOTICS_ORIENTATION_ENCODED_LEN,
};
use conduit_mpu6050::{
    DerivationFailure, DerivedImuObservation, GravityCalibration, ImuDeriver, ImuThresholds,
    Mpu6050Failure, Mpu6050I2cProvider, Mpu6050Session, RawImuSample,
};
use conduit_planner::{
    plan_selected_realizations_with_characteristics_and_authority, PlannerError,
    SelectedRealizationPlanning,
};

pub const MPU6050_PROFILE: &str = "pete/mpu6050-observation@1";
pub const MPU6050_CAPABILITY: &str = "pete/mpu6050/orientation";
pub const MPU6050_IMPLEMENTATION: &str = "pete/mpu6050-observe-imu@1";
pub const MPU6050_ARTIFACT: &str = "conduit-pete/mpu6050-observation@1";
pub const MPU6050_OPERATION: &str = "pete.host/mpu6050-observe-imu@1";
pub const I2C_BASE_RESOURCE: &str = "pete.resource/i2c-base@1";
pub const MPU6050_ATTACHMENT_RESOURCE: &str = "pete.resource/mpu6050-attachment@1";
pub const MPU6050_SESSION_RESOURCE: &str = "pete.resource/mpu6050-session@1";

pub const MPU6050_FORM: &str = r#"form pete_imu {
    imu: robotics/observe-imu
}
"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mpu6050Evidence {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub i2c_base_id: String,
    pub attachment_id: String,
    pub session_resource_id: String,
    pub body_frame_id: String,
    pub mounting_id: String,
    pub address: u8,
    pub calibration: GravityCalibration,
    pub thresholds: ImuThresholds,
    pub observed_at_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mpu6050OfferRefusal {
    MissingIdentity,
    InvalidAddress,
    InvalidFreshness,
    StaleEvidence,
    InvalidCalibration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mpu6050ExecutionFailure {
    Device(Mpu6050Failure),
    Derivation(DerivationFailure),
    PortableValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mpu6050Snapshot {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub i2c_base_id: String,
    pub attachment_id: String,
    pub body_frame_id: String,
    pub mounting_id: String,
    pub raw: RawImuSample,
    pub derived: DerivedImuObservation,
    pub orientation: OrientationObservation,
}

impl Mpu6050Snapshot {
    pub const fn local_safety_inputs(
        &self,
    ) -> (
        conduit_create_oi::SafetyInputObservation,
        conduit_create_oi::SafetyInputObservation,
    ) {
        use conduit_create_oi::SafetyInputObservation::{Active, Clear};
        (
            if self.derived.tilt_active {
                Active
            } else {
                Clear
            },
            if self.derived.impact_active {
                Active
            } else {
                Clear
            },
        )
    }
}

pub struct Mpu6050Realization {
    session: Mpu6050Session,
    deriver: ImuDeriver,
}

impl Mpu6050Realization {
    pub fn new(evidence: &Mpu6050Evidence) -> Result<Self, Mpu6050OfferRefusal> {
        validate_evidence(evidence, evidence.observed_at_tick)?;
        Ok(Self {
            session: Mpu6050Session::new(evidence.address)
                .map_err(|_| Mpu6050OfferRefusal::InvalidAddress)?,
            deriver: ImuDeriver::new(evidence.calibration),
        })
    }

    pub fn observe<P: Mpu6050I2cProvider>(
        &mut self,
        evidence: &Mpu6050Evidence,
        provider: &mut P,
        observed_at_tick: u64,
        now_tick: u64,
    ) -> Result<Mpu6050Snapshot, Mpu6050ExecutionFailure> {
        let raw = self
            .session
            .observe(provider, observed_at_tick)
            .map_err(Mpu6050ExecutionFailure::Device)?;
        let derived = self
            .deriver
            .derive(raw, now_tick, evidence.thresholds)
            .map_err(Mpu6050ExecutionFailure::Derivation)?;
        let orientation = OrientationObservation::new(
            derived.roll_microradians,
            derived.pitch_microradians,
            derived.yaw_microradians,
        )
        .map_err(|_| Mpu6050ExecutionFailure::PortableValue)?;
        Ok(Mpu6050Snapshot {
            host_id: evidence.host_id.clone(),
            boot_id: evidence.boot_id.clone(),
            offer_generation: evidence.offer_generation,
            i2c_base_id: evidence.i2c_base_id.clone(),
            attachment_id: evidence.attachment_id.clone(),
            body_frame_id: evidence.body_frame_id.clone(),
            mounting_id: evidence.mounting_id.clone(),
            raw,
            derived,
            orientation,
        })
    }
}

pub fn live_mpu6050_advertisement(
    evidence: &Mpu6050Evidence,
    now_tick: u64,
) -> Result<HostAdvertisement, Mpu6050OfferRefusal> {
    validate_evidence(evidence, now_tick)?;
    let mut resources = vec![
        resource_offer(&evidence.i2c_base_id, I2C_BASE_RESOURCE, 1),
        resource_offer(&evidence.attachment_id, MPU6050_ATTACHMENT_RESOURCE, 1),
        resource_offer(&evidence.session_resource_id, MPU6050_SESSION_RESOURCE, 1),
    ];
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    Ok(HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: evidence.host_id.clone(),
        boot_id: evidence.boot_id.clone(),
        offer_generation: evidence.offer_generation,
        profile: conduit_core::HostProfileId::from(MPU6050_PROFILE),
        resources,
        capabilities: vec![mpu6050_offer()],
        planner_capabilities: Vec::new(),
    })
}

pub fn mpu6050_plan(evidence: &Mpu6050Evidence) -> Result<conduit_core::Plan, PlannerError> {
    let (_, profile) = crate::catalogs().expect("fixed Pete catalogs are valid");
    let checked = conduit_form::parse(MPU6050_FORM, &profile)
        .expect("portable IMU Form checks without mechanism facts");
    let host = live_mpu6050_advertisement(evidence, evidence.observed_at_tick)
        .expect("caller supplies valid current MPU-6050 evidence");
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
            sign_id: SignId::from(format!("mpu6050-resource-{index}")),
        })
        .collect::<Vec<_>>();
    plan_selected_realizations_with_characteristics_and_authority(
        &checked,
        SelectedRealizationPlanning {
            hosts: &[host],
            bases: &[ConnectionBase::Local],
            requirements: &BTreeMap::new(),
            advertisements: &[],
            observations: &observations,
            policies: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: ROBOTICS_ORIENTATION_ENCODED_LEN as u32,
            authority_grants: &[],
        },
    )
}

pub fn validate_mpu6050_plan(
    plan: &conduit_core::Plan,
    evidence: &Mpu6050Evidence,
) -> Result<(), &'static str> {
    let placement = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.implementation_id.as_str() == MPU6050_IMPLEMENTATION)
        .ok_or("Plan has no MPU-6050 orientation placement")?;
    if placement.host_id != evidence.host_id
        || placement.boot_id != evidence.boot_id
        || placement.offer_generation != evidence.offer_generation
        || placement.host_operations.len() != 1
        || placement.host_operations[0].contract_id.as_str() != MPU6050_OPERATION
        || placement.host_operations[0].maximum_input_bytes != 0
        || placement.host_operations[0].maximum_output_bytes
            != ROBOTICS_ORIENTATION_ENCODED_LEN as u32
        || placement.resources.len() != 3
    {
        return Err("Plan does not seal the exact MPU-6050 realization");
    }
    for (class, pool) in [
        (I2C_BASE_RESOURCE, evidence.i2c_base_id.as_str()),
        (MPU6050_ATTACHMENT_RESOURCE, evidence.attachment_id.as_str()),
        (
            MPU6050_SESSION_RESOURCE,
            evidence.session_resource_id.as_str(),
        ),
    ] {
        if !placement.resources.iter().any(|binding| {
            binding.class_id.as_str() == class
                && binding.pool_id.as_str() == pool
                && binding.units == 1
        }) {
            return Err("Plan resource binding does not match MPU-6050 evidence");
        }
    }
    Ok(())
}

fn mpu6050_offer() -> CapabilityOffer {
    let contract = conduit_std_catalog::robotics_observe_imu_contract();
    CapabilityOffer {
        startup_parameters: contract
            .configuration
            .iter()
            .map(|field| FaceStartupParameter {
                name: field.key.clone(),
                value_type: match field.default_value {
                    conduit_core::ConfigurationValue::Text(_) => "Text",
                    conduit_core::ConfigurationValue::I64(_) => "Scalar",
                    _ => unreachable!("IMU configuration is finite text/scalar"),
                }
                .into(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(MPU6050_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_std_catalog::ROBOTICS_OBSERVE_IMU_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(MPU6050_PROFILE),
            implementation_id: ImplementationId::from(MPU6050_IMPLEMENTATION),
            artifact_id: ArtifactId::from(MPU6050_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(MPU6050_OPERATION),
            target_kind: Some(conduit_core::kind_id(
                conduit_core::ROBOTICS_ORIENTATION_INFO_ID,
            )),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: ROBOTICS_ORIENTATION_ENCODED_LEN as u32,
        }],
        resource_requirements: vec![
            resource_requirement(I2C_BASE_RESOURCE, 1),
            resource_requirement(MPU6050_ATTACHMENT_RESOURCE, 1),
            resource_requirement(MPU6050_SESSION_RESOURCE, 1),
        ],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: ROBOTICS_ORIENTATION_ENCODED_LEN as u32,
        },
    }
}

fn validate_evidence(evidence: &Mpu6050Evidence, now_tick: u64) -> Result<(), Mpu6050OfferRefusal> {
    if evidence.i2c_base_id.is_empty()
        || evidence.attachment_id.is_empty()
        || evidence.session_resource_id.is_empty()
        || evidence.body_frame_id.is_empty()
        || evidence.mounting_id.is_empty()
    {
        return Err(Mpu6050OfferRefusal::MissingIdentity);
    }
    if !matches!(evidence.address, 0x68 | 0x69) {
        return Err(Mpu6050OfferRefusal::InvalidAddress);
    }
    if evidence.thresholds.maximum_sample_age_ticks == 0 {
        return Err(Mpu6050OfferRefusal::InvalidFreshness);
    }
    if now_tick < evidence.observed_at_tick
        || now_tick - evidence.observed_at_tick > evidence.thresholds.maximum_sample_age_ticks
    {
        return Err(Mpu6050OfferRefusal::StaleEvidence);
    }
    if evidence.calibration.generation == 0
        || evidence.calibration.captured_at_tick > evidence.observed_at_tick
    {
        return Err(Mpu6050OfferRefusal::InvalidCalibration);
    }
    Ok(())
}

#[cfg(test)]
#[path = "imu_observation_tests.rs"]
mod tests;
