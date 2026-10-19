//! Product callback ordering over the existing finite ingress.
use super::*;
use crate::arch::HidKeyTransition;

#[test]
fn product_service_prioritizes_portable_queue_before_service_phase() {
    let mut ingress = KeyboardIngress::new();
    ingress.admit(HidKeyTransition::new(4, true, 0)).unwrap();
    ingress.admit(HidKeyTransition::new(5, true, 0)).unwrap();

    let mut usages = [0_u8; 2];
    let mut count = 0;
    let mut service_seen = false;
    let control = service_product_ingress(&mut ingress, &mut |event| match event {
        ProductInputEvent::Key(event) => {
            usages[count] = event.usage();
            count += 1;
            Ok(ProductInputControl::Continue)
        }
        ProductInputEvent::Service => {
            service_seen = true;
            Ok(ProductInputControl::Continue)
        }
        ProductInputEvent::LocalRescue(_) => panic!("unexpected local-rescue observation"),
        ProductInputEvent::Lost(_) => panic!("unexpected loss"),
        ProductInputEvent::Pointer(_) | ProductInputEvent::PointerLost(_) => {
            panic!("unexpected pointer event")
        }
    })
    .unwrap();

    assert_eq!(control, ProductInputControl::Continue);
    assert_eq!(usages, [4, 5]);
    assert_eq!(count, 2);
    assert!(!service_seen);
    assert_eq!(ingress.pending(), 0);
}

#[test]
fn product_service_runs_face_only_from_idle_ingress_turn() {
    let mut ingress = KeyboardIngress::new();
    let mut service_seen = false;
    let control = service_product_ingress(&mut ingress, &mut |event| match event {
        ProductInputEvent::Service => {
            service_seen = true;
            Ok(ProductInputControl::Continue)
        }
        ProductInputEvent::Key(_) => panic!("unexpected keyboard delivery"),
        ProductInputEvent::LocalRescue(_) => panic!("unexpected local-rescue observation"),
        ProductInputEvent::Lost(_) => panic!("unexpected loss"),
        ProductInputEvent::Pointer(_) | ProductInputEvent::PointerLost(_) => {
            panic!("unexpected pointer event")
        }
    })
    .unwrap();

    assert_eq!(control, ProductInputControl::Continue);
    assert!(service_seen);
}

#[test]
fn pointer_loss_and_retained_sample_precede_keyboard_handoff() {
    let mut ingress = KeyboardIngress::new();
    ingress.admit(HidKeyTransition::new(69, false, 0)).unwrap();
    let mut pending = Some(conduit_semantic_catalog::NormalizedPointerSample {
        position_x: 500_000,
        position_y: 500_000,
        delta_x: 0,
        delta_y: 0,
        primary_pressed: true,
        sequence: 1,
        coalesced: 0,
        dropped: 0,
        queue_capacity: 2,
    });
    let mut events = alloc::vec::Vec::new();
    let result = service_product_inputs(
        &mut ingress,
        &mut pending,
        Some("mouse-removed"),
        &mut |event| {
            match event {
                ProductInputEvent::PointerLost(reason) => {
                    assert_eq!(reason, "mouse-removed");
                    events.push("loss");
                }
                ProductInputEvent::Pointer(sample) => {
                    assert!(sample.primary_pressed);
                    events.push("pointer");
                }
                ProductInputEvent::Key(_) => {
                    events.push("keyboard-yield");
                    return Ok(ProductInputControl::Yield);
                }
                _ => panic!("unexpected service or rescue occurrence"),
            }
            Ok(ProductInputControl::Continue)
        },
    )
    .unwrap();
    assert_eq!(result, ProductInputControl::Yield);
    assert_eq!(events, ["loss", "pointer", "keyboard-yield"]);
    assert!(pending.is_none());
}

#[test]
fn loss_requested_handoff_finishes_retained_physical_batch_without_keyboard_work() {
    let mut ingress = KeyboardIngress::new();
    ingress.admit(HidKeyTransition::new(4, true, 0)).unwrap();
    let mut pointer = Some(conduit_semantic_catalog::NormalizedPointerSample {
        position_x: 0,
        position_y: 0,
        delta_x: 0,
        delta_y: 0,
        primary_pressed: false,
        sequence: 1,
        coalesced: 0,
        dropped: 0,
        queue_capacity: 2,
    });
    let mut retired = false;
    let result = service_product_inputs(
        &mut ingress,
        &mut pointer,
        Some("removed"),
        &mut |event| match event {
            ProductInputEvent::PointerLost(_) => {
                retired = true;
                Ok(ProductInputControl::Yield)
            }
            ProductInputEvent::Pointer(_) => {
                assert!(retired);
                Ok(ProductInputControl::Continue)
            }
            _ => panic!("handoff must not drain keyboard or run service"),
        },
    )
    .unwrap();
    assert_eq!(result, ProductInputControl::Yield);
    assert!(pointer.is_none());
    assert_eq!(ingress.pending(), 1);
}
