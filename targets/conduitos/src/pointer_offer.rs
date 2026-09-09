//! Optional boot-scoped USB pointer realization behind the portable pointer Kind.

use alloc::{format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement,
    HostOperationContractId, HostOperationRequirement, ImplementationId, KindContractRevision,
    PortDescriptor, PortDirection, PortTemporal, kind_id, port_id, resource_offer,
};

pub const POINTER_IMPLEMENTATION: &str = "conduitos/usb-hid-pointer@1";
pub const POINTER_EXECUTION_PROFILE: &str = "conduitos/usb-input-cooperative@1";
pub const PS2_POINTER_IMPLEMENTATION: &str = "conduitos/ps2-pointer@1";
pub const PS2_INPUT_EXECUTION_PROFILE: &str = "conduitos/ps2-input-cooperative@1";
pub const NEXT_POINTER_EVENT_HOST_OPERATION: &str = "conduit.host/input-next-pointer-event@1";
pub const POINTER_EVENT_SLOTS: u16 = 8;
pub const POINTER_OPERATION_SLOTS: u16 = 1;
pub const POINTER_EVENT_MAXIMUM_BYTES: u32 = 512;
pub const POINTER_DEVICE_RESOURCE: &str = "conduitos.resource/pointer-device-instance@1";
pub const POINTER_INTERFACE_RESOURCE: &str = "conduitos.resource/pointer-interface-instance@1";
pub const POINTER_ENDPOINT_RESOURCE: &str = "conduitos.resource/pointer-endpoint-instance@1";
pub const POINTER_REPORT_RESOURCE: &str = "conduitos.resource/pointer-report-buffer@1";
pub const POINTER_TRANSITION_RESOURCE: &str = "conduitos.resource/pointer-event-slot@1";
pub const POINTER_OPERATION_RESOURCE: &str = "conduitos.resource/pointer-operation-slot@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerRealization {
    pub mechanism: PointerMechanism,
    pub controller_id: [u8; 32],
    pub device_id: [u8; 32],
    pub interface_id: [u8; 32],
    pub endpoint_id: [u8; 32],
    pub report_buffers: u16,
    pub event_slots: u16,
    pub operation_slots: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerMechanism {
    UsbHid,
    Ps2,
}

impl PointerMechanism {
    const fn implementation(self) -> &'static str {
        match self {
            Self::UsbHid => POINTER_IMPLEMENTATION,
            Self::Ps2 => PS2_POINTER_IMPLEMENTATION,
        }
    }

    const fn execution_profile(self) -> &'static str {
        match self {
            Self::UsbHid => POINTER_EXECUTION_PROFILE,
            Self::Ps2 => PS2_INPUT_EXECUTION_PROFILE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointerOffer<'a> {
    pub artifact_build: &'a str,
    pub realization: PointerRealization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerOfferError {
    EmptyIdentity,
    DuplicateIdentity,
    InvalidCapacity,
    ArtifactMismatch,
}

impl PointerRealization {
    pub fn validate(self) -> Result<(), PointerOfferError> {
        let identities = [
            self.controller_id,
            self.device_id,
            self.interface_id,
            self.endpoint_id,
        ];
        if identities.contains(&[0; 32]) {
            return Err(PointerOfferError::EmptyIdentity);
        }
        if identities
            .iter()
            .enumerate()
            .any(|(index, id)| identities[..index].contains(id))
        {
            return Err(PointerOfferError::DuplicateIdentity);
        }
        if self.report_buffers == 0 || self.event_slots == 0 || self.operation_slots == 0 {
            return Err(PointerOfferError::InvalidCapacity);
        }
        Ok(())
    }
}

impl PointerOffer<'_> {
    pub fn validate(self, expected_build: &str) -> Result<(), PointerOfferError> {
        if self.artifact_build.is_empty() || self.artifact_build != expected_build {
            return Err(PointerOfferError::ArtifactMismatch);
        }
        self.realization.validate()
    }
}

pub(crate) fn append_to_advertisement(
    advertisement: &mut HostAdvertisement,
    pointer: PointerOffer<'_>,
    build_id: &str,
) -> Result<(), PointerOfferError> {
    pointer.validate(build_id)?;
    let realization = pointer.realization;
    let resources = [
        (
            realization.controller_id,
            crate::keyboard_offer::CONTROLLER_RESOURCE,
            1_u32,
        ),
        (realization.device_id, POINTER_DEVICE_RESOURCE, 1),
        (realization.interface_id, POINTER_INTERFACE_RESOURCE, 1),
        (realization.endpoint_id, POINTER_ENDPOINT_RESOURCE, 1),
        (
            realization.endpoint_id,
            POINTER_REPORT_RESOURCE,
            u32::from(realization.report_buffers),
        ),
        (
            realization.endpoint_id,
            POINTER_TRANSITION_RESOURCE,
            u32::from(realization.event_slots),
        ),
        (
            realization.endpoint_id,
            POINTER_OPERATION_RESOURCE,
            u32::from(realization.operation_slots),
        ),
    ];
    for (index, (identity, class, capacity)) in resources.into_iter().enumerate() {
        let pool_id = format!(
            "conduitos-device-{index}-{}",
            crate::identity::hex(&identity)
        );
        if !advertisement
            .resources
            .iter()
            .any(|item| item.pool_id.as_str() == pool_id)
        {
            advertisement
                .resources
                .push(resource_offer(&pool_id, class, capacity));
        }
    }
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let value_type = conduit_semantic_catalog::pointer_event_type();
    let value_kind = value_type
        .profile()
        .map_err(|_| PointerOfferError::InvalidCapacity)?
        .value_kind()
        .clone();
    let mut requirements = vec![conduit_core::resource_requirement(
        "conduit.resource/runtime-memory@1",
        4_096,
    )];
    for class in [
        crate::keyboard_offer::CONTROLLER_RESOURCE,
        POINTER_DEVICE_RESOURCE,
        POINTER_INTERFACE_RESOURCE,
        POINTER_ENDPOINT_RESOURCE,
        POINTER_REPORT_RESOURCE,
        POINTER_TRANSITION_RESOURCE,
        POINTER_OPERATION_RESOURCE,
    ] {
        requirements.push(conduit_core::resource_requirement(class, 1));
    }
    requirements.sort();
    advertisement
        .capabilities
        .push(conduit_core::CapabilityOffer {
            startup_parameters: Vec::new(),
            shorthand: None,
            capability_id: CapabilityId::from("conduitos/input-pointer@1"),
            kind_id: kind_id(conduit_semantic_catalog::POINTER_SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(
                conduit_semantic_catalog::GENERALIZED_INPUT_REVISION,
            ),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(
                    realization.mechanism.execution_profile(),
                ),
                implementation_id: ImplementationId::from(realization.mechanism.implementation()),
                artifact_id: ArtifactId::from(format!("conduitos-build/{build_id}")),
            },
            inputs: Vec::new(),
            outputs: vec![PortDescriptor {
                port_id: port_id("pointer"),
                value_kind,
                direction: PortDirection::Output,
                temporal: PortTemporal::Value,
            }],
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(NEXT_POINTER_EVENT_HOST_OPERATION),
                target_kind: Some(kind_id(conduit_semantic_catalog::POINTER_EVENT_INFO_ID)),
                maximum_in_flight: 1,
                maximum_input_bytes: 0,
                maximum_output_bytes: POINTER_EVENT_MAXIMUM_BYTES,
            }],
            resource_requirements: requirements,
            authority_requirements: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: realization.operation_slots,
                max_queue_items: realization.event_slots,
                max_queue_bytes: POINTER_EVENT_MAXIMUM_BYTES * u32::from(realization.event_slots),
            },
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration, PROTOCOL_VERSION};

    fn realization() -> PointerRealization {
        PointerRealization {
            mechanism: PointerMechanism::UsbHid,
            controller_id: [1; 32],
            device_id: [2; 32],
            interface_id: [3; 32],
            endpoint_id: [4; 32],
            report_buffers: 2,
            event_slots: POINTER_EVENT_SLOTS,
            operation_slots: POINTER_OPERATION_SLOTS,
        }
    }

    #[test]
    fn exact_device_chain_and_capacities_are_required() {
        assert_eq!(realization().validate(), Ok(()));
        let mut empty = realization();
        empty.endpoint_id = [0; 32];
        assert_eq!(empty.validate(), Err(PointerOfferError::EmptyIdentity));
        let mut duplicate = realization();
        duplicate.endpoint_id = duplicate.interface_id;
        assert_eq!(
            duplicate.validate(),
            Err(PointerOfferError::DuplicateIdentity)
        );
        let mut exhausted = realization();
        exhausted.event_slots = 0;
        assert_eq!(
            exhausted.validate(),
            Err(PointerOfferError::InvalidCapacity)
        );
    }

    #[test]
    fn portable_offer_preserves_exact_chain_and_reuses_controller_pool() {
        let realization = realization();
        let controller_pool = format!(
            "conduitos-device-0-{}",
            crate::identity::hex(&realization.controller_id)
        );
        let mut advertisement = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("host"),
            boot_id: BootId::from("boot"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("profile"),
            resources: vec![resource_offer(
                &controller_pool,
                crate::keyboard_offer::CONTROLLER_RESOURCE,
                1,
            )],
            capabilities: Vec::new(),
            planner_capabilities: Vec::new(),
        };
        append_to_advertisement(
            &mut advertisement,
            PointerOffer {
                artifact_build: "build",
                realization,
            },
            "build",
        )
        .unwrap();
        assert_eq!(
            advertisement
                .resources
                .iter()
                .filter(|item| item.pool_id.as_str() == controller_pool)
                .count(),
            1
        );
        assert_eq!(advertisement.resources.len(), 7);
        let capability = &advertisement.capabilities[0];
        assert_eq!(
            capability.kind_id.as_str(),
            conduit_semantic_catalog::POINTER_SOURCE_KIND
        );
        assert_eq!(capability.outputs[0].port_id.as_str(), "pointer");
        assert_eq!(capability.limits.max_queue_items, POINTER_EVENT_SLOTS);
        assert_eq!(capability.host_operations[0].maximum_in_flight, 1);
    }
}
