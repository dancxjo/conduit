use super::*;

fn configuration() -> SerialConfiguration {
    SerialConfiguration {
        baud_rate: 115_200,
        data_bits: 8,
        stop_bits: 1,
        parity: SerialParity::None,
        buffer_size: 4_096,
    }
}

fn bounds() -> SerialTransferBounds {
    SerialTransferBounds {
        maximum_transfer_bytes: 4_096,
        maximum_reads: 2,
        maximum_writes: 2,
        maximum_signal_operations: 2,
        maximum_in_flight: 1,
    }
}

fn offer() -> SerialAcquisitionOffer {
    SerialAcquisitionOffer {
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
        offer_generation: OfferGeneration(1),
        operation_contract: HostOperationContractId::from(SERIAL_ACQUIRE_OPERATION),
        request_authority_contract: AuthorityContractId::from(SERIAL_REQUEST_AUTHORITY),
        maximum_in_flight: 1,
        maximum_result_bytes: MAXIMUM_SERIAL_RESULT_BYTES as u32,
    }
}

fn authority() -> SerialAcquisitionAuthority {
    SerialAcquisitionAuthority {
        grant_id: AuthorityGrantId::from("serial-request/one"),
        contract_id: AuthorityContractId::from(SERIAL_REQUEST_AUTHORITY),
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
    }
}

fn operation() -> HostOperationId {
    HostOperationId::from("serial-acquire/one")
}

fn request() -> SerialAcquisitionRequest {
    SerialAcquisitionRequest {
        operation_id: operation(),
        configuration: configuration(),
        transfer_bounds: bounds(),
    }
}

fn resource() -> AcquiredSerialResource {
    AcquiredSerialResource {
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
        offer_generation: OfferGeneration(1),
        handle_id: ResourceHandleId::from("serial/opaque-one"),
        class_id: ResourceClassId::from(SERIAL_RESOURCE_CLASS),
        base_implementation_id: BaseImplementationId::from(SERIAL_BASE_IMPLEMENTATION),
        base_instance_id: BaseInstanceId::from("serial-base/one"),
        configuration: configuration(),
        transfer_bounds: bounds(),
        use_authority_contract: AuthorityContractId::from(SERIAL_USE_AUTHORITY),
        use_authority_grant: AuthorityGrantId::from("serial-use/one"),
        usb_vendor_id: Some(0x2e8a),
        usb_product_id: Some(0x000a),
    }
}

fn acquiring() -> BrowserSerialSession {
    let mut session = BrowserSerialSession::new();
    session
        .seal_acquisition(
            PlanId::from("serial-acquisition-plan/one"),
            &offer(),
            Some(&authority()),
            request(),
        )
        .unwrap();
    session.start_acquisition().unwrap();
    session
}

fn acquired() -> BrowserSerialSession {
    let mut session = acquiring();
    session
        .complete_acquisition(
            &operation(),
            256,
            SerialAcquisitionResult::Acquired(Box::new(resource())),
        )
        .unwrap();
    session
}

#[test]
fn device_context_exists_only_for_the_current_acquired_resource() {
    let capabilities = vec![CapabilityId::from("device/acquire-webserial@1")];
    assert!(BrowserSerialSession::new()
        .current_device_association(capabilities.clone())
        .is_none());

    let mut session = acquired();
    let association = session
        .current_device_association(capabilities.clone())
        .unwrap();
    assert_eq!(association.host_id, offer().host_id);
    assert_eq!(association.boot_id, offer().boot_id);
    assert_eq!(association.offer_generation, offer().offer_generation);
    assert_eq!(association.capability_ids, capabilities);
    assert_eq!(association.resources[0].handle_id, resource().handle_id);
    assert_eq!(
        association.identity_evidence.strength,
        conduit_core::DeviceIdentityStrength::BootLocalResource
    );
    assert!(association.validate_shape().is_ok());

    session.device_lost().unwrap();
    assert!(session
        .current_device_association(vec![CapabilityId::from("device/acquire-webserial@1")])
        .is_none());
}

fn playing() -> BrowserSerialSession {
    let mut session = acquired();
    let resource = resource();
    session
        .seal_use(
            PlanId::from("serial-use-plan/one"),
            &SerialUseRequirement {
                host_id: resource.host_id.clone(),
                boot_id: resource.boot_id.clone(),
                class_id: resource.class_id.clone(),
                base_implementation_id: resource.base_implementation_id.clone(),
                base_instance_id: resource.base_instance_id.clone(),
                transfer_bounds: resource.transfer_bounds,
            },
            Some(&resource.use_authority_grant),
        )
        .unwrap();
    session.start_use().unwrap();
    session
}

#[test]
fn explicit_acquisition_yields_exact_resource_then_bounded_use() {
    let mut session = playing();
    let BrowserSerialPhase::UsePlaying { plan_id, resource } = session.phase() else {
        panic!("serial Base use did not start")
    };
    assert_eq!(plan_id.as_str(), "serial-use-plan/one");
    assert_eq!(
        resource.base_implementation_id.as_str(),
        SERIAL_BASE_IMPLEMENTATION
    );
    assert_eq!(resource.base_instance_id.as_str(), "serial-base/one");
    assert_eq!(resource.usb_vendor_id, Some(0x2e8a));

    session
        .begin_transfer(SerialTransferDirection::Write)
        .unwrap();
    session
        .complete_transfer(SerialTransferDirection::Write, 4_096)
        .unwrap();
    assert_eq!(session.retained_bytes(), 4_096);
    session.release_transfer().unwrap();
    session
        .begin_transfer(SerialTransferDirection::Read)
        .unwrap();
    session
        .complete_transfer(SerialTransferDirection::Read, 3)
        .unwrap();
    session.release_transfer().unwrap();
    assert_eq!(session.admitted_reads(), 1);
    assert_eq!(session.admitted_writes(), 1);
    session
        .begin_transfer(SerialTransferDirection::Signals)
        .unwrap();
    session
        .complete_transfer(SerialTransferDirection::Signals, 0)
        .unwrap();
    session.release_transfer().unwrap();
    assert_eq!(session.admitted_signal_operations(), 1);
    session.close().unwrap();
    assert_eq!(
        session.phase(),
        &BrowserSerialPhase::Terminal(BrowserSerialTerminal::Closed)
    );
}

#[test]
fn request_authority_configuration_and_bounds_refuse_before_mutation() {
    let mut absent = BrowserSerialSession::new();
    assert_eq!(
        absent.seal_acquisition(PlanId::from("p"), &offer(), None, request()),
        Err(BrowserSerialRefusal::RequestAuthorityMissing)
    );
    assert_eq!(absent.phase(), &BrowserSerialPhase::OfferAvailable);

    let mut wrong_authority = authority();
    wrong_authority.boot_id = BootId::from("stale-boot");
    assert_eq!(
        absent.seal_acquisition(
            PlanId::from("p"),
            &offer(),
            Some(&wrong_authority),
            request()
        ),
        Err(BrowserSerialRefusal::RequestAuthorityMismatch)
    );
    assert_eq!(absent.phase(), &BrowserSerialPhase::OfferAvailable);

    let mut invalid_configuration = request();
    invalid_configuration.configuration.data_bits = 9;
    assert_eq!(
        absent.seal_acquisition(
            PlanId::from("p"),
            &offer(),
            Some(&authority()),
            invalid_configuration
        ),
        Err(BrowserSerialRefusal::InvalidConfiguration)
    );
    let mut invalid_bounds = request();
    invalid_bounds.transfer_bounds.maximum_in_flight = 2;
    assert_eq!(
        absent.seal_acquisition(
            PlanId::from("p"),
            &offer(),
            Some(&authority()),
            invalid_bounds
        ),
        Err(BrowserSerialRefusal::InvalidBounds)
    );
    assert_eq!(absent.phase(), &BrowserSerialPhase::OfferAvailable);
}

#[test]
fn stale_identity_and_malformed_completion_never_enter_resource_truth() {
    let mut stale = resource();
    stale.boot_id = BootId::from("stale-boot");
    let mut session = acquiring();
    assert_eq!(
        session.complete_acquisition(
            &operation(),
            256,
            SerialAcquisitionResult::Acquired(Box::new(stale))
        ),
        Err(BrowserSerialRefusal::MalformedResource)
    );
    assert_eq!(
        session.phase(),
        &BrowserSerialPhase::Terminal(BrowserSerialTerminal::MalformedCompletion)
    );

    let mut oversized = acquiring();
    assert_eq!(
        oversized.complete_acquisition(
            &operation(),
            MAXIMUM_SERIAL_RESULT_BYTES + 1,
            SerialAcquisitionResult::Acquired(Box::new(resource()))
        ),
        Err(BrowserSerialRefusal::CompletionTooLarge)
    );
    assert_eq!(
        oversized.phase(),
        &BrowserSerialPhase::Terminal(BrowserSerialTerminal::MalformedCompletion)
    );
}

#[test]
fn use_requires_the_exact_resource_base_and_authority() {
    let mut session = acquired();
    let mut requirement = SerialUseRequirement {
        host_id: resource().host_id,
        boot_id: resource().boot_id,
        class_id: resource().class_id,
        base_implementation_id: resource().base_implementation_id,
        base_instance_id: BaseInstanceId::from("stale-base"),
        transfer_bounds: bounds(),
    };
    assert_eq!(
        session.seal_use(PlanId::from("p"), &requirement, None),
        Err(BrowserSerialRefusal::UseAuthorityMissing)
    );
    assert!(matches!(
        session.phase(),
        BrowserSerialPhase::ResourceTruth(_)
    ));
    assert_eq!(
        session.seal_use(
            PlanId::from("p"),
            &requirement,
            Some(&AuthorityGrantId::from("serial-use/one"))
        ),
        Err(BrowserSerialRefusal::UseRequirementMismatch)
    );
    requirement.base_instance_id = BaseInstanceId::from("serial-base/one");
    session
        .seal_use(
            PlanId::from("p"),
            &requirement,
            Some(&AuthorityGrantId::from("serial-use/one")),
        )
        .unwrap();
}

#[test]
fn pressure_limits_loss_and_cancellation_remain_distinct() {
    let mut pressure = playing();
    pressure
        .begin_transfer(SerialTransferDirection::Write)
        .unwrap();
    assert_eq!(
        pressure.begin_transfer(SerialTransferDirection::Read),
        Err(BrowserSerialRefusal::Pressure)
    );
    pressure
        .complete_transfer(SerialTransferDirection::Write, 1)
        .unwrap();
    pressure.release_transfer().unwrap();
    pressure
        .begin_transfer(SerialTransferDirection::Read)
        .unwrap();
    assert_eq!(
        pressure.complete_transfer(SerialTransferDirection::Read, 4_097),
        Err(BrowserSerialRefusal::TransferTooLarge)
    );
    pressure
        .fail_transfer(BrowserSerialTerminal::TransferTooLarge)
        .unwrap();
    assert_eq!(
        pressure.phase(),
        &BrowserSerialPhase::Terminal(BrowserSerialTerminal::TransferTooLarge)
    );

    let mut cancelled = acquiring();
    cancelled.cancel().unwrap();
    assert_eq!(
        cancelled.phase(),
        &BrowserSerialPhase::Terminal(BrowserSerialTerminal::AcquisitionCancelled)
    );
}

#[test]
fn every_browser_acquisition_terminal_is_machine_distinct() {
    let cases = [
        (
            SerialAcquisitionResult::PermissionDenied,
            BrowserSerialTerminal::PermissionDenied,
        ),
        (
            SerialAcquisitionResult::NoPortSelected,
            BrowserSerialTerminal::NoPortSelected,
        ),
        (
            SerialAcquisitionResult::Unsupported,
            BrowserSerialTerminal::Unsupported,
        ),
        (
            SerialAcquisitionResult::OpenFailed,
            BrowserSerialTerminal::OpenFailed,
        ),
        (
            SerialAcquisitionResult::Cancelled,
            BrowserSerialTerminal::AcquisitionCancelled,
        ),
        (
            SerialAcquisitionResult::PlatformFailure,
            BrowserSerialTerminal::PlatformFailure,
        ),
    ];
    for (result, terminal) in cases {
        let mut session = acquiring();
        session
            .complete_acquisition(&operation(), 64, result)
            .unwrap();
        assert_eq!(session.phase(), &BrowserSerialPhase::Terminal(terminal));
    }
}
