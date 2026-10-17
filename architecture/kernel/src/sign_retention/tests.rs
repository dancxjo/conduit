use super::*;
use crate::NodeId;
use core::cell::Cell;

fn event(sticky: bool) -> KernelEvent {
    KernelEvent {
        sequence: 0,
        node: NodeId(0),
        port: None,
        request: None,
        kind: if sticky {
            KernelEventKind::BackCompleted
        } else {
            KernelEventKind::ValueConsumed
        },
    }
}

#[test]
fn physical_headroom_does_not_visit_history() {
    let visits = Cell::new(0);
    let events = core::iter::once(event(true)).inspect(|_| visits.set(visits.get() + 1));
    let charge = size_of::<KernelEvent>() as u32;
    assert_eq!(
        ensure_retained_sign_capacity(events, 8, 8 * charge, 1, charge, 1),
        Ok(())
    );
    assert_eq!(visits.get(), 0);
}

#[test]
fn pressured_logs_preserve_retained_history_admission() {
    let charge = size_of::<KernelEvent>() as u32;
    for capacity in 1u16..=8 {
        for occupied in 0..=capacity {
            for pattern in 0u16..(1u16 << occupied) {
                let retained = (pattern & ((1u16 << occupied) - 1)).count_ones();
                for additional in 0..=capacity + 1 {
                    for byte_items in 0..=capacity + 1 {
                        let expected = if retained + u32::from(additional) > u32::from(capacity) {
                            Err(SignError::ItemCapacityExceeded)
                        } else if retained + u32::from(additional) > u32::from(byte_items) {
                            Err(SignError::ByteCapacityExceeded)
                        } else {
                            Ok(())
                        };
                        let events = (0..occupied).map(|index| event(pattern & (1 << index) != 0));
                        assert_eq!(ensure_retained_sign_capacity(events, capacity,
                            u32::from(byte_items) * charge, occupied,
                            u32::from(occupied) * charge, additional), expected,
                            "capacity={capacity}, occupied={occupied}, pattern={pattern}, additional={additional}, byte_items={byte_items}");
                    }
                }
            }
        }
    }
}

#[test]
fn overflowing_physical_count_uses_retained_history_rule() {
    let charge = size_of::<KernelEvent>() as u32;
    assert_eq!(
        ensure_retained_sign_capacity(
            core::iter::once(event(false)),
            u16::MAX,
            u32::from(u16::MAX) * charge,
            1,
            charge,
            u16::MAX
        ),
        Ok(())
    );
}

#[test]
fn physical_item_or_byte_pressure_inspects_retained_history() {
    let charge = size_of::<KernelEvent>() as u32;
    for (capacity, byte_capacity, expected) in [
        (2, 3 * charge, Ok(())),
        (3, 2 * charge, Ok(())),
        (3, charge, Err(SignError::ByteCapacityExceeded)),
    ] {
        let visits = Cell::new(0);
        let events = [event(true), event(false)]
            .into_iter()
            .inspect(|_| visits.set(visits.get() + 1));
        assert_eq!(
            ensure_retained_sign_capacity(events, capacity, byte_capacity, 2, 2 * charge, 1),
            expected
        );
        assert_eq!(visits.get(), 2);
    }
}
