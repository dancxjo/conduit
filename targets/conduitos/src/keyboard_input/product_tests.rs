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
    })
    .unwrap();

    assert_eq!(control, ProductInputControl::Continue);
    assert_eq!(usages, [4, 5]);
    assert_eq!(count, 2);
    assert!(!service_seen);
    assert_eq!(ingress.pending(), 0);
}

#[test]
fn product_service_runs_presentation_only_from_idle_ingress_turn() {
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
    })
    .unwrap();

    assert_eq!(control, ProductInputControl::Continue);
    assert!(service_seen);
}
