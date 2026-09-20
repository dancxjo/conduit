//! Optional boot-scoped device realization behind the portable keyboard Kind.

use alloc::{format, vec, vec::Vec};

use conduit_core::{
    ArtifactId, BaseEnforcementClass, BaseImplementationId, BaseInstanceId, BaseLifecycle,
    BaseProviderEntry, BaseRegistry, BaseRegistryLimits, CapabilityId, ExecutionProfileId,
    HostAdvertisement, HostBaseId, HostBaseKindId, ImplementationId, resource_offer,
};

pub const KEYBOARD_IMPLEMENTATION: &str = "conduitos/usb-hid-keyboard@1";
pub const KEYBOARD_EXECUTION_PROFILE: &str = "conduitos/usb-input-cooperative@1";
pub const PS2_KEYBOARD_IMPLEMENTATION: &str = "conduitos/ps2-keyboard@1";
pub const PS2_INPUT_EXECUTION_PROFILE: &str = "conduitos/ps2-input-cooperative@1";
pub const XHCI_BASE_IMPLEMENTATION: &str = "conduitos.base/xhci@1";
pub const I8042_BASE_IMPLEMENTATION: &str = "conduitos.base/i8042@1";
pub const CONTROLLER_RESOURCE: &str = "conduitos.resource/device-controller-instance@1";
pub const DEVICE_RESOURCE: &str = "conduitos.resource/device-instance@1";
pub const INTERFACE_RESOURCE: &str = "conduitos.resource/device-interface-instance@1";
pub const ENDPOINT_RESOURCE: &str = "conduitos.resource/device-endpoint-instance@1";
pub const REPORT_RESOURCE: &str = "conduitos.resource/input-report-buffer@1";
pub const TRANSITION_RESOURCE: &str = "conduitos.resource/input-transition-slot@1";
pub const OPERATION_RESOURCE: &str = "conduitos.resource/input-operation-slot@1";
pub const NEXT_KEY_EVENT_HOST_OPERATION: &str = "conduit.host/input-next-key-event@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardRealization {
    pub mechanism: KeyboardMechanism,
    pub controller_id: [u8; 32],
    pub device_id: [u8; 32],
    pub interface_id: [u8; 32],
    pub endpoint_id: [u8; 32],
    pub report_buffers: u16,
    pub transition_slots: u16,
    pub operation_slots: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardMechanism {
    UsbHid,
    Ps2,
}

impl KeyboardMechanism {
    pub const fn implementation(self) -> &'static str {
        match self {
            Self::UsbHid => KEYBOARD_IMPLEMENTATION,
            Self::Ps2 => PS2_KEYBOARD_IMPLEMENTATION,
        }
    }

    pub const fn execution_profile(self) -> &'static str {
        match self {
            Self::UsbHid => KEYBOARD_EXECUTION_PROFILE,
            Self::Ps2 => PS2_INPUT_EXECUTION_PROFILE,
        }
    }

    pub const fn base_implementation(self) -> &'static str {
        match self {
            Self::UsbHid => XHCI_BASE_IMPLEMENTATION,
            Self::Ps2 => I8042_BASE_IMPLEMENTATION,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardOffer<'a> {
    pub artifact_build: &'a str,
    pub realization: KeyboardRealization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardOfferError {
    EmptyIdentity,
    DuplicateIdentity,
    InvalidCapacity,
    ArtifactMismatch,
}

impl KeyboardRealization {
    pub fn validate(self) -> Result<(), KeyboardOfferError> {
        let identities = [
            self.controller_id,
            self.device_id,
            self.interface_id,
            self.endpoint_id,
        ];
        if identities.contains(&[0; 32]) {
            return Err(KeyboardOfferError::EmptyIdentity);
        }
        if identities
            .iter()
            .enumerate()
            .any(|(index, id)| identities[..index].contains(id))
        {
            return Err(KeyboardOfferError::DuplicateIdentity);
        }
        if self.report_buffers == 0 || self.transition_slots == 0 || self.operation_slots == 0 {
            return Err(KeyboardOfferError::InvalidCapacity);
        }
        Ok(())
    }
}

impl KeyboardOffer<'_> {
    pub fn validate(self, expected_build: &str) -> Result<(), KeyboardOfferError> {
        if self.artifact_build.is_empty() || self.artifact_build != expected_build {
            return Err(KeyboardOfferError::ArtifactMismatch);
        }
        self.realization.validate()
    }
}

pub(crate) fn append_to_advertisement(
    advertisement: &mut HostAdvertisement,
    keyboard: KeyboardOffer<'_>,
    build_id: &str,
) -> Result<(), KeyboardOfferError> {
    keyboard.validate(build_id)?;
    let realization = keyboard.realization;
    let resources = [
        (realization.controller_id, CONTROLLER_RESOURCE, 1_u32),
        (realization.device_id, DEVICE_RESOURCE, 1),
        (realization.interface_id, INTERFACE_RESOURCE, 1),
        (realization.endpoint_id, ENDPOINT_RESOURCE, 1),
        (
            realization.endpoint_id,
            REPORT_RESOURCE,
            u32::from(realization.report_buffers),
        ),
        (
            realization.endpoint_id,
            TRANSITION_RESOURCE,
            u32::from(realization.transition_slots),
        ),
        (
            realization.endpoint_id,
            OPERATION_RESOURCE,
            u32::from(realization.operation_slots),
        ),
    ];
    let resources = resources
        .into_iter()
        .enumerate()
        .map(|(index, (identity, class, capacity))| {
            resource_offer(
                &format!(
                    "conduitos-device-{index}-{}",
                    crate::identity::hex(&identity)
                ),
                class,
                capacity,
            )
        })
        .collect::<Vec<_>>();
    let contract = conduit_semantic_catalog::keyboard_contract();
    let mut requirements = vec![conduit_core::resource_requirement(
        "conduit.resource/runtime-memory@1",
        4_096,
    )];
    for class in [
        CONTROLLER_RESOURCE,
        DEVICE_RESOURCE,
        INTERFACE_RESOURCE,
        ENDPOINT_RESOURCE,
        REPORT_RESOURCE,
        TRANSITION_RESOURCE,
        OPERATION_RESOURCE,
    ] {
        requirements.push(conduit_core::resource_requirement(class, 1));
    }
    requirements.sort();
    let capability = conduit_core::CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos/input-keyboard@1"),
        kind_id: contract.kind_id,
        kind_contract_revision: conduit_semantic_catalog::keyboard_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                realization.mechanism.execution_profile(),
            ),
            implementation_id: ImplementationId::from(realization.mechanism.implementation()),
            artifact_id: ArtifactId::from(format!("conduitos-build/{build_id}")),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(NEXT_KEY_EVENT_HOST_OPERATION),
            target_kind: Some(conduit_core::kind_id(conduit_human::KEY_EVENT_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        }],
        resource_requirements: requirements,
        authority_requirements: Vec::new(),
        limits: contract.limits,
    };
    let maximum_advertised_capabilities =
        u16::try_from(advertisement.capabilities.len().saturating_add(1))
            .map_err(|_| KeyboardOfferError::InvalidCapacity)?;
    let maximum_advertised_resources = u16::try_from(
        advertisement
            .resources
            .len()
            .saturating_add(resources.len()),
    )
    .map_err(|_| KeyboardOfferError::InvalidCapacity)?;
    let mut registry = BaseRegistry::new(BaseRegistryLimits {
        maximum_bases: 1,
        maximum_capabilities_per_base: 1,
        maximum_resources_per_base: resources.len() as u16,
        maximum_advertised_capabilities,
        maximum_advertised_resources,
    })
    .map_err(|_| KeyboardOfferError::InvalidCapacity)?;
    registry
        .register(BaseProviderEntry {
            base_id: HostBaseId::from(crate::identity::hex(&realization.controller_id)),
            provider_instance_id: BaseInstanceId::from(crate::identity::hex(
                &realization.endpoint_id,
            )),
            provider_generation: advertisement.offer_generation.0,
            implementation_id: BaseImplementationId::from(
                realization.mechanism.base_implementation(),
            ),
            mechanism_family: HostBaseKindId::from("conduitos.base/keyboard-input@1"),
            enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![capability],
            resources,
        })
        .map_err(|_| KeyboardOfferError::InvalidCapacity)?;
    registry
        .project_ready_into(advertisement)
        .map_err(|_| KeyboardOfferError::InvalidCapacity)?;
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn realization() -> KeyboardRealization {
        KeyboardRealization {
            mechanism: KeyboardMechanism::UsbHid,
            controller_id: [1; 32],
            device_id: [2; 32],
            interface_id: [3; 32],
            endpoint_id: [4; 32],
            report_buffers: 2,
            transition_slots: 8,
            operation_slots: 2,
        }
    }

    #[test]
    fn exact_device_chain_and_capacities_are_required() {
        assert_eq!(
            KeyboardMechanism::UsbHid.base_implementation(),
            XHCI_BASE_IMPLEMENTATION
        );
        assert_ne!(
            KeyboardMechanism::UsbHid.base_implementation(),
            KeyboardMechanism::UsbHid.implementation()
        );
        assert_eq!(realization().validate(), Ok(()));
        let mut empty = realization();
        empty.endpoint_id = [0; 32];
        assert_eq!(empty.validate(), Err(KeyboardOfferError::EmptyIdentity));
        let mut duplicate = realization();
        duplicate.endpoint_id = duplicate.interface_id;
        assert_eq!(
            duplicate.validate(),
            Err(KeyboardOfferError::DuplicateIdentity)
        );
        let mut exhausted = realization();
        exhausted.transition_slots = 0;
        assert_eq!(
            exhausted.validate(),
            Err(KeyboardOfferError::InvalidCapacity)
        );
    }

    #[test]
    fn ps2_mechanism_keeps_portable_kind_and_names_exact_implementation() {
        let mut realization = realization();
        realization.mechanism = KeyboardMechanism::Ps2;
        let mut advertisement = conduit_core::HostAdvertisement {
            protocol_version: conduit_core::PROTOCOL_VERSION,
            host_id: conduit_core::HostId::from("host"),
            boot_id: conduit_core::BootId::from("boot"),
            offer_generation: conduit_core::OfferGeneration(1),
            profile: conduit_core::HostProfileId::from("profile"),
            capabilities: Vec::new(),
            bases: vec![],
            resources: Vec::new(),
            planner_capabilities: Vec::new(),
        };
        append_to_advertisement(
            &mut advertisement,
            KeyboardOffer {
                artifact_build: "build",
                realization,
            },
            "build",
        )
        .unwrap();
        let capability = &advertisement.capabilities[0];
        assert_eq!(
            capability.kind_id.as_str(),
            conduit_semantic_catalog::KEYBOARD_KIND
        );
        assert_eq!(
            capability.implementation.implementation_id.as_str(),
            PS2_KEYBOARD_IMPLEMENTATION
        );
        assert_eq!(
            capability.implementation.execution_profile_id.as_str(),
            PS2_INPUT_EXECUTION_PROFILE
        );
        assert_eq!(advertisement.bases.len(), 1);
        assert_eq!(
            advertisement.bases[0].implementation_id.as_str(),
            I8042_BASE_IMPLEMENTATION
        );
        assert_eq!(
            advertisement.bases[0].provider_instance_id.as_str(),
            crate::identity::hex(&realization.endpoint_id)
        );
        assert_eq!(advertisement.bases[0].capability_ids.len(), 1);
        assert_eq!(advertisement.bases[0].resource_pool_ids.len(), 7);
    }
}
