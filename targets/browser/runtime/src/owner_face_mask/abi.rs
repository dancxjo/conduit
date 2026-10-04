//! Finite WASM entrance for owner Face frames and exact Show acknowledgements.

use super::*;
use std::cell::{Cell, RefCell};

const INPUT_CAPACITY: usize = 40 * 1024;
const OUTPUT_CAPACITY: usize = 64 * 1024;

thread_local! {
    static INPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; INPUT_CAPACITY].into_boxed_slice());
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(OUTPUT_CAPACITY));
    static CURRENT: RefCell<Option<OwnerBrowserMask>> = const { RefCell::new(None) };
    static NEXT_PLAY_SEQUENCE: Cell<u64> = const { Cell::new(1) };
}

fn write_output(value: &impl Serialize) -> i32 {
    let Ok(bytes) = serde_json::to_vec(value) else {
        return -3;
    };
    if bytes.is_empty() || bytes.len() > OUTPUT_CAPACITY {
        return -4;
    }
    OUTPUT.with(|output| *output.borrow_mut() = bytes);
    0
}

#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_input_capacity() -> usize {
    INPUT_CAPACITY
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_output_ptr() -> usize {
    OUTPUT.with(|output| output.borrow().as_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_output_len() -> usize {
    OUTPUT.with(|output| output.borrow().len())
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_output_capacity() -> usize {
    OUTPUT_CAPACITY
}

#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_prepare(basis_len: usize, frame_len: usize) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    let Some(total_len) = basis_len.checked_add(frame_len) else {
        return -1;
    };
    if basis_len == 0 || frame_len == 0 || total_len > INPUT_CAPACITY {
        return -1;
    }
    let decoded = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let basis = serde_json::from_slice::<HostBasis>(&input[..basis_len]);
        let frame = serde_json::from_slice::<OwnerFrame>(&input[basis_len..basis_len + frame_len]);
        input[..total_len].fill(0);
        (basis, frame)
    });
    let (Ok(basis), Ok(frame)) = decoded else {
        return -2;
    };
    if frame.kind != "face-snapshot-response" || frame.protocol != 1 {
        return -2;
    }
    match frame.response {
        OwnerFaceSnapshotResponse::Snapshot {
            schema,
            presentation,
            interactions_admitted,
            route,
        } if schema == OWNER_FACE_RESPONSE_SCHEMA => {
            if CURRENT.with(|current| {
                current.borrow().as_ref().is_some_and(|mask| {
                    mask.show.show.lifecycle == ManifestationLifecycle::Prepared
                })
            }) {
                return -8;
            }
            let Some(sequence) = NEXT_PLAY_SEQUENCE.with(|next| {
                let current = next.get();
                next.set(current.saturating_add(1));
                (current < u64::MAX).then_some(current)
            }) else {
                return -8;
            };
            let Some(route) = route else {
                return -5;
            };
            let Ok(mask) = OwnerBrowserMask::prepare(
                basis,
                *presentation,
                *route,
                sequence,
                interactions_admitted,
            ) else {
                return -5;
            };
            let result = write_output(&mask.view());
            if result == 0 {
                let replaced = CURRENT.with(|current| {
                    let mut current = current.borrow_mut();
                    if let Some(previous) = current.as_mut() {
                        let next_show = match previous.show.transition(
                            ManifestationLifecycle::Replaced,
                            SignId::from(format!(
                                "sign/browser-owner-face-replaced/{}",
                                previous.presentation.revision
                            )),
                        ) {
                            Ok(show) => show,
                            Err(_) => return false,
                        };
                        if previous.scheduler.cancel().is_err() {
                            return false;
                        }
                        previous.show = next_show;
                    }
                    *current = Some(mask);
                    true
                });
                if !replaced {
                    return -9;
                }
            }
            result
        }
        OwnerFaceSnapshotResponse::Unchanged {
            schema,
            revision,
            identity,
        } if schema == OWNER_FACE_RESPONSE_SCHEMA => CURRENT.with(|current| {
            let current = current.borrow();
            let Some(mask) = current.as_ref() else {
                return -6;
            };
            if mask.body_id != basis.body_id
                || mask.play.host_id != basis.host_id
                || mask.play.boot_id != basis.boot_id
                || mask.presentation.revision != revision
                || mask.presentation.identity != identity
            {
                return -6;
            }
            write_output(&mask.view())
        }),
        OwnerFaceSnapshotResponse::Refused { schema, code }
            if schema == OWNER_FACE_RESPONSE_SCHEMA && !code.is_empty() && code.len() <= 128 =>
        {
            write_output(&serde_json::json!({
                "schema": "conduit.browser/owner-face-refusal@1", "code": code,
            }))
        }
        _ => -2,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_interact(length: usize) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    if length == 0 || length > INPUT_CAPACITY {
        return -1;
    }
    let proposed = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let value = serde_json::from_slice::<ProposedInteraction>(&input[..length]);
        input[..length].fill(0);
        value
    });
    let Ok(proposed) = proposed else {
        return -2;
    };
    CURRENT.with(|current| {
        let mut current = current.borrow_mut();
        let Some(mask) = current.as_mut() else {
            return -6;
        };
        match mask.interact(proposed) {
            Ok(emission) => write_output(&emission),
            Err(_) => -5,
        }
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_ack(length: usize) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    if length == 0 || length > INPUT_CAPACITY {
        return -1;
    }
    let ack = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let value = serde_json::from_slice::<Acknowledgement>(&input[..length]);
        input[..length].fill(0);
        value
    });
    let Ok(ack) = ack else {
        return -2;
    };
    CURRENT.with(|current| {
        let mut current = current.borrow_mut();
        let Some(mask) = current.as_mut() else {
            return -6;
        };
        if mask.acknowledge(ack).is_err() {
            return -5;
        }
        write_output(&mask.view())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_show_receipt() -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    CURRENT.with(|current| {
        let current = current.borrow();
        let Some(mask) = current.as_ref() else {
            return -6;
        };
        if mask.show.show.lifecycle != ManifestationLifecycle::Available {
            return -5;
        }
        write_output(&mask.show)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_clear() {
    CURRENT.with(|current| {
        if let Some(mut mask) = current.borrow_mut().take() {
            let _ = mask.scheduler.cancel();
        }
    });
    OUTPUT.with(|output| output.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::Body;
    use conduit_core::{CheckedPlotId, SourceDocumentId};
    use conduit_presentation::{Face, FaceContext, FaceFocus};

    fn send(basis: &serde_json::Value, response: &OwnerFaceSnapshotResponse) -> i32 {
        let basis = serde_json::to_vec(basis).unwrap();
        let frame = serde_json::to_vec(&serde_json::json!({
            "kind": "face-snapshot-response", "protocol": 1, "response": response,
        }))
        .unwrap();
        assert!(basis.len() + frame.len() <= INPUT_CAPACITY);
        INPUT.with(|input| {
            let mut input = input.borrow_mut();
            input[..basis.len()].copy_from_slice(&basis);
            input[basis.len()..basis.len() + frame.len()].copy_from_slice(&frame);
        });
        conduit_browser_owner_face_prepare(basis.len(), frame.len())
    }

    #[test]
    fn raw_owner_face_frame_refuses_a_snapshot_without_an_owner_route() {
        conduit_browser_owner_face_clear();
        let body = Body::born(
            SourceDocumentId::from("source/browser-owner-abi"),
            CheckedPlotId::from("checked/browser-owner-abi"),
            1,
            SignId::from("sign/browser-owner-abi-birth"),
        )
        .unwrap();
        let revision = 9_007_199_254_740_993;
        let face = Face::project(
            &body,
            None,
            revision,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
        )
        .unwrap()
        .presentation;
        let basis = serde_json::json!({
            "body_id": body.body_id,
            "host_id": "host/browser-owner-abi",
            "boot_id": "boot/browser-owner-abi",
        });
        assert_eq!(
            send(
                &basis,
                &OwnerFaceSnapshotResponse::Snapshot {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    presentation: Box::new(face.clone()),
                    interactions_admitted: false,
                    route: None,
                }
            ),
            -5
        );
        assert_eq!(
            send(
                &basis,
                &OwnerFaceSnapshotResponse::Unchanged {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    revision,
                    identity: face.identity,
                }
            ),
            -6
        );
        conduit_browser_owner_face_clear();
    }
}
