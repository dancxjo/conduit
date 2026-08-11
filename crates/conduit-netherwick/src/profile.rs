use conduit_core::{
    kind_id, port_id, resource_offer, resource_requirement, ArtifactId, BootId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, ImplementationOffer, KindContractRevision, OfferGeneration,
    PortDescriptor, PortDirection, PortTemporal, PROTOCOL_VERSION,
};
use conduit_form::{ProfileCatalog, StartupCatalog};

pub const BUMP_KIND: &str = conduit_std_catalog::ROBOTICS_OBSERVE_BUMP_KIND;
pub const IMU_KIND: &str = conduit_std_catalog::ROBOTICS_OBSERVE_IMU_KIND;
pub const DRIVE_KIND: &str = conduit_std_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND;
pub const SAFETY_DESCRIPTION_KIND: &str = "robotics/observe-safety-boundary";
pub const ACTUATOR_DESCRIPTION_KIND: &str = "robotics/describe-actuator";
pub const OBSERVATION_VALUE: &str = "conduit.robotics/observation";
pub const COMMAND_VALUE: &str = "conduit.robotics/command";
pub const BRAINSTEM_HOST: &str = "netherwick/pete-brainstem";
pub const BRAINSTEM_BOOT: &str = "netherwick/describe-fixture/brainstem-boot-f43ff138";
pub const MOTHERBRAIN_HOST: &str = "netherwick/pete-motherbrain";
pub const MOTHERBRAIN_BOOT: &str = "netherwick/describe-fixture/motherbrain-boot-f43ff138";
pub const SENSOR_RESOURCE: &str = "netherwick.resource/brainstem-sensor-bus@f43ff138";

pub fn catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_robotics_catalogs(&mut startup, &mut profile)?;
    Ok((startup, profile))
}

pub fn brainstem_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(BRAINSTEM_HOST),
        boot_id: BootId::from(BRAINSTEM_BOOT),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("netherwick/pete-brainstem-describe-only@f43ff138"),
        resources: vec![resource_offer(
            "netherwick/pete-brainstem/sensor-bus",
            SENSOR_RESOURCE,
            4,
        )],
        capabilities: vec![
            sensor_describe_offer(conduit_std_catalog::robotics_observe_bump_offer(), "bump"),
            sensor_describe_offer(conduit_std_catalog::robotics_observe_imu_offer(), "imu"),
            describe_offer(
                SAFETY_DESCRIPTION_KIND,
                "conduit.robotics/observe-safety-boundary@1",
                "safety-boundary",
            ),
            describe_offer(
                ACTUATOR_DESCRIPTION_KIND,
                "conduit.robotics/describe-actuator@1",
                "actuator-description",
            ),
        ],
        planner_capabilities: vec![],
    }
}

fn sensor_describe_offer(mut offer: CapabilityOffer, slug: &str) -> CapabilityOffer {
    offer.capability_id = CapabilityId::from(format!("netherwick/pete-brainstem/{slug}"));
    offer.implementation = ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from("netherwick/describe-only@1"),
        implementation_id: ImplementationId::from(format!(
            "netherwick/pete-brainstem/{slug}-describe@f43ff138"
        )),
        artifact_id: ArtifactId::from("netherwick/pete-brainstem@f43ff138"),
    };
    offer.host_operations.clear();
    offer.resource_requirements = vec![resource_requirement(SENSOR_RESOURCE, 1)];
    offer.authority_requirements.clear();
    offer
}

pub fn motherbrain_advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(MOTHERBRAIN_HOST),
        boot_id: BootId::from(MOTHERBRAIN_BOOT),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("netherwick/pete-motherbrain-describe-only@f43ff138"),
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

fn describe_offer(kind: &str, revision: &str, slug: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("netherwick/pete-brainstem/{slug}")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("netherwick/describe-only@1"),
            implementation_id: ImplementationId::from(format!(
                "netherwick/pete-brainstem/{slug}-describe@f43ff138"
            )),
            artifact_id: ArtifactId::from("netherwick/pete-brainstem@f43ff138"),
        },
        inputs: vec![],
        outputs: vec![port(
            "observation",
            OBSERVATION_VALUE,
            PortDirection::Output,
        )],
        host_operations: vec![],
        resource_requirements: vec![resource_requirement(SENSOR_RESOURCE, 1)],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 512,
        },
    }
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Current,
    }
}
