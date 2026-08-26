//! Live Create observation offers over one exact correlated session resource.

use crate::{
    CreateOdometryError, CreateOdometrySample, CreatePortableObservation,
    CreateSensorLoweringError, OiMode,
};
use conduit_core::{
    resource_offer, resource_requirement, ArtifactId, BatteryObservation, BootId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    FaceStartupParameter, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    OfferGeneration, PROTOCOL_VERSION, ROBOTICS_BATTERY_ENCODED_LEN, ROBOTICS_BEACON_ENCODED_LEN,
    ROBOTICS_BUTTONS_ENCODED_LEN, ROBOTICS_CHARGING_ENCODED_LEN, ROBOTICS_CLIFF_ENCODED_LEN,
    ROBOTICS_CONTACT_ENCODED_LEN, ROBOTICS_ODOMETRY_ENCODED_LEN, ROBOTICS_PROXIMITY_ENCODED_LEN,
    ROBOTICS_WHEEL_DROP_ENCODED_LEN,
};

pub const CREATE_OBSERVATION_PROFILE: &str = "pete/create1-correlated-observation@1";
pub const CREATE_OBSERVATION_ARTIFACT: &str = "conduit-pete/create1-observation@1";
pub const CREATE_OBSERVATION_RESOURCE: &str = "pete.resource/create1-observation-session@1";
pub const CREATE_UART_BASE_RESOURCE: &str = "pete.resource/create1-uart-base@1";
pub const CREATE_DEVICE_RESOURCE: &str = "pete.resource/create1-device@1";

const MAXIMUM_CHANNELS: usize = 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateObservationChannel {
    Contact,
    Cliff,
    WheelDrop,
    Proximity,
    VirtualWall,
    Infrared,
    Buttons,
    Charging,
    Battery,
    Odometry,
    BumpAggregate,
}

impl CreateObservationChannel {
    const ALL: [Self; MAXIMUM_CHANNELS] = [
        Self::Contact,
        Self::Cliff,
        Self::WheelDrop,
        Self::Proximity,
        Self::VirtualWall,
        Self::Infrared,
        Self::Buttons,
        Self::Charging,
        Self::Battery,
        Self::Odometry,
        Self::BumpAggregate,
    ];

    pub const fn implementation_id(self) -> &'static str {
        match self {
            Self::Contact => "pete/create1-observe-contact@1",
            Self::Cliff => "pete/create1-observe-cliff@1",
            Self::WheelDrop => "pete/create1-observe-wheel-drop@1",
            Self::Proximity => "pete/create1-observe-proximity@1",
            Self::VirtualWall => "pete/create1-observe-virtual-wall@1",
            Self::Infrared => "pete/create1-observe-infrared@1",
            Self::Buttons => "pete/create1-observe-buttons@1",
            Self::Charging => "pete/create1-observe-charging@1",
            Self::Battery => "pete/create1-observe-battery@1",
            Self::Odometry => "pete/create1-observe-odometry@1",
            Self::BumpAggregate => "pete/create1-observe-bump@1",
        }
    }

    pub const fn operation_id(self) -> &'static str {
        match self {
            Self::Contact => "pete.host/create1-observe-contact@1",
            Self::Cliff => "pete.host/create1-observe-cliff@1",
            Self::WheelDrop => "pete.host/create1-observe-wheel-drop@1",
            Self::Proximity => "pete.host/create1-observe-proximity@1",
            Self::VirtualWall => "pete.host/create1-observe-virtual-wall@1",
            Self::Infrared => "pete.host/create1-observe-infrared@1",
            Self::Buttons => "pete.host/create1-observe-buttons@1",
            Self::Charging => "pete.host/create1-observe-charging@1",
            Self::Battery => "pete.host/create1-observe-battery@1",
            Self::Odometry => "pete.host/create1-observe-odometry@1",
            Self::BumpAggregate => "pete.host/create1-observe-bump@1",
        }
    }

    const fn capability_id(self) -> &'static str {
        match self {
            Self::Contact => "pete/create1/contact",
            Self::Cliff => "pete/create1/cliff",
            Self::WheelDrop => "pete/create1/wheel-drop",
            Self::Proximity => "pete/create1/proximity",
            Self::VirtualWall => "pete/create1/virtual-wall",
            Self::Infrared => "pete/create1/infrared",
            Self::Buttons => "pete/create1/buttons",
            Self::Charging => "pete/create1/charging",
            Self::Battery => "pete/create1/battery",
            Self::Odometry => "pete/create1/odometry",
            Self::BumpAggregate => "pete/create1/bump",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateObservationEvidence {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub session_resource_id: String,
    pub mode: OiMode,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateObservationOfferRefusal {
    MissingIdentity,
    UnsupportedMode,
    StaleEvidence,
    InvalidFreshness,
}

pub fn live_create_observation_advertisement(
    evidence: &CreateObservationEvidence,
    now_tick: u64,
) -> Result<HostAdvertisement, CreateObservationOfferRefusal> {
    if evidence.serial_base_id.is_empty()
        || evidence.robot_identity.is_empty()
        || evidence.session_resource_id.is_empty()
    {
        return Err(CreateObservationOfferRefusal::MissingIdentity);
    }
    if !matches!(evidence.mode, OiMode::Safe | OiMode::Full) {
        return Err(CreateObservationOfferRefusal::UnsupportedMode);
    }
    if evidence.maximum_age_ticks == 0 {
        return Err(CreateObservationOfferRefusal::InvalidFreshness);
    }
    if now_tick.saturating_sub(evidence.observed_at_tick) > u64::from(evidence.maximum_age_ticks) {
        return Err(CreateObservationOfferRefusal::StaleEvidence);
    }
    Ok(HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: evidence.host_id.clone(),
        boot_id: evidence.boot_id.clone(),
        offer_generation: evidence.offer_generation,
        profile: conduit_core::HostProfileId::from(CREATE_OBSERVATION_PROFILE),
        resources: vec![
            resource_offer(&evidence.serial_base_id, CREATE_UART_BASE_RESOURCE, 1),
            resource_offer(&evidence.robot_identity, CREATE_DEVICE_RESOURCE, 1),
            resource_offer(
                &evidence.session_resource_id,
                CREATE_OBSERVATION_RESOURCE,
                1,
            ),
        ],
        capabilities: CreateObservationChannel::ALL
            .into_iter()
            .map(observation_offer)
            .collect(),
        planner_capabilities: Vec::new(),
    })
}

pub(crate) fn observation_offer(channel: CreateObservationChannel) -> CapabilityOffer {
    let (contract, revision, maximum_output_bytes) = contract(channel);
    CapabilityOffer {
        startup_parameters: contract
            .configuration
            .iter()
            .map(|field| FaceStartupParameter {
                name: field.key.clone(),
                value_type: match field.default_value {
                    ConfigurationValue::Text(_) => "Text",
                    ConfigurationValue::U64(_) => "Count",
                    ConfigurationValue::I64(_) => "Scalar",
                    _ => unreachable!("robotics configuration is finite text/integer"),
                }
                .into(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(channel.capability_id()),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(CREATE_OBSERVATION_PROFILE),
            implementation_id: ImplementationId::from(channel.implementation_id()),
            artifact_id: ArtifactId::from(CREATE_OBSERVATION_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(channel.operation_id()),
            target_kind: Some(output_kind(channel)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes,
        }],
        resource_requirements: vec![
            resource_requirement(CREATE_DEVICE_RESOURCE, 1),
            resource_requirement(CREATE_OBSERVATION_RESOURCE, 1),
            resource_requirement(CREATE_UART_BASE_RESOURCE, 1),
        ],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: maximum_output_bytes,
        },
    }
}

fn contract(
    channel: CreateObservationChannel,
) -> (conduit_std_catalog::StandardKindContract, &'static str, u32) {
    match channel {
        CreateObservationChannel::Contact => (
            conduit_std_catalog::robotics_observe_contact_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_CONTACT_REVISION,
            ROBOTICS_CONTACT_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::Cliff => (
            conduit_std_catalog::robotics_observe_cliff_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_CLIFF_REVISION,
            ROBOTICS_CLIFF_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::WheelDrop => (
            conduit_std_catalog::robotics_observe_wheel_drop_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_WHEEL_DROP_REVISION,
            ROBOTICS_WHEEL_DROP_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::Proximity => (
            conduit_std_catalog::robotics_observe_proximity_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_PROXIMITY_REVISION,
            ROBOTICS_PROXIMITY_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::VirtualWall | CreateObservationChannel::Infrared => (
            conduit_std_catalog::robotics_observe_beacon_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_BEACON_REVISION,
            ROBOTICS_BEACON_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::Buttons => (
            conduit_std_catalog::robotics_observe_buttons_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_BUTTONS_REVISION,
            ROBOTICS_BUTTONS_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::Charging => (
            conduit_std_catalog::robotics_observe_charging_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_CHARGING_REVISION,
            ROBOTICS_CHARGING_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::Battery => (
            conduit_std_catalog::robotics_observe_battery_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_BATTERY_REVISION,
            ROBOTICS_BATTERY_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::Odometry => (
            conduit_std_catalog::robotics_observe_odometry_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_ODOMETRY_REVISION,
            ROBOTICS_ODOMETRY_ENCODED_LEN as u32,
        ),
        CreateObservationChannel::BumpAggregate => (
            conduit_std_catalog::robotics_observe_bump_contract(),
            conduit_std_catalog::ROBOTICS_OBSERVE_BUMP_REVISION,
            conduit_core::BOOL_ENCODED_LEN as u32,
        ),
    }
}

fn output_kind(channel: CreateObservationChannel) -> conduit_core::KindId {
    contract(channel).0.outputs[0].value_kind.clone()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodedCreateObservation {
    Contact([u8; ROBOTICS_CONTACT_ENCODED_LEN]),
    Cliff([u8; ROBOTICS_CLIFF_ENCODED_LEN]),
    WheelDrop([u8; ROBOTICS_WHEEL_DROP_ENCODED_LEN]),
    Proximity([u8; ROBOTICS_PROXIMITY_ENCODED_LEN]),
    Beacon([u8; ROBOTICS_BEACON_ENCODED_LEN]),
    Buttons([u8; ROBOTICS_BUTTONS_ENCODED_LEN]),
    Charging([u8; ROBOTICS_CHARGING_ENCODED_LEN]),
    Battery([u8; ROBOTICS_BATTERY_ENCODED_LEN]),
    Odometry([u8; ROBOTICS_ODOMETRY_ENCODED_LEN]),
    BumpAggregate([u8; conduit_core::BOOL_ENCODED_LEN]),
}

impl EncodedCreateObservation {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Contact(value) => value,
            Self::Cliff(value) => value,
            Self::WheelDrop(value) => value,
            Self::Proximity(value) => value,
            Self::Beacon(value) => value,
            Self::Buttons(value) => value,
            Self::Charging(value) => value,
            Self::Battery(value) => value,
            Self::Odometry(value) => value,
            Self::BumpAggregate(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateObservationEncodeRefusal {
    InvalidFreshness,
    StaleObservation,
    InvalidObservation(CreateSensorLoweringError),
    Odometry(CreateOdometryError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateObservationSnapshot {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    pub observation_generation: u32,
    pub observed_at_tick: u64,
    pub maximum_age_ticks: u32,
    pub observation: CreatePortableObservation,
    pub odometry: Option<CreateOdometrySample>,
}

pub fn encode_create_observation(
    snapshot: &CreateObservationSnapshot,
    channel: CreateObservationChannel,
    now_tick: u64,
) -> Result<Option<EncodedCreateObservation>, CreateObservationEncodeRefusal> {
    if snapshot.maximum_age_ticks == 0 {
        return Err(CreateObservationEncodeRefusal::InvalidFreshness);
    }
    if now_tick.saturating_sub(snapshot.observed_at_tick) > u64::from(snapshot.maximum_age_ticks) {
        return Err(CreateObservationEncodeRefusal::StaleObservation);
    }
    let observation = snapshot.observation;
    let group = observation.group_zero;
    Ok(match channel {
        CreateObservationChannel::Contact => {
            Some(EncodedCreateObservation::Contact(group.contact.encode()))
        }
        CreateObservationChannel::Cliff => {
            Some(EncodedCreateObservation::Cliff(group.cliff.encode()))
        }
        CreateObservationChannel::WheelDrop => Some(EncodedCreateObservation::WheelDrop(
            group.wheel_drop.encode(),
        )),
        CreateObservationChannel::Proximity => Some(EncodedCreateObservation::Proximity(
            group.proximity.encode(),
        )),
        CreateObservationChannel::VirtualWall => group
            .virtual_wall
            .map(|value| EncodedCreateObservation::Beacon(value.encode())),
        CreateObservationChannel::Infrared => group
            .infrared
            .map(|value| EncodedCreateObservation::Beacon(value.encode())),
        CreateObservationChannel::Buttons => {
            Some(EncodedCreateObservation::Buttons(group.buttons.encode()))
        }
        CreateObservationChannel::Charging => Some(EncodedCreateObservation::Charging(
            group
                .charging
                .with_sources(observation.charging_sources)
                .map_err(CreateObservationEncodeRefusal::InvalidObservation)?
                .encode(),
        )),
        CreateObservationChannel::Battery => group
            .charging
            .battery()
            .map_err(CreateObservationEncodeRefusal::InvalidObservation)?
            .map(BatteryObservation::encode)
            .map(EncodedCreateObservation::Battery),
        CreateObservationChannel::Odometry => snapshot
            .odometry
            .map(|sample| EncodedCreateObservation::Odometry(sample.value.encode())),
        CreateObservationChannel::BumpAggregate => Some(EncodedCreateObservation::BumpAggregate(
            conduit_core::InfoBool::new(
                snapshot
                    .observation
                    .group_zero
                    .contact
                    .active_body_sectors()
                    != 0,
            )
            .encode(),
        )),
    })
}

#[cfg(test)]
#[path = "create_observation_offer_tests.rs"]
mod tests;
