use super::*;

#[test]
fn claim_cannot_substitute_native_provider_generation_resource_or_envelope() {
    let identity = UsbNativeIdentity {
        host_id: "host/product".into(),
        boot_id: "boot/current".into(),
        base_id: "base/controller".into(),
        provider_instance_id: "provider/device".into(),
        provider_generation: 3,
        resource_pool_id: "resource/dma".into(),
        resource_generation_id: ResourceGenerationId("generation/current".into()),
        envelope_id: "envelope/current".into(),
        artifact_id: "artifact/product".into(),
    };
    let claim = BaseOperationClaim {
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: 3,
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        implementation_id: CONTROL_IMPLEMENTATION.into(),
        plan_id: "plan/current".into(),
        active_play_id: "play/current".into(),
        operation_contract_id: crate::usb_base::control_contract::CONTROL_CALL.into(),
        subject_kind: crate::usb_base::control_contract::CONTROL_KIND.into(),
        parameter_bytes: crate::usb_base::control_contract::CONTROL_MAXIMUM_BYTES,
        work_units: 1,
    };
    assert_eq!(validate_claim(&identity, &claim), Ok(()));
    let variants = [
        BaseOperationClaim {
            host_id: "host/other".into(),
            ..claim.clone()
        },
        BaseOperationClaim {
            boot_id: "boot/other".into(),
            ..claim.clone()
        },
        BaseOperationClaim {
            base_instance_id: "provider/other".into(),
            ..claim.clone()
        },
        BaseOperationClaim {
            base_provider_generation: 4,
            ..claim.clone()
        },
        BaseOperationClaim {
            resource_pool_id: "resource/other".into(),
            ..claim.clone()
        },
        BaseOperationClaim {
            resource_generation_id: ResourceGenerationId("generation/other".into()),
            ..claim.clone()
        },
        BaseOperationClaim {
            envelope_id: "envelope/other".into(),
            ..claim.clone()
        },
        BaseOperationClaim {
            implementation_id: "implementation/other".into(),
            ..claim
        },
    ];
    for invalid in variants {
        assert_eq!(
            validate_claim(&identity, &invalid),
            Err(UsbControlCallRefusal::Ownership(
                ControlOwnerRefusal::WrongBinding
            ))
        );
    }
}

#[test]
fn native_provider_generation_must_equal_actual_attachment_epoch() {
    assert_eq!(validate_attachment_identity(7, 7, 1, 2, 4), Ok(()));
    for (generation, epoch, slot, port, maximum) in [
        (6, 7, 1, 2, 4),
        (8, 7, 1, 2, 4),
        (0, 7, 1, 2, 4),
        (u64::from(u32::MAX) + 1, u32::MAX, 1, 2, 4),
        (0, 0, 1, 2, 4),
        (7, 7, 0, 2, 4),
        (7, 7, 1, 0, 4),
        (7, 7, 1, 5, 4),
    ] {
        assert_eq!(
            validate_attachment_identity(generation, epoch, slot, port, maximum),
            Err(UsbInstallationRefusal::InvalidIdentity),
        );
    }
}
