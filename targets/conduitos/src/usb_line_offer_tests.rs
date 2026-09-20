use super::*;

fn realization() -> UsbLineRealization {
    UsbLineRealization {
        controller_id: [1; 32],
        device_id: [2; 32],
        interface_id: [3; 32],
        input_endpoint_id: [4; 32],
        output_endpoint_id: [5; 32],
        attachment_epoch: 1,
        input_dci: 3,
        output_dci: 4,
        packet_bytes: 64,
        payload_bytes: 62,
        transfer_trbs_per_direction: 128,
    }
}

fn observation() -> UsbLineObservation {
    UsbLineObservation {
        realization: realization(),
        base_instance_id: BaseInstanceId::from("base/usb-ftdi/boot-a/1"),
        state: UsbLineState::Current,
        state_sign_id: SignId::from("sign/usb-ftdi/current/1"),
    }
}

fn identity() -> UsbLineIdentity {
    UsbLineIdentity {
        line_id: LineId::from("line/usb-ftdi/boot-a/1"),
        binding_id: LinkBindingId::from("binding/usb-ftdi/boot-a/1"),
        base_instance_id: BaseInstanceId::from("base/usb-ftdi/boot-a/1"),
        source_host_id: HostId::from("host/conduitos"),
        source_boot_id: BootId::from("boot/conduitos/a"),
        source_endpoint_id: LinkEndpointId::from("endpoint/conduitos/ftdi"),
        sink_host_id: HostId::from("host/qemu-harness-peer"),
        sink_boot_id: BootId::from("boot/qemu-harness-peer/a"),
        sink_endpoint_id: LinkEndpointId::from("endpoint/qemu-harness-peer/ftdi"),
    }
}

#[test]
fn exact_current_ftdi_chain_becomes_one_bounded_line() {
    let observation = observation();
    let offer = offer_usb_ftdi_line(identity(), &observation).unwrap();
    assert_eq!(offer.binding.base.as_str(), USB_FTDI_BASE);
    assert_eq!(offer.binding.limits, usb_ftdi_limits());
    assert_eq!(offer.contract, usb_ftdi_contract());
    assert_eq!(offer.availability.availability, LineAvailability::Ready);
    assert!(offer.validate_sign_identity());

    let admitted = AdmittedUsbLineBasis::from_offer(&offer, &observation).unwrap();
    assert_eq!(admitted.validate_current(&observation), Ok(()));
}

#[test]
fn presence_without_exact_current_truth_never_becomes_an_offer() {
    let mut absent = observation();
    absent.state = UsbLineState::Lost;
    assert_eq!(
        offer_usb_ftdi_line(identity(), &absent),
        Err(UsbLineOfferError::NotCurrent)
    );

    let mut wrong_endpoint = observation();
    wrong_endpoint.realization.output_dci = 5;
    assert_eq!(
        offer_usb_ftdi_line(identity(), &wrong_endpoint),
        Err(UsbLineOfferError::WrongEndpoint)
    );

    let mut wrong_base = identity();
    wrong_base.base_instance_id = BaseInstanceId::from("base/other");
    assert_eq!(
        offer_usb_ftdi_line(wrong_base, &observation()),
        Err(UsbLineOfferError::BaseInstanceMismatch)
    );
}

#[test]
fn loss_and_replacement_refuse_the_admitted_basis_distinctly() {
    let current = observation();
    let offer = offer_usb_ftdi_line(identity(), &current).unwrap();
    let admitted = AdmittedUsbLineBasis::from_offer(&offer, &current).unwrap();

    let mut lost = current.clone();
    lost.state = UsbLineState::Lost;
    lost.state_sign_id = SignId::from("sign/usb-ftdi/lost/1");
    assert_eq!(
        admitted.validate_current(&lost),
        Err(UsbLineOfferError::Lost)
    );

    let mut replacement = current.clone();
    replacement.realization.attachment_epoch = 2;
    replacement.realization.device_id = [9; 32];
    replacement.base_instance_id = BaseInstanceId::from("base/usb-ftdi/boot-a/2");
    replacement.state_sign_id = SignId::from("sign/usb-ftdi/current/2");
    assert_eq!(
        admitted.validate_current(&replacement),
        Err(UsbLineOfferError::BaseInstanceMismatch)
    );

    let mut stale_sign = current;
    stale_sign.state_sign_id = SignId::from("sign/usb-ftdi/current/changed");
    assert_eq!(
        admitted.validate_current(&stale_sign),
        Err(UsbLineOfferError::StaleState)
    );
}

#[test]
fn device_line_binding_and_host_identities_remain_separate() {
    let identity = identity();
    let realization = realization();
    let device_hex = crate::identity::hex(&realization.device_id);
    assert_ne!(identity.line_id.as_str(), device_hex);
    assert_ne!(identity.line_id.as_str(), identity.binding_id.as_str());
    assert_ne!(identity.source_host_id, identity.sink_host_id);
    assert_eq!(usb_ftdi_contract().security, LineSecurity::ProcessBoundary);
    assert_eq!(usb_ftdi_contract().continuation, LineContinuation::None);
}
