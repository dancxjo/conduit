use super::{
    BrowserMediaPhase, BrowserMediaSession, MAXIMUM_BROWSER_MEDIA_RESULT_BYTES,
    MAXIMUM_BROWSER_MEDIA_VALUE_BYTES,
};
use conduit_core::{
    AcquiredMediaResource, AuthorityContractId, AuthorityGrantId, BootId, HostId,
    HostOperationContractId, HostOperationId, HumanMediaKind, KindId, KnownPermissionState,
    MediaAcquisitionAuthority, MediaAcquisitionOffer, MediaAcquisitionRequest,
    MediaAcquisitionResult, MediaConstraints, MediaFlowBounds, MediaResourceAvailability,
    MediaUseRequirement, OfferGeneration, PlanId, ResourceClassId, ResourceHandleId,
};
use std::cell::RefCell;

const INPUT_BYTES: usize = MAXIMUM_BROWSER_MEDIA_VALUE_BYTES;
const EVIDENCE_BYTES: usize = 4096;
const CAMERA: i32 = 1;
const MICROPHONE: i32 = 2;

struct AbiState {
    host_id: HostId,
    boot_id: BootId,
    kind: HumanMediaKind,
    session: BrowserMediaSession,
    operation_id: HostOperationId,
    constraints: MediaConstraints,
    bounds: MediaFlowBounds,
    evidence: [u8; EVIDENCE_BYTES],
    evidence_len: usize,
    last_value_checksum: u32,
    stages: u16,
    resource_handle: Option<ResourceHandleId>,
    use_authority_grant: Option<AuthorityGrantId>,
}

thread_local! {
    static STATE: RefCell<Option<AbiState>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; INPUT_BYTES]> = const { RefCell::new([0; INPUT_BYTES]) };
}

fn kind(code: i32) -> Option<HumanMediaKind> {
    match code {
        CAMERA => Some(HumanMediaKind::Camera),
        MICROPHONE => Some(HumanMediaKind::Microphone),
        _ => None,
    }
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

#[no_mangle]
pub extern "C" fn conduit_browser_media_input_ptr() -> usize {
    INPUT.with(|input| input.borrow().as_ptr() as usize)
}

#[no_mangle]
pub extern "C" fn conduit_browser_media_input_capacity() -> usize {
    INPUT_BYTES
}

#[allow(clippy::too_many_arguments)]
#[no_mangle]
pub extern "C" fn conduit_browser_media_start_acquisition(
    host_len: usize,
    boot_len: usize,
    kind_code: i32,
    explicit_action: i32,
    request_authority: i32,
    minimum_primary: u32,
    maximum_primary: u32,
    minimum_secondary: u32,
    maximum_secondary: u32,
    maximum_rate_or_channels: u32,
) -> i32 {
    if explicit_action != 1 {
        return -2;
    }
    if request_authority != 1 {
        return -3;
    }
    let Some((host_id, boot_id)) = read_identity(host_len, boot_len) else {
        return -1;
    };
    let Some(kind) = kind(kind_code) else {
        return -4;
    };
    let constraints = match kind {
        HumanMediaKind::Camera => MediaConstraints::Camera {
            minimum_width: minimum_primary.try_into().unwrap_or(0),
            maximum_width: maximum_primary.try_into().unwrap_or(0),
            minimum_height: minimum_secondary.try_into().unwrap_or(0),
            maximum_height: maximum_secondary.try_into().unwrap_or(0),
            maximum_frames_per_second: maximum_rate_or_channels.try_into().unwrap_or(0),
        },
        HumanMediaKind::Microphone => MediaConstraints::Microphone {
            minimum_sample_rate_hz: minimum_primary,
            maximum_sample_rate_hz: maximum_primary,
            maximum_channels: maximum_rate_or_channels.try_into().unwrap_or(0),
        },
    };
    let bounds = MediaFlowBounds {
        maximum_value_bytes: MAXIMUM_BROWSER_MEDIA_VALUE_BYTES as u32,
        maximum_queue_items: 1,
        maximum_queue_bytes: MAXIMUM_BROWSER_MEDIA_VALUE_BYTES as u32,
    };
    let operation_id =
        HostOperationId::from(format!("{}/media-acquire/1", host_id.as_str()).as_str());
    let offer = MediaAcquisitionOffer {
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        offer_generation: OfferGeneration(1),
        kind,
        operation_contract: HostOperationContractId::from("conduit.host/acquire-human-media@1"),
        request_authority_contract: AuthorityContractId::from(
            "conduit.authority/request-human-media@1",
        ),
        known_permission: KnownPermissionState::Prompt,
        maximum_in_flight: 1,
        maximum_result_bytes: MAXIMUM_BROWSER_MEDIA_RESULT_BYTES as u32,
    };
    let authority = MediaAcquisitionAuthority {
        grant_id: AuthorityGrantId::from(format!("{}/media-request/1", host_id.as_str()).as_str()),
        contract_id: offer.request_authority_contract.clone(),
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        kind,
    };
    let request = MediaAcquisitionRequest {
        operation_id: operation_id.clone(),
        constraints,
        flow_bounds: bounds,
    };
    let mut session = BrowserMediaSession::new();
    if session
        .seal_acquisition(
            PlanId::from(format!("{}/media-acquisition-plan/1", host_id.as_str()).as_str()),
            &offer,
            Some(&authority),
            request,
        )
        .and_then(|()| session.start_acquisition())
        .is_err()
    {
        return -5;
    }
    let mut state = AbiState {
        host_id,
        boot_id,
        kind,
        session,
        operation_id,
        constraints,
        bounds,
        evidence: [0; EVIDENCE_BYTES],
        evidence_len: 0,
        last_value_checksum: 0,
        stages: 0b0000_0111,
        resource_handle: None,
        use_authority_grant: None,
    };
    refresh_evidence(&mut state);
    STATE.with(|slot| *slot.borrow_mut() = Some(state));
    0
}

#[no_mangle]
pub extern "C" fn conduit_browser_media_effect_kind() -> i32 {
    STATE.with(|slot| {
        slot.borrow().as_ref().map_or(0, |state| match state.kind {
            HumanMediaKind::Camera => CAMERA,
            HumanMediaKind::Microphone => MICROPHONE,
        })
    })
}

/// Outcome: 0 success, 1 denied, 2 dismissed, 3 cancelled, 4 no device,
/// 5 unsupported constraints, 6 capacity, 7 closed, 8 malformed.
#[allow(clippy::too_many_arguments)]
#[no_mangle]
pub extern "C" fn conduit_browser_media_complete_acquisition(
    outcome: i32,
    handle_len: usize,
    primary: u32,
    secondary: u32,
    rate_or_channels: u32,
    encoded_result_bytes: usize,
) -> i32 {
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return -1;
        };
        if outcome == 8 {
            return malformed(state);
        }
        let result = match outcome {
            0 => {
                if handle_len == 0 || handle_len > 256 {
                    return malformed(state);
                }
                let handle = INPUT.with(|input| {
                    core::str::from_utf8(&input.borrow()[..handle_len])
                        .ok()
                        .map(ResourceHandleId::from)
                });
                let Some(handle_id) = handle else {
                    return malformed(state);
                };
                let settings = match state.kind {
                    HumanMediaKind::Camera => MediaConstraints::Camera {
                        minimum_width: primary.try_into().unwrap_or(0),
                        maximum_width: primary.try_into().unwrap_or(0),
                        minimum_height: secondary.try_into().unwrap_or(0),
                        maximum_height: secondary.try_into().unwrap_or(0),
                        maximum_frames_per_second: rate_or_channels.try_into().unwrap_or(0),
                    },
                    HumanMediaKind::Microphone => MediaConstraints::Microphone {
                        minimum_sample_rate_hz: primary,
                        maximum_sample_rate_hz: primary,
                        maximum_channels: rate_or_channels.try_into().unwrap_or(0),
                    },
                };
                if !settings.is_valid() {
                    return malformed(state);
                }
                let use_authority_grant = AuthorityGrantId::from(
                    format!("{}/media-use/1", state.host_id.as_str()).as_str(),
                );
                state.resource_handle = Some(handle_id.clone());
                state.use_authority_grant = Some(use_authority_grant.clone());
                MediaAcquisitionResult::Acquired(AcquiredMediaResource {
                    host_id: state.host_id.clone(),
                    boot_id: state.boot_id.clone(),
                    handle_id,
                    class_id: ResourceClassId::from(match state.kind {
                        HumanMediaKind::Camera => "conduit.resource/acquired-camera@1",
                        HumanMediaKind::Microphone => "conduit.resource/acquired-microphone@1",
                    }),
                    value_kind: KindId::from(match state.kind {
                        HumanMediaKind::Camera => "media/camera-frame@1",
                        HumanMediaKind::Microphone => "media/microphone-frame@1",
                    }),
                    settings,
                    flow_bounds: state.bounds,
                    use_authority_contract: AuthorityContractId::from(
                        "conduit.authority/use-human-media@1",
                    ),
                    use_authority_grant,
                    availability: MediaResourceAvailability::Available,
                })
            }
            1 => MediaAcquisitionResult::Denied,
            2 => MediaAcquisitionResult::Dismissed,
            3 => MediaAcquisitionResult::Cancelled,
            4 => MediaAcquisitionResult::NoMatchingDevice,
            5 => MediaAcquisitionResult::UnsupportedConstraints,
            6 => MediaAcquisitionResult::CapacityExhausted,
            7 => MediaAcquisitionResult::Closed,
            _ => return malformed(state),
        };
        let operation = state.operation_id.clone();
        if state
            .session
            .complete_acquisition(&operation, encoded_result_bytes, result)
            .is_err()
        {
            return -6;
        }
        state.stages |= 0b0001_1000;
        refresh_evidence(state);
        0
    })
}

fn malformed(state: &mut AbiState) -> i32 {
    let operation = state.operation_id.clone();
    let _ = state
        .session
        .complete_acquisition(&operation, 0, MediaAcquisitionResult::Closed);
    refresh_evidence(state);
    -8
}

#[no_mangle]
pub extern "C" fn conduit_browser_media_start_use(use_authority: i32) -> i32 {
    if use_authority != 1 {
        return -2;
    }
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return -1;
        };
        let BrowserMediaPhase::ResourceTruth(resource) = state.session.phase() else {
            return -3;
        };
        let grant = resource.use_authority_grant.clone();
        let requirement = MediaUseRequirement {
            kind: state.kind,
            class_id: resource.class_id.clone(),
            value_kind: resource.value_kind.clone(),
            flow_bounds: state.bounds,
        };
        if state
            .session
            .seal_use(
                PlanId::from(format!("{}/media-use-plan/1", state.host_id.as_str()).as_str()),
                &requirement,
                Some(&grant),
            )
            .and_then(|()| state.session.start_use())
            .is_err()
        {
            return -4;
        }
        state.stages |= 0b0110_0000;
        refresh_evidence(state);
        0
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_media_submit_value(value_len: usize) -> i32 {
    if value_len == 0 || value_len > INPUT_BYTES {
        return -2;
    }
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return -1;
        };
        if state.session.admit_value(value_len).is_err() {
            return -3;
        }
        state.last_value_checksum = INPUT.with(|input| {
            input.borrow()[..value_len].iter().fold(0_u32, |sum, byte| {
                sum.wrapping_mul(16777619) ^ u32::from(*byte)
            })
        });
        state.stages |= 0b1000_0000;
        refresh_evidence(state);
        0
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_media_release_value() -> i32 {
    mutate(|state| state.session.release_value())
}
#[no_mangle]
pub extern "C" fn conduit_browser_media_device_lost() -> i32 {
    mutate(|state| state.session.device_lost())
}
#[no_mangle]
pub extern "C" fn conduit_browser_media_track_ended() -> i32 {
    mutate(|state| state.session.track_ended())
}
#[no_mangle]
pub extern "C" fn conduit_browser_media_close() -> i32 {
    mutate(|state| state.session.close_media())
}
#[no_mangle]
pub extern "C" fn conduit_browser_media_cancel() -> i32 {
    mutate(|state| state.session.cancel())
}

fn mutate(f: impl FnOnce(&mut AbiState) -> Result<(), super::BrowserMediaRefusal>) -> i32 {
    STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return -1;
        };
        if f(state).is_err() {
            return -2;
        }
        refresh_evidence(state);
        0
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_media_evidence_ptr() -> usize {
    STATE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map_or(0, |state| state.evidence.as_ptr() as usize)
    })
}
#[no_mangle]
pub extern "C" fn conduit_browser_media_evidence_len() -> usize {
    STATE.with(|slot| slot.borrow().as_ref().map_or(0, |state| state.evidence_len))
}

fn refresh_evidence(state: &mut AbiState) {
    let (phase, terminal) = match state.session.phase() {
        BrowserMediaPhase::OfferAvailable => ("offer-available", None),
        BrowserMediaPhase::AcquisitionPlanned(_) => ("acquisition-plan-sealed", None),
        BrowserMediaPhase::AcquisitionPlaying(_) => ("acquisition-play-started", None),
        BrowserMediaPhase::ResourceTruth(_) => ("resource-truth", None),
        BrowserMediaPhase::UsePlanned { .. } => ("media-use-plan-sealed", None),
        BrowserMediaPhase::UsePlaying { .. } => ("media-play-started", None),
        BrowserMediaPhase::Terminal(value) => ("terminal", Some(format!("{value:?}"))),
    };
    let all_stages = [
        (0b0000_0001, "offer-available"),
        (0b0000_0010, "acquisition-plan-sealed"),
        (0b0000_0100, "acquisition-play-started"),
        (0b0000_1000, "browser-result"),
        (0b0001_0000, "resource-truth-entered"),
        (0b0010_0000, "media-use-plan-sealed"),
        (0b0100_0000, "media-play-started"),
        (0b1000_0000, "bounded-media-value-observed"),
    ];
    let stages = all_stages
        .into_iter()
        .filter_map(|(bit, name)| (state.stages & bit != 0).then_some(name))
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "schema": "conduit.browser/human-media-evidence@1",
        "host_id": state.host_id.as_str(), "boot_id": state.boot_id.as_str(),
        "operation_id": state.operation_id.as_str(), "phase": phase, "terminal": terminal,
        "observed_values": state.session.observed_values(), "retained_bytes": state.session.retained_bytes(),
        "last_value_checksum": state.last_value_checksum,
        "constraints": format!("{:?}", state.constraints),
        "stages": stages,
        "resource_handle": state.resource_handle.as_ref().map(ResourceHandleId::as_str),
        "use_authority_grant": state.use_authority_grant.as_ref().map(AuthorityGrantId::as_str),
    });
    if let Ok(encoded) = serde_json::to_vec(&value) {
        if encoded.len() <= EVIDENCE_BYTES {
            state.evidence[..encoded.len()].copy_from_slice(&encoded);
            state.evidence_len = encoded.len();
        }
    }
}
