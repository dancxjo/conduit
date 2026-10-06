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

#[cfg(target_arch = "x86_64")]
#[test]
fn source_batch_retains_pressure_and_yield_without_duplicate_delivery() {
    let mut batch =
        crate::source_keyboard_batch::fixture_pending_batch(core::array::from_fn(|index| {
            HidKeyTransition::new(4 + index as u8, true, 0)
        }));
    let mut ingress = KeyboardIngress::new();
    for _ in 0..INGRESS_CAPACITY {
        ingress.admit(HidKeyTransition::new(30, true, 0)).unwrap();
    }
    let mut delivered = alloc::vec::Vec::new();
    service_source_batch(&mut batch, &mut ingress, &mut |event| {
        match event {
            ProductInputEvent::Key(key) => delivered.push(key.usage()),
            ProductInputEvent::LocalRescue(_) => {
                panic!("pressure must retain the Source transition")
            }
            ProductInputEvent::Service => panic!("queued keys precede service"),
            ProductInputEvent::Lost(_) => panic!("unexpected loss"),
        }
        Ok(ProductInputControl::Continue)
    })
    .unwrap();
    assert_eq!(batch.pending(), 20);
    assert_eq!(delivered, alloc::vec![30; INGRESS_CAPACITY]);
    delivered.clear();
    assert_eq!(
        service_source_batch(&mut batch, &mut ingress, &mut |event| {
            assert!(matches!(event, ProductInputEvent::LocalRescue(_)));
            Ok(ProductInputControl::Yield)
        })
        .unwrap(),
        ProductInputControl::Yield
    );
    assert_eq!(batch.pending(), 19);
    assert_eq!(ingress.pending(), 1);
    let mut rescue_count = 1;
    while batch.pending() != 0 || ingress.pending() != 0 {
        service_source_batch(&mut batch, &mut ingress, &mut |event| {
            match event {
                ProductInputEvent::Key(key) => delivered.push(key.usage()),
                ProductInputEvent::LocalRescue(_) => rescue_count += 1,
                ProductInputEvent::Service => panic!("queued keys precede service"),
                ProductInputEvent::Lost(_) => panic!("unexpected loss"),
            }
            Ok(ProductInputControl::Continue)
        })
        .unwrap();
    }
    assert_eq!(rescue_count, 20);
    assert_eq!(delivered, (4_u8..24).collect::<alloc::vec::Vec<_>>());
}
