//! Finite WASM entrance for owner Face frames and exact Show acknowledgements.

use super::*;
use std::cell::{Cell, RefCell};
#[path = "abi/full.rs"]
mod full;

// The owner frame is bounded separately at 60 KiB; admission basis bytes
// accompany it at this WASM entrance without crossing the owner Line.
const INPUT_CAPACITY: usize = 72 * 1024;

thread_local! {
    static INPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; INPUT_CAPACITY].into_boxed_slice());
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(OUTPUT_CAPACITY));
    static CURRENT: RefCell<Option<OwnerBrowserMask>> = const { RefCell::new(None) };
    static FULL_INTERFACE: Cell<bool> = const { Cell::new(false) };
    static NEXT_PLAY_SEQUENCE: Cell<u64> = const { Cell::new(1) };
}

fn write_output(value: &impl Serialize) -> i32 {
    let Ok(bytes) = serde_json::to_vec(value) else {
        return -3;
    };
    if bytes.is_empty() || bytes.len() > OUTPUT_CAPACITY {
        return -4;
    }
    if FULL_INTERFACE.with(Cell::get) {
        OUTPUT.with(|output| {
            let mut output = output.borrow_mut();
            output.clear();
            output.extend_from_slice(&bytes);
        });
    } else {
        OUTPUT.with(|output| *output.borrow_mut() = bytes);
    }
    0
}
fn write_full_output(bytes: &[u8]) -> i32 {
    OUTPUT.with(|output| {
        let mut output = output.borrow_mut();
        if !FULL_INTERFACE.with(Cell::get) || bytes.is_empty() || bytes.len() > output.capacity() {
            return -4;
        }
        output.clear();
        output.extend_from_slice(bytes);
        0
    })
}
pub(super) fn prepare_full_interface() -> Result<(), String> {
    if FULL_INTERFACE.with(Cell::get) || CURRENT.with(|c| c.borrow().is_some()) {
        return Err("owner Full interface already live".into());
    }
    let input = super::full_input::input_capacity();
    let output = conduit_host_browser_make::OWNER_FACE_FULL_MAXIMUM_VIEW_BYTES as usize;
    let charge = INPUT_CAPACITY
        + OUTPUT_CAPACITY
        + 2 * input
        + output
        + core::mem::size_of::<Vec<u8>>()
        + core::mem::size_of::<Box<[u8]>>();
    super::full_profile_pool::admit_bootstrap(charge)?;
    let mut prepared_input = Vec::new();
    prepared_input
        .try_reserve_exact(input)
        .map_err(|_| "Full input preparation unavailable")?;
    prepared_input.resize(input, 0);
    let prepared_input = prepared_input.into_boxed_slice();
    let mut prepared_output = Vec::new();
    prepared_output
        .try_reserve_exact(output)
        .map_err(|_| "Full output preparation unavailable")?;
    INPUT.with(|i| *i.borrow_mut() = prepared_input);
    OUTPUT.with(|o| *o.borrow_mut() = prepared_output);
    FULL_INTERFACE.with(|ready| ready.set(true));
    super::full_profile_pool::mark_interface_ready(input, output)?;
    Ok(())
}
pub(super) fn reset_full_interface() -> Result<(), String> {
    if !FULL_INTERFACE.with(Cell::get) {
        return Ok(());
    }
    if CURRENT.with(|c| c.borrow().is_some()) {
        return Err("owner Mask retains interface custody".into());
    }
    if FULL_INTERFACE.with(Cell::get) {
        INPUT.with(|i| *i.borrow_mut() = vec![0; INPUT_CAPACITY].into_boxed_slice());
        OUTPUT.with(|o| *o.borrow_mut() = Vec::with_capacity(OUTPUT_CAPACITY));
        FULL_INTERFACE.with(|ready| ready.set(false));
    }
    Ok(())
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_full_profile_ready() -> i32 {
    i32::from(FULL_INTERFACE.with(Cell::get) && super::full_profile_pool::interface_ready())
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_full_maximum_frame_bytes() -> usize {
    if conduit_browser_owner_face_full_profile_ready() == 1 {
        conduit_host_browser_make::OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES as usize
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_input_ptr() -> usize {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_input_capacity() -> usize {
    INPUT.with(|i| i.borrow().len())
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
    if FULL_INTERFACE.with(Cell::get) {
        OUTPUT.with(|o| o.borrow().capacity())
    } else {
        OUTPUT_CAPACITY
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_prepare(basis_len: usize, frame_len: usize) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    let Some(total_len) = basis_len.checked_add(frame_len) else {
        return -1;
    };
    if basis_len == 0 || frame_len == 0 || total_len > conduit_browser_owner_face_input_capacity() {
        return -1;
    }
    if let Some(result) = full::try_prepare(basis_len, frame_len) {
        return result;
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
            face,
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
            let Ok(mask) =
                OwnerBrowserMask::prepare(basis, *face, *route, sequence, interactions_admitted)
            else {
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
                                previous.face.revision
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
                || mask.face.revision != revision
                || mask.face.identity != identity
            {
                return -6;
            }
            if mask.requires_full_visual_snapshot
                || mask
                    .component
                    .as_ref()
                    .is_some_and(|value| value.local_look.is_some())
                || basis
                    .visual_constraints
                    .as_ref()
                    .is_some_and(|value| value.requires_full_snapshot())
            {
                return write_output(&serde_json::json!({
                    "schema": "conduit.browser/owner-face-refusal@1",
                    "code": "owner-visual-preferences-require-full-snapshot",
                }));
            }
            if let Some(full) = mask.full.as_ref() {
                write_full_output(full.view(mask.show.show.lifecycle))
            } else {
                write_output(&mask.view())
            }
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
    let selected_full = CURRENT.with(|c| c.borrow().as_ref().is_some_and(|m| m.full.is_some()));
    if selected_full {
        let result = INPUT.with(|input| {
            let input = input.borrow();
            let ack = match super::full_ack::parse(&input[..length]) {
                Ok(v) => v,
                Err(_) => return -2,
            };
            CURRENT.with(|current| {
                let mut current = current.borrow_mut();
                let Some(mask) = current.as_mut() else {
                    return -6;
                };
                if mask.acknowledge_full(ack).is_err() {
                    return -5;
                }
                write_full_output(
                    mask.full
                        .as_ref()
                        .expect("selected Full")
                        .view(mask.show.show.lifecycle),
                )
            })
        });
        INPUT.with(|i| i.borrow_mut()[..length].fill(0));
        return result;
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
        if let Some(full) = mask.full.as_ref() {
            write_full_output(&full.receipt)
        } else {
            write_output(&mask.show)
        }
    })
}

/// Read the retained original Face without the lossy human-readable projection.
/// This is observation only; it neither refreshes nor acknowledges a Show.
#[no_mangle]
pub extern "C" fn conduit_browser_owner_face_canonical_snapshot() -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    CURRENT.with(|current| {
        let current = current.borrow();
        let Some(mask) = current.as_ref() else {
            return -6;
        };
        if mask.show.show.lifecycle != ManifestationLifecycle::Available {
            return -5;
        }
        if let Some(full) = mask.full.as_ref() {
            return write_full_output(&full.canonical);
        }
        write_output(&serde_json::json!({
            "schema": "conduit.browser/owner-canonical-face@1",
            "face": &mask.face,
            "face_revision_decimal": mask.face.revision.to_string(),
            "route": &mask.route,
            "show_id": mask.show.show_id.as_str(),
        }))
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
#[path = "abi/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "abi/visual_preference_tests.rs"]
mod visual_preference_tests;
