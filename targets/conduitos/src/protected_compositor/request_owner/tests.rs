use super::*;
use conduitos_compositor_core::wire::{FrameSpan, OUTPUT_BYTES};
use conduitos_compositor_core::{Error, FramePlan, FrameRect, Handle, MAX_CHUNK, MAX_PIXELS, Rect};
const HANDLE: Handle = Handle {
    slot: 7,
    generation: u64::MAX,
};
const RECT: Rect = Rect {
    x: i16::MIN,
    y: i16::MAX,
    width: 2048,
    height: 1024,
};
fn reply(owner: &RequestOwner, response: Response) -> alloc::vec::Vec<u8> {
    let (envelope, _) = wire::decode_request(owner.command().unwrap()).unwrap();
    let mut bytes = alloc::vec![0;wire::EXCHANGE_BYTES];
    let length = wire::encode_response(envelope, response, &mut bytes).unwrap();
    bytes.truncate(length);
    bytes
}
#[test]
fn every_supported_request_fits_exact_immutable_command_bound() {
    let requests = [
        Request::Admit { rect: RECT, z: 255 },
        Request::Remove(HANDLE),
        Request::Resize {
            surface: HANDLE,
            rect: RECT,
            z: 255,
        },
        Request::Place {
            surface: HANDLE,
            x: i16::MIN,
            y: i16::MAX,
            z: 255,
        },
        Request::Visible {
            surface: HANDLE,
            visible: true,
        },
        Request::Focus(HANDLE),
        Request::Cursor {
            position: Some((i32::MAX as u32 - 10, i32::MAX as u32 - 16)),
            hover: true,
        },
        Request::Cursor {
            position: None,
            hover: false,
        },
        Request::BeginUpload {
            surface: HANDLE,
            revision: u64::MAX,
            rect: RECT,
        },
        Request::PrepareUpload {
            pixels: MAX_CHUNK as u32,
        },
        Request::UploadWindow {
            offset: MAX_PIXELS as u32 - MAX_CHUNK as u32,
            window_offset: 0,
            pixels: MAX_CHUNK as u32,
        },
        Request::UploadWindow {
            offset: 0,
            window_offset: OUTPUT_BYTES - 4,
            pixels: 1,
        },
        Request::CommitUpload,
        Request::AbortUpload,
        Request::BeginFrame,
        Request::FrameStep {
            sequence: u64::MAX,
            pixels: MAX_CHUNK as u32,
        },
        Request::AcknowledgeScanout { sequence: u64::MAX },
        Request::FailFrame,
        Request::Pointer {
            x: u32::MAX,
            y: u32::MAX,
        },
        Request::Keyboard,
        Request::Focused,
        Request::DamagePending,
        Request::Initialize {
            width: u16::MAX,
            height: u16::MAX,
        },
    ];
    let mut largest = 0;
    for request in requests {
        let mut owner = RequestOwner::new(7, 2048, 1024);
        // Encoding completeness is independent of a frame's active ACK admission.
        let mut bytes = [0; COMMAND_BYTES];
        let length = wire::encode_request(
            Envelope {
                instance: 7,
                request: 1,
            },
            request,
            &mut bytes,
        )
        .unwrap();
        largest = largest.max(length);
        assert_eq!(wire::decode_request(&bytes[..length]).unwrap().1, request);
        if !matches!(request, Request::AcknowledgeScanout { .. }) {
            owner.begin(request).unwrap();
            assert_eq!(owner.command().unwrap(), &bytes[..length]);
        }
    }
    assert_eq!(largest, COMMAND_BYTES);
}
#[test]
fn duplicate_begin_preserves_command_and_response_overwrite_cannot_replace_it() {
    let mut owner = RequestOwner::new(9, 16, 16);
    assert_eq!(owner.instance(), 9);
    owner.begin(Request::Keyboard).unwrap();
    let original = owner.command().unwrap().to_vec();
    assert_eq!(owner.begin(Request::Focused), Err(OwnerError::Busy));
    assert_eq!(owner.command().unwrap(), original);
    let mut page = alloc::vec![0;wire::EXCHANGE_BYTES];
    let response = reply(&owner, Response::Refused(Error::Bounds));
    page[..response.len()].copy_from_slice(&response);
    assert_eq!(owner.command().unwrap(), original);
    assert_eq!(
        owner.complete_exchange(&page),
        Ok(Response::Refused(Error::Bounds))
    );
    owner.begin(Request::Focused).unwrap();
    let (e, r) = wire::decode_request(owner.command().unwrap()).unwrap();
    assert_eq!(
        e,
        Envelope {
            instance: 9,
            request: 2
        }
    );
    assert_eq!(r, Request::Focused);
    assert_eq!(
        owner.complete_response(&response),
        Err(OwnerError::Protocol)
    );
    assert_eq!(owner.begin(Request::Keyboard), Err(OwnerError::Terminated));
}
#[test]
fn malformed_foreign_and_wrong_kind_packets_are_terminal() {
    for mutation in 0..6 {
        let mut owner = RequestOwner::new(3, 16, 16);
        owner
            .begin(Request::Initialize {
                width: 16,
                height: 16,
            })
            .unwrap();
        let mut bytes = reply(&owner, Response::Unit);
        match mutation {
            0 => {
                bytes[16] ^= 1;
            }
            1 => {
                bytes[24] ^= 1;
            }
            2 => {
                bytes.pop();
            }
            3 => {
                bytes.push(0);
            }
            4 => {
                bytes = reply(&owner, Response::Focused(None));
            }
            _ => {
                bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
            }
        }
        assert_eq!(
            owner.complete_response(&bytes),
            Err(OwnerError::Protocol),
            "mutation {mutation}"
        );
        assert_eq!(owner.command(), Err(OwnerError::Terminated));
    }
}
#[test]
fn fault_cancel_quarantine_and_ordinal_wrap_never_refill() {
    for pending in [false, true] {
        let mut owner = RequestOwner::new(4, 16, 16);
        if pending {
            owner.begin(Request::Keyboard).unwrap();
        }
        owner.quarantine();
        owner.quarantine();
        assert_eq!(owner.begin(Request::Keyboard), Err(OwnerError::Terminated));
    }
    let mut owner = RequestOwner::new(4, 16, 16);
    owner.ordinal = u64::MAX;
    assert_eq!(owner.begin(Request::Keyboard), Err(OwnerError::Exhausted));
    assert_eq!(owner.begin(Request::Keyboard), Err(OwnerError::Terminated));
}

#[test]
fn actual_numeric_frame_grant_is_validated_before_retirement_and_applied_once() {
    for invalid in [false, true] {
        let mut owner = RequestOwner::new(8, 16, 16);
        owner.begin(Request::BeginFrame).unwrap();
        let mut plan = FramePlan {
            sequence: 1,
            count: 1,
            rects: [FrameRect::default(); 16],
            conservative_fallback: false,
            cursor_visible: false,
            focus_visible: false,
        };
        plan.rects[0] = FrameRect {
            x: 0,
            y: 0,
            width: 2,
            height: 1,
        };
        let bytes = reply(&owner, Response::FrameStarted(plan));
        assert_eq!(
            owner.complete_response(&bytes),
            Ok(Response::FrameStarted(plan))
        );
        owner
            .begin(Request::FrameStep {
                sequence: 1,
                pixels: 2,
            })
            .unwrap();
        let span = FrameSpan {
            sequence: 1,
            x: if invalid { 1 } else { 0 },
            y: 0,
            window_offset: 0,
            pixels: 2,
            complete: true,
            selected_mask: 0,
            coverage: [0; 8],
        };
        let bytes = reply(&owner, Response::Frame(span));
        if invalid {
            assert_eq!(owner.complete_response(&bytes), Err(OwnerError::Protocol));
            assert_eq!(
                owner.begin(Request::AcknowledgeScanout { sequence: 1 }),
                Err(OwnerError::Terminated)
            );
        } else {
            assert_eq!(owner.complete_response(&bytes), Ok(Response::Frame(span)));
            owner
                .begin(Request::AcknowledgeScanout { sequence: 1 })
                .unwrap();
            let ack = reply(&owner, Response::Unit);
            assert_eq!(owner.complete_response(&ack), Ok(Response::Unit));
            // The exact production FrameGrant cannot acknowledge the same completed frame twice.
            assert_eq!(
                owner.begin(Request::AcknowledgeScanout { sequence: 1 }),
                Err(OwnerError::Protocol)
            );
        }
    }
}
#[test]
fn page_length_bounds_and_completion_without_live_request_are_terminal() {
    for length in [0, 31, 4097, u32::MAX] {
        let mut owner = RequestOwner::new(3, 16, 16);
        owner.begin(Request::Keyboard).unwrap();
        let mut page = alloc::vec![0;wire::EXCHANGE_BYTES];
        page[8..12].copy_from_slice(&length.to_le_bytes());
        assert_eq!(owner.complete_exchange(&page), Err(OwnerError::Protocol));
        assert_eq!(owner.begin(Request::Keyboard), Err(OwnerError::Terminated));
    }
    let mut owner = RequestOwner::new(3, 16, 16);
    assert_eq!(owner.complete_response(&[]), Err(OwnerError::Protocol));
    assert_eq!(owner.begin(Request::Keyboard), Err(OwnerError::Terminated));
}
