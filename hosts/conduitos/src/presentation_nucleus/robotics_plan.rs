//! Exact ordinary PREWAKE Form and Plan preparation for all robotics contracts.

use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    KindContractRevision, OfferGeneration, PROTOCOL_VERSION, Plan, PortDescriptor, PortDirection,
    PortTemporal, kind_id, port_id,
};
use conduit_form::{ProfileCatalog, StartupCatalog, parse};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};

use super::robotics_play::RoboticsError;

pub(super) const IMU_SINK_KIND: &str = "conduitos/fixture-robotics-imu-sink";
pub(super) const ODOMETRY_SINK_KIND: &str = "conduitos/fixture-robotics-odometry-sink";
pub(super) const BATTERY_SINK_KIND: &str = "conduitos/fixture-robotics-battery-sink";
const SINK_REVISION: &str = "conduitos/fixture-robotics-sink@1";

pub struct PreparedRobotics {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
}

pub fn prepare_robotics(
    host: &str,
    boot: &str,
    bumper_pressed: bool,
    distance_mm: u32,
    age_ms: u32,
) -> Result<PreparedRobotics, RoboticsError> {
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    conduit_std_catalog::install_robotics_catalogs(&mut startup, &mut catalog)
        .map_err(|_| RoboticsError::Catalog)?;
    for (kind, value_kind) in discard_kinds() {
        catalog
            .insert(conduit_form::KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from(SINK_REVISION),
                inputs: discard_offer(kind, value_kind).inputs,
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .map_err(|_| RoboticsError::Catalog)?;
    }
    let state = if bumper_pressed { "pressed" } else { "clear" };
    let source = format!(
        "form 0\n\nprewake {{\n bump: robotics/observe-bump\n imu: robotics/observe-imu\n range: robotics/observe-range\n odometry: robotics/observe-odometry\n battery: robotics/observe-battery\n intent: robotics/velocity-intent\n drive: robotics/drive-differential\n imu_sink: {IMU_SINK_KIND}\n odometry_sink: {ODOMETRY_SINK_KIND}\n battery_sink: {BATTERY_SINK_KIND}\n bump.state = \"{state}\"\n imu.roll-microradians = 10\n imu.pitch-microradians = -20\n imu.yaw-microradians = 30\n range.distance-mm = {distance_mm}\n range.age-ms = {age_ms}\n odometry.forward-mm = 40\n odometry.lateral-mm = -50\n odometry.yaw-microradians = 60\n battery.charge-permille = 750\n battery.millivolts = 12000\n intent.linear-microunits = 750000\n intent.angular-microunits = -250000\n drive.minimum-clearance-mm = 250\n drive.maximum-range-age-ms = 1000\n bump.observation -> drive.bumper-pressed\n imu.orientation -> imu_sink.value\n range.range -> drive.forward-range\n odometry.odometry -> odometry_sink.value\n battery.battery -> battery_sink.value\n intent.linear -> drive.linear\n intent.angular -> drive.angular\n}}\n"
    );
    let form = parse(&source, &catalog).map_err(|_| RoboticsError::Form)?;
    let advertisement = advertisement(host, boot);
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts).map_err(|_| RoboticsError::Placement)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::ROBOTICS_ODOMETRY_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| RoboticsError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(RoboticsError::Plan);
    }
    Ok(PreparedRobotics {
        advertisement,
        plan,
    })
}

fn advertisement(host: &str, boot: &str) -> HostAdvertisement {
    let mut capabilities = conduit_std_catalog::conduitos_robotics_offers();
    capabilities.extend(
        discard_kinds()
            .into_iter()
            .map(|(kind, value_kind)| discard_offer(kind, value_kind)),
    );
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities,
    }
}

fn discard_kinds() -> [(&'static str, &'static str); 3] {
    [
        (IMU_SINK_KIND, conduit_core::ROBOTICS_ORIENTATION_INFO_ID),
        (ODOMETRY_SINK_KIND, conduit_core::ROBOTICS_ODOMETRY_INFO_ID),
        (BATTERY_SINK_KIND, conduit_core::ROBOTICS_BATTERY_INFO_ID),
    ]
}

fn discard_offer(kind: &str, value_kind: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!(
            "conduitos-fixture-{}@1",
            kind.rsplit('/').next().unwrap_or("robotics-sink")
        )),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(SINK_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_ROBOTICS_EXECUTION_PROFILE,
            ),
            implementation_id: ImplementationId::from("conduitos.fixture/robotics-sink@1"),
            artifact_id: ArtifactId::from("conduitos/robotics-fixture@1"),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(value_kind),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: Vec::new(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::ROBOTICS_ODOMETRY_ENCODED_LEN as u32,
        },
    }
}
