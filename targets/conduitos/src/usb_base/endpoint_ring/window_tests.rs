use super::*;

#[test]
fn eight_admitted_reservations_preserve_order_pressure_and_reuse_without_growth() {
    let mut cursor = EndpointRingCursor::with_maximum_pending(64, 8).unwrap();
    let mut retained: [Option<EndpointRingReservation>; 8] = core::array::from_fn(|_| None);
    let allocations = crate::test_allocations::allocations(|| {
        for slot in &mut retained {
            *slot = Some(cursor.reserve(8).unwrap());
        }
        for sequence in 0..10_000 {
            assert_eq!(cursor.pending_count(), 8);
            assert_eq!(cursor.reserve(8), Err(EndpointRingRefusal::Pending));
            assert_eq!(
                cursor.begin_configuration(),
                Err(EndpointRingRefusal::Pending)
            );
            let head = sequence % retained.len();
            let later = retained[(head + 1) % retained.len()].as_ref().unwrap();
            let position = cursor.position();
            assert_eq!(
                cursor.actual(later, 0),
                Err(EndpointRingRefusal::StaleCompletion)
            );
            // SAFETY: inert cursor fixture, with no native DMA or controller.
            assert_eq!(
                unsafe { cursor.complete_quiesced(later) },
                Err(EndpointRingRefusal::StaleCompletion)
            );
            assert_eq!(cursor.position(), position);
            assert_eq!(cursor.pending_count(), 8);
            let first = retained[head].take().unwrap();
            assert_eq!(
                (first.slot, first.cycle),
                (sequence % 63, 1 ^ ((sequence / 63) & 1) as u32)
            );
            assert_eq!(cursor.actual(&first, 0), Ok(8));
            assert_eq!(cursor.actual(&first, 9), Err(EndpointRingRefusal::Residual));
            unsafe { cursor.complete_quiesced(&first) }.unwrap();
            assert_eq!(cursor.pending_count(), 7);
            assert_eq!(
                unsafe { cursor.complete_quiesced(&first) },
                Err(EndpointRingRefusal::StaleCompletion)
            );
            retained[head] = Some(cursor.reserve(8).unwrap());
        }
        for sequence in 10_000..10_008 {
            let head = sequence % retained.len();
            let first = retained[head].take().unwrap();
            unsafe { cursor.complete_quiesced(&first) }.unwrap();
        }
        assert_eq!(cursor.pending_count(), 0);
        assert_eq!(cursor.ensure_idle(), Ok(()));
        assert_eq!(
            cursor.position(),
            (10_008 % 63, 1 ^ ((10_008 / 63) & 1) as u32)
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn capture_bound_fits_the_ring_and_never_changes_the_single_transfer_default() {
    assert_eq!(EndpointRingCursor::new(64).unwrap().maximum_pending(), 1);
    for (slots, bound) in [(64, 0), (64, 9), (8, 8), (2, 2), (1, 1), (4097, 1)] {
        assert!(matches!(
            EndpointRingCursor::with_maximum_pending(slots, bound),
            Err(EndpointRingRefusal::Geometry)
        ));
    }
    let mut cursor = EndpointRingCursor::with_maximum_pending(9, 8).unwrap();
    cursor.begin_configuration().unwrap();
    assert_eq!(cursor.reserve(8), Err(EndpointRingRefusal::Uncertain));
    // SAFETY: this fixture acknowledges its own inert configuration.
    unsafe { cursor.complete_configuration() }.unwrap();
    let mut retained: [Option<EndpointRingReservation>; 8] = core::array::from_fn(|_| None);
    for slot in &mut retained {
        *slot = Some(cursor.reserve(8).unwrap());
    }
    let first = retained[0].take().unwrap();
    unsafe { cursor.complete_quiesced(&first) }.unwrap();
    let position = cursor.position();
    assert_eq!(cursor.reserve(0), Err(EndpointRingRefusal::Geometry));
    assert_eq!(cursor.position(), position);
    assert_eq!(cursor.pending_count(), 7);
    cursor.sequence = u64::MAX;
    assert_eq!(cursor.reserve(8), Err(EndpointRingRefusal::Exhausted));
    assert_eq!(cursor.position(), position);
    assert_eq!(cursor.pending_count(), 7);
    assert_eq!(cursor.ensure_idle(), Err(EndpointRingRefusal::Pending));
}
