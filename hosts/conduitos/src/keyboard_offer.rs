//! Optional boot-scoped device realization behind the portable keyboard Kind.

use alloc::{format, vec, vec::Vec};

use conduit_core::{
    ArtifactId, CapabilityId, ExecutionProfileId, HostAdvertisement, ImplementationId,
    resource_offer,
};

pub const KEYBOARD_IMPLEMENTATION: &str = "conduitos/usb-hid-keyboard@1";
pub const KEYBOARD_EXECUTION_PROFILE: &str = "conduitos/usb-input-cooperative@1";
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
    pub controller_id: [u8; 32],
    pub device_id: [u8; 32],
    pub interface_id: [u8; 32],
    pub endpoint_id: [u8; 32],
    pub report_buffers: u16,
    pub transition_slots: u16,
    pub operation_slots: u16,
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
    for (index, (identity, class, capacity)) in resources.into_iter().enumerate() {
        advertisement.resources.push(resource_offer(
            &format!(
                "conduitos-device-{index}-{}",
                crate::identity::hex(&identity)
            ),
            class,
            capacity,
        ));
    }
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
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
    advertisement
        .capabilities
        .push(conduit_core::CapabilityOffer {
            startup_parameters: Vec::new(),
            shorthand: None,
            capability_id: CapabilityId::from("conduitos/input-keyboard@1"),
            kind_id: contract.kind_id,
            kind_contract_revision: conduit_semantic_catalog::keyboard_contract_revision(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(KEYBOARD_EXECUTION_PROFILE),
                implementation_id: ImplementationId::from(KEYBOARD_IMPLEMENTATION),
                artifact_id: ArtifactId::from(format!("conduitos-build/{build_id}")),
            },
            inputs: contract.inputs,
            outputs: contract.outputs,
            host_operations: vec![conduit_core::HostOperationRequirement {
                contract_id: conduit_core::HostOperationContractId::from(
                    NEXT_KEY_EVENT_HOST_OPERATION,
                ),
                target_kind: Some(conduit_core::kind_id(conduit_core::KEY_EVENT_INFO_ID)),
                maximum_in_flight: 1,
                maximum_input_bytes: 0,
                maximum_output_bytes: conduit_core::KEY_EVENT_ENCODED_LEN as u32,
            }],
            resource_requirements: requirements,
            authority_requirements: Vec::new(),
            limits: contract.limits,
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn realization() -> KeyboardRealization {
        KeyboardRealization {
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
}
