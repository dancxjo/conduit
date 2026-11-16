//! Fixture authority exercises ordinary admission, without physical possession.
use super::*;
fn identity() -> UsbNativeIdentity {
    UsbNativeIdentity {
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        base_id: "fixture/controller".into(),
        provider_instance_id: "fixture/provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pio-control".into(),
        resource_generation_id: ResourceGenerationId("fixture/resource-generation".into()),
        envelope_id: "fixture/envelope".into(),
        artifact_id: "fixture/artifact".into(),
    }
}
fn grant(identity: &UsbNativeIdentity) -> AuthorityGrant {
    AuthorityGrant {
        grant_id: "fixture/explicit-control-grant".into(),
        contract_id: super::super::control_factory::CONTROL_AUTHORITY.into(),
        host_call_contract_id: CONTROL_CALL.into(),
        subject_kind: CONTROL_KIND.into(),
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        capability_id: super::super::control_factory::CONTROL_IMPLEMENTATION.into(),
    }
}
fn package() -> ProtocolSourcePackage {
    crate::protocol_source::usb_enumeration_package().unwrap()
}

#[test]
fn caller_authority_selects_control_in_the_existing_production_kernel() {
    let identity = identity();
    let grant = grant(&identity);
    let artifact = prepare(package(), "usb-hub-power-port", &identity, &grant).unwrap();
    assert_eq!(
        artifact.artifact().definition().internal_plan.fragments[0]
            .placements
            .iter()
            .filter(|p| p.implementation_id.as_str()
                == super::super::control_factory::CONTROL_IMPLEMENTATION)
            .count(),
        1
    );
    let mut kernel = crate::usb_base::hid_source_kernel::PreparedHidSourceKernel::prepare_control(
        artifact,
        conduit_composite::KernelCompositeSignStorage::default(),
    )
    .unwrap();
    assert_eq!(
        kernel.kernel_mut().definition().boundary.input_fronts.len(),
        1
    );
    let artifact = prepare(package(), "usb-hub-power-port", &identity, &grant).unwrap();
    assert!(
        crate::usb_base::hid_source_kernel::PreparedHidSourceKernel::prepare(
            artifact,
            conduit_composite::KernelCompositeSignStorage::default()
        )
        .is_err()
    );
    let mut stale = grant.clone();
    stale.boot_id = "fixture/old-boot".into();
    assert!(matches!(
        prepare(package(), "usb-hub-power-port", &identity, &stale),
        Err(ControlSourcePrepareRefusal::WrongGrant)
    ));
    let mut wrong = grant;
    wrong.capability_id = "fixture/read".into();
    assert!(matches!(
        prepare(package(), "usb-hub-power-port", &identity, &wrong),
        Err(ControlSourcePrepareRefusal::WrongGrant)
    ));
}
