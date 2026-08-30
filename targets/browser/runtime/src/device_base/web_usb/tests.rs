use super::*;

fn configuration() -> UsbConfiguration {
    UsbConfiguration {
        configuration_value: 1,
        interface_number: 2,
        alternate_setting: 0,
        in_endpoint: 3,
        out_endpoint: 4,
    }
}

fn bounds() -> UsbTransferBounds {
    UsbTransferBounds {
        maximum_transfer_bytes: 4_096,
        maximum_in_transfers: 2,
        maximum_out_transfers: 2,
        maximum_in_flight: 1,
    }
}

fn offer() -> UsbAcquisitionOffer {
    UsbAcquisitionOffer {
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
        offer_generation: OfferGeneration(1),
        operation_contract: HostOperationContractId::from(USB_ACQUIRE_OPERATION),
        request_authority_contract: AuthorityContractId::from(USB_REQUEST_AUTHORITY),
        maximum_in_flight: 1,
        maximum_result_bytes: MAXIMUM_USB_RESULT_BYTES as u32,
    }
}

fn authority() -> UsbAcquisitionAuthority {
    UsbAcquisitionAuthority {
        grant_id: AuthorityGrantId::from("usb-request/one"),
        contract_id: AuthorityContractId::from(USB_REQUEST_AUTHORITY),
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
    }
}

fn operation() -> HostOperationId {
    HostOperationId::from("usb-acquire/one")
}

fn request() -> UsbAcquisitionRequest {
    UsbAcquisitionRequest {
        operation_id: operation(),
        configuration: configuration(),
        transfer_bounds: bounds(),
    }
}

fn resource() -> AcquiredUsbResource {
    AcquiredUsbResource {
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
        handle_id: ResourceHandleId::from("usb/opaque-one"),
        class_id: ResourceClassId::from(USB_RESOURCE_CLASS),
        base_implementation_id: BaseImplementationId::from(USB_BASE_IMPLEMENTATION),
        base_instance_id: BaseInstanceId::from("usb-base/one"),
        configuration: configuration(),
        transfer_bounds: bounds(),
        use_authority_contract: AuthorityContractId::from(USB_USE_AUTHORITY),
        use_authority_grant: AuthorityGrantId::from("usb-use/one"),
        vendor_id: 0x2e8a,
        product_id: 0x000a,
    }
}

fn acquiring() -> BrowserUsbSession {
    let mut session = BrowserUsbSession::new();
    session
        .seal_acquisition(
            PlanId::from("usb-acquisition-plan/one"),
            &offer(),
            Some(&authority()),
            request(),
        )
        .unwrap();
    session.start_acquisition().unwrap();
    session
}

fn acquired() -> BrowserUsbSession {
    let mut session = acquiring();
    session
        .complete_acquisition(
            &operation(),
            256,
            UsbAcquisitionResult::Acquired(Box::new(resource())),
        )
        .unwrap();
    session
}

fn playing() -> BrowserUsbSession {
    let mut session = acquired();
    let resource = resource();
    session
        .seal_use(
            PlanId::from("usb-use-plan/one"),
            &UsbUseRequirement {
                host_id: resource.host_id.clone(),
                boot_id: resource.boot_id.clone(),
                class_id: resource.class_id.clone(),
                base_implementation_id: resource.base_implementation_id.clone(),
                base_instance_id: resource.base_instance_id.clone(),
                configuration: resource.configuration,
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
    let BrowserUsbPhase::UsePlaying { plan_id, resource } = session.phase() else {
        panic!("USB use did not start")
    };
    assert_eq!(plan_id.as_str(), "usb-use-plan/one");
    assert_eq!(
        resource.base_implementation_id.as_str(),
        USB_BASE_IMPLEMENTATION
    );
    assert_eq!(resource.configuration.interface_number, 2);
    session
        .begin_transfer(UsbTransferKind::Bulk, UsbTransferDirection::Out, None)
        .unwrap();
    session
        .complete_transfer(UsbTransferKind::Bulk, UsbTransferDirection::Out, 4_096)
        .unwrap();
    assert_eq!(session.retained_bytes(), 4_096);
    session.release_transfer().unwrap();
    session
        .begin_transfer(
            UsbTransferKind::Control,
            UsbTransferDirection::In,
            Some(UsbControlSetup {
                request_type: UsbControlRequestType::Vendor,
                recipient: UsbControlRecipient::Interface,
                request: 0x42,
                value: 0,
                index: 2,
            }),
        )
        .unwrap();
    assert_eq!(
        session.retained_control_setup(),
        Some(UsbControlSetup {
            request_type: UsbControlRequestType::Vendor,
            recipient: UsbControlRecipient::Interface,
            request: 0x42,
            value: 0,
            index: 2,
        })
    );
    assert_eq!(
        session.release_transfer(),
        Err(BrowserUsbRefusal::WrongPhase)
    );
    session
        .complete_transfer(UsbTransferKind::Control, UsbTransferDirection::In, 0)
        .unwrap();
    session.release_transfer().unwrap();
    assert_eq!(session.admitted_in_transfers(), 1);
    assert_eq!(session.admitted_out_transfers(), 1);
    session
        .terminate_resource(BrowserUsbTerminal::Closed)
        .unwrap();
    assert_eq!(
        session.phase(),
        &BrowserUsbPhase::Terminal(BrowserUsbTerminal::Closed)
    );
}

#[test]
fn authority_configuration_and_bounds_refuse_without_mutation() {
    let mut session = BrowserUsbSession::new();
    assert_eq!(
        session.seal_acquisition(PlanId::from("p"), &offer(), None, request()),
        Err(BrowserUsbRefusal::RequestAuthorityMissing)
    );
    let mut stale = authority();
    stale.boot_id = BootId::from("stale");
    assert_eq!(
        session.seal_acquisition(PlanId::from("p"), &offer(), Some(&stale), request()),
        Err(BrowserUsbRefusal::RequestAuthorityMismatch)
    );
    let mut invalid = request();
    invalid.configuration.in_endpoint = 0;
    assert_eq!(
        session.seal_acquisition(PlanId::from("p"), &offer(), Some(&authority()), invalid),
        Err(BrowserUsbRefusal::InvalidConfiguration)
    );
    let mut invalid = request();
    invalid.transfer_bounds.maximum_in_flight = 2;
    assert_eq!(
        session.seal_acquisition(PlanId::from("p"), &offer(), Some(&authority()), invalid),
        Err(BrowserUsbRefusal::InvalidBounds)
    );
    assert_eq!(session.phase(), &BrowserUsbPhase::OfferAvailable);
}

#[test]
fn stale_resource_and_wrong_use_authority_never_start_use() {
    let mut stale = resource();
    stale.boot_id = BootId::from("stale");
    let mut session = acquiring();
    assert_eq!(
        session.complete_acquisition(
            &operation(),
            256,
            UsbAcquisitionResult::Acquired(Box::new(stale))
        ),
        Err(BrowserUsbRefusal::MalformedResource)
    );
    assert_eq!(
        session.phase(),
        &BrowserUsbPhase::Terminal(BrowserUsbTerminal::MalformedCompletion)
    );

    let mut session = acquired();
    let resource = resource();
    let requirement = UsbUseRequirement {
        host_id: resource.host_id,
        boot_id: resource.boot_id,
        class_id: resource.class_id,
        base_implementation_id: resource.base_implementation_id,
        base_instance_id: resource.base_instance_id,
        configuration: resource.configuration,
        transfer_bounds: resource.transfer_bounds,
    };
    assert_eq!(
        session.seal_use(PlanId::from("p"), &requirement, None),
        Err(BrowserUsbRefusal::UseAuthorityMissing)
    );
    assert!(matches!(session.phase(), BrowserUsbPhase::ResourceTruth(_)));
}

#[test]
fn pressure_transfer_status_loss_and_cancellation_are_distinct() {
    let mut session = playing();
    session
        .begin_transfer(UsbTransferKind::Bulk, UsbTransferDirection::Out, None)
        .unwrap();
    assert_eq!(
        session.begin_transfer(UsbTransferKind::Control, UsbTransferDirection::In, None),
        Err(BrowserUsbRefusal::Pressure)
    );
    session
        .fail_transfer(BrowserUsbTerminal::TransferStalled)
        .unwrap();
    assert_eq!(
        session.phase(),
        &BrowserUsbPhase::Terminal(BrowserUsbTerminal::TransferStalled)
    );

    let mut lost = acquired();
    lost.terminate_resource(BrowserUsbTerminal::DeviceLost)
        .unwrap();
    assert_eq!(
        lost.phase(),
        &BrowserUsbPhase::Terminal(BrowserUsbTerminal::DeviceLost)
    );

    let mut cancelled = acquiring();
    cancelled.cancel().unwrap();
    assert_eq!(
        cancelled.phase(),
        &BrowserUsbPhase::Terminal(BrowserUsbTerminal::AcquisitionCancelled)
    );
}

#[test]
fn every_acquisition_terminal_is_machine_distinct() {
    let cases = [
        (
            UsbAcquisitionResult::PermissionDenied,
            BrowserUsbTerminal::PermissionDenied,
        ),
        (
            UsbAcquisitionResult::NoDeviceSelected,
            BrowserUsbTerminal::NoDeviceSelected,
        ),
        (
            UsbAcquisitionResult::Unsupported,
            BrowserUsbTerminal::Unsupported,
        ),
        (
            UsbAcquisitionResult::OpenFailed,
            BrowserUsbTerminal::OpenFailed,
        ),
        (
            UsbAcquisitionResult::ConfigurationFailed,
            BrowserUsbTerminal::ConfigurationFailed,
        ),
        (
            UsbAcquisitionResult::InterfaceClaimFailed,
            BrowserUsbTerminal::InterfaceClaimFailed,
        ),
        (
            UsbAcquisitionResult::AlternateFailed,
            BrowserUsbTerminal::AlternateFailed,
        ),
        (
            UsbAcquisitionResult::Cancelled,
            BrowserUsbTerminal::AcquisitionCancelled,
        ),
        (
            UsbAcquisitionResult::PlatformFailure,
            BrowserUsbTerminal::PlatformFailure,
        ),
    ];
    for (result, terminal) in cases {
        let mut session = acquiring();
        session
            .complete_acquisition(&operation(), 64, result)
            .unwrap();
        assert_eq!(session.phase(), &BrowserUsbPhase::Terminal(terminal));
    }
}
