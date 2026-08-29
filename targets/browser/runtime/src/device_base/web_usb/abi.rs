use super::evidence::refresh_evidence;
use super::*;
use std::cell::RefCell;

const INPUT_BYTES: usize = MAXIMUM_USB_TRANSFER_BYTES;
const EVIDENCE_BYTES: usize = 4_096;
const IN: i32 = 1;
const OUT: i32 = 2;

pub(super) struct AbiState {
    pub(super) host_id: HostId,
    pub(super) boot_id: BootId,
    pub(super) session: BrowserUsbSession,
    pub(super) operation_id: HostOperationId,
    pub(super) acquisition_plan_id: PlanId,
    pub(super) use_plan_id: Option<PlanId>,
    pub(super) configuration: UsbConfiguration,
    pub(super) transfer_bounds: UsbTransferBounds,
    pub(super) resource: Option<AcquiredUsbResource>,
    pub(super) evidence: [u8; EVIDENCE_BYTES],
    pub(super) evidence_len: usize,
    pub(super) last_transfer_direction: Option<UsbTransferDirection>,
    pub(super) last_transfer_bytes: usize,
    pub(super) last_transfer_checksum: u32,
    pub(super) stages: u16,
}

thread_local! {
    static STATE: RefCell<Option<AbiState>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; INPUT_BYTES]> = const { RefCell::new([0; INPUT_BYTES]) };
}

fn read_identity(host_len: usize, boot_len: usize) -> Option<(HostId, BootId)> {
    if host_len == 0 || boot_len == 0 || host_len.checked_add(boot_len)? > INPUT_BYTES {
        return None;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        let host = core::str::from_utf8(&input[..host_len]).ok()?;
        let boot = core::str::from_utf8(&input[host_len..host_len + boot_len]).ok()?;
        Some((HostId::from(host), BootId::from(boot)))
    })
}

fn read_pair(first_len: usize, second_len: usize) -> Option<(String, String)> {
    if first_len == 0 || second_len == 0 || first_len.checked_add(second_len)? > INPUT_BYTES {
        return None;
    }
    INPUT.with(|input| {
        let input = input.borrow();
        let first = core::str::from_utf8(&input[..first_len]).ok()?.to_owned();
        let second = core::str::from_utf8(&input[first_len..first_len + second_len])
            .ok()?
            .to_owned();
        Some((first, second))
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_input_ptr() -> usize {
    INPUT.with(|input| input.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_input_capacity() -> usize {
    INPUT_BYTES
}

#[allow(clippy::too_many_arguments)]
#[no_mangle]
pub extern "C" fn conduit_browser_usb_start_acquisition(
    host_len: usize,
    boot_len: usize,
    explicit_action: i32,
    request_authority: i32,
    configuration_value: u8,
    interface_number: u8,
    alternate_setting: u8,
    in_endpoint: u8,
    out_endpoint: u8,
    maximum_transfer_bytes: u32,
    maximum_in_transfers: u16,
    maximum_out_transfers: u16,
) -> i32 {
    if explicit_action != 1 {
        return -2;
    }
    if request_authority != 1 {
        return -3;
    }
    if STATE.with(|slot| slot.borrow().is_some()) {
        return -4;
    }
    let Some((host_id, boot_id)) = read_identity(host_len, boot_len) else {
        return -1;
    };
    let configuration = UsbConfiguration {
        configuration_value,
        interface_number,
        alternate_setting,
        in_endpoint,
        out_endpoint,
    };
    let transfer_bounds = UsbTransferBounds {
        maximum_transfer_bytes,
        maximum_in_transfers,
        maximum_out_transfers,
        maximum_in_flight: 1,
    };
    let operation_id =
        HostOperationId::from(format!("{}/usb-acquire/1", host_id.as_str()).as_str());
    let offer = UsbAcquisitionOffer {
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        offer_generation: OfferGeneration(1),
        operation_contract: HostOperationContractId::from(USB_ACQUIRE_OPERATION),
        request_authority_contract: AuthorityContractId::from(USB_REQUEST_AUTHORITY),
        maximum_in_flight: 1,
        maximum_result_bytes: MAXIMUM_USB_RESULT_BYTES as u32,
    };
    let authority = UsbAcquisitionAuthority {
        grant_id: AuthorityGrantId::from(format!("{}/usb-request/1", host_id.as_str()).as_str()),
        contract_id: offer.request_authority_contract.clone(),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
    };
    let request = UsbAcquisitionRequest {
        operation_id: operation_id.clone(),
        configuration,
        transfer_bounds,
    };
    let acquisition_plan_id =
        PlanId::from(format!("{}/usb-acquisition-plan/1", host_id.as_str()).as_str());
    let mut session = BrowserUsbSession::new();
    if session
        .seal_acquisition(
            acquisition_plan_id.clone(),
            &offer,
            Some(&authority),
            request,
        )
        .and_then(|()| session.start_acquisition())
        .is_err()
    {
        return -6;
    }
    let mut state = AbiState {
        host_id,
        boot_id,
        session,
        operation_id,
        acquisition_plan_id,
        use_plan_id: None,
        configuration,
        transfer_bounds,
        resource: None,
        evidence: [0; EVIDENCE_BYTES],
        evidence_len: 0,
        last_transfer_direction: None,
        last_transfer_bytes: 0,
        last_transfer_checksum: 0,
        stages: 0b0000_0111,
    };
    refresh_evidence(&mut state);
    STATE.with(|slot| *slot.borrow_mut() = Some(state));
    0
}

/// Outcomes: 0 acquired, 1 denied, 2 no device, 3 unsupported, 4 open,
/// 5 configuration, 6 interface, 7 alternate, 8 cancelled, 9 platform.
#[allow(clippy::too_many_arguments)]
#[no_mangle]
pub extern "C" fn conduit_browser_usb_complete_acquisition(
    outcome: i32,
    handle_len: usize,
    base_instance_len: usize,
    vendor_id: u32,
    product_id: u32,
    encoded_result_bytes: usize,
) -> i32 {
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return -1;
        };
        let result = match outcome {
            0 => {
                let Some((handle, base_instance)) = read_pair(handle_len, base_instance_len) else {
                    return malformed(state);
                };
                let (Ok(vendor_id), Ok(product_id)) =
                    (u16::try_from(vendor_id), u16::try_from(product_id))
                else {
                    return malformed(state);
                };
                UsbAcquisitionResult::Acquired(Box::new(AcquiredUsbResource {
                    host_id: state.host_id.clone(),
                    boot_id: state.boot_id.clone(),
                    handle_id: ResourceHandleId::from(handle.as_str()),
                    class_id: ResourceClassId::from(USB_RESOURCE_CLASS),
                    base_implementation_id: BaseImplementationId::from(USB_BASE_IMPLEMENTATION),
                    base_instance_id: BaseInstanceId::from(base_instance.as_str()),
                    configuration: state.configuration,
                    transfer_bounds: state.transfer_bounds,
                    use_authority_contract: AuthorityContractId::from(USB_USE_AUTHORITY),
                    use_authority_grant: AuthorityGrantId::from(
                        format!("{}/usb-use/1", state.host_id.as_str()).as_str(),
                    ),
                    vendor_id,
                    product_id,
                }))
            }
            1 => UsbAcquisitionResult::PermissionDenied,
            2 => UsbAcquisitionResult::NoDeviceSelected,
            3 => UsbAcquisitionResult::Unsupported,
            4 => UsbAcquisitionResult::OpenFailed,
            5 => UsbAcquisitionResult::ConfigurationFailed,
            6 => UsbAcquisitionResult::InterfaceClaimFailed,
            7 => UsbAcquisitionResult::AlternateFailed,
            8 => UsbAcquisitionResult::Cancelled,
            9 => UsbAcquisitionResult::PlatformFailure,
            _ => return malformed(state),
        };
        let acquired = match &result {
            UsbAcquisitionResult::Acquired(value) => Some((**value).clone()),
            _ => None,
        };
        let operation = state.operation_id.clone();
        if state
            .session
            .complete_acquisition(&operation, encoded_result_bytes, result)
            .is_err()
        {
            refresh_evidence(state);
            return -7;
        }
        state.resource = acquired;
        state.stages |= 0b0000_1000;
        if state.resource.is_some() {
            state.stages |= 0b0001_0000;
        }
        refresh_evidence(state);
        0
    })
}

fn malformed(state: &mut AbiState) -> i32 {
    let operation = state.operation_id.clone();
    let _ =
        state
            .session
            .complete_acquisition(&operation, 0, UsbAcquisitionResult::PlatformFailure);
    refresh_evidence(state);
    -8
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_start_use(plan_len: usize, use_authority: i32) -> i32 {
    if plan_len == 0 || plan_len > INPUT_BYTES || use_authority != 1 {
        return -1;
    }
    let plan_id = INPUT.with(|input| {
        core::str::from_utf8(&input.borrow()[..plan_len])
            .ok()
            .map(PlanId::from)
    });
    let Some(plan_id) = plan_id else {
        return -1;
    };
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return -2;
        };
        let Some(resource) = state.resource.as_ref() else {
            return -3;
        };
        let requirement = UsbUseRequirement {
            host_id: state.host_id.clone(),
            boot_id: state.boot_id.clone(),
            class_id: resource.class_id.clone(),
            base_implementation_id: resource.base_implementation_id.clone(),
            base_instance_id: resource.base_instance_id.clone(),
            configuration: state.configuration,
            transfer_bounds: state.transfer_bounds,
        };
        let grant = resource.use_authority_grant.clone();
        if state
            .session
            .seal_use(plan_id.clone(), &requirement, Some(&grant))
            .and_then(|()| state.session.start_use())
            .is_err()
        {
            return -4;
        }
        state.use_plan_id = Some(plan_id);
        state.stages |= 0b0110_0000;
        refresh_evidence(state);
        0
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_begin_transfer(direction_code: i32) -> i32 {
    let direction = match direction_code {
        IN => UsbTransferDirection::In,
        OUT => UsbTransferDirection::Out,
        _ => return -3,
    };
    mutate(|session| session.begin_transfer(direction))
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_complete_transfer(
    direction_code: i32,
    transfer_len: usize,
) -> i32 {
    if transfer_len == 0 || transfer_len > INPUT_BYTES {
        return -2;
    }
    let direction = match direction_code {
        IN => UsbTransferDirection::In,
        OUT => UsbTransferDirection::Out,
        _ => return -3,
    };
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return -1;
        };
        if state
            .session
            .complete_transfer(direction, transfer_len)
            .is_err()
        {
            return -4;
        }
        state.last_transfer_direction = Some(direction);
        state.last_transfer_bytes = transfer_len;
        state.last_transfer_checksum = INPUT.with(|input| {
            input.borrow()[..transfer_len]
                .iter()
                .fold(0_u32, |sum, byte| {
                    sum.wrapping_mul(16_777_619) ^ u32::from(*byte)
                })
        });
        state.stages |= 0b1000_0000;
        refresh_evidence(state);
        0
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_release_transfer() -> i32 {
    mutate(BrowserUsbSession::release_transfer)
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_transfer_failed(failure_code: i32) -> i32 {
    let terminal = match failure_code {
        1 => BrowserUsbTerminal::TransferTooLarge,
        2 => BrowserUsbTerminal::TransferStalled,
        3 => BrowserUsbTerminal::TransferBabble,
        4 => BrowserUsbTerminal::TransferFailed,
        _ => return -3,
    };
    mutate(|session| session.fail_transfer(terminal))
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_device_lost() -> i32 {
    mutate(|session| session.terminate_resource(BrowserUsbTerminal::DeviceLost))
}
#[no_mangle]
pub extern "C" fn conduit_browser_usb_close_failed() -> i32 {
    mutate(|session| session.terminate_resource(BrowserUsbTerminal::CloseFailed))
}
#[no_mangle]
pub extern "C" fn conduit_browser_usb_close() -> i32 {
    mutate(|session| session.terminate_resource(BrowserUsbTerminal::Closed))
}
#[no_mangle]
pub extern "C" fn conduit_browser_usb_cancel() -> i32 {
    mutate(BrowserUsbSession::cancel)
}

fn mutate(operation: impl FnOnce(&mut BrowserUsbSession) -> Result<(), BrowserUsbRefusal>) -> i32 {
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return -1;
        };
        if operation(&mut state.session).is_err() {
            return -2;
        }
        refresh_evidence(state);
        0
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_usb_evidence_ptr() -> usize {
    STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |state| state.evidence.as_ptr() as usize)
    })
}
#[no_mangle]
pub extern "C" fn conduit_browser_usb_evidence_len() -> usize {
    STATE.with(|slot| slot.borrow().as_ref().map_or(0, |state| state.evidence_len))
}
