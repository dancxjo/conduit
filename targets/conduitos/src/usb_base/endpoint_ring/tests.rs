use super::*;

#[test]
fn bounded_receive_ring_reuses_every_slot_across_many_complete_cycles() {
    let mut cursor = EndpointRingCursor::new(64).unwrap();
    let allocations = crate::test_allocations::allocations(|| {
        for sequence in 0..10_000 {
            let reservation = cursor.reserve(2048).unwrap();
            assert_eq!(reservation.slot, sequence % 63);
            assert_eq!(reservation.cycle, 1 ^ ((sequence / 63) & 1) as u32);
            assert_eq!(
                reservation.normal(0x1234_5678_0000),
                [0x5678_0000, 0x1234, 2048, 1056 | reservation.cycle]
            );
            assert_eq!(
                reservation.link(0x1000),
                [0x1000, 0, 0, 6146 | reservation.cycle]
            );
            assert_eq!(cursor.reserve(8), Err(EndpointRingRefusal::Pending));
            assert_eq!(cursor.actual(&reservation, 0), Ok(2048));
            assert_eq!(cursor.actual(&reservation, 2048), Ok(0));
            assert_eq!(
                cursor.actual(&reservation, 2049),
                Err(EndpointRingRefusal::Residual)
            );
            assert_eq!(
                cursor.actual(&reservation, u32::MAX),
                Err(EndpointRingRefusal::Residual)
            );
            assert_eq!(cursor.ensure_idle(), Err(EndpointRingRefusal::Pending));
            // SAFETY: inert cursor fixture; no controller ever owns these words.
            unsafe { cursor.complete_quiesced(&reservation) }.unwrap();
            assert_eq!(
                unsafe { cursor.complete_quiesced(&reservation) },
                Err(EndpointRingRefusal::StaleCompletion)
            );
        }
    });
    assert_eq!(allocations, 0);
}

#[test]
fn malformed_geometry_or_length_never_consumes_a_reservation() {
    for slots in [0, 1, 4097, usize::MAX] {
        assert!(matches!(
            EndpointRingCursor::new(slots),
            Err(EndpointRingRefusal::Geometry)
        ));
    }
    let mut cursor = EndpointRingCursor::new(2).unwrap();
    for length in [0, 2049, u16::MAX] {
        assert_eq!(cursor.reserve(length), Err(EndpointRingRefusal::Geometry));
        assert_eq!(cursor.ensure_idle(), Ok(()));
    }
    let first = cursor.reserve(8).unwrap();
    assert_eq!((first.slot, first.cycle), (0, 1));
    unsafe { cursor.complete_quiesced(&first) }.unwrap();
    let second = cursor.reserve(8).unwrap();
    assert_eq!((second.slot, second.cycle), (0, 0));
    assert_eq!(
        unsafe { cursor.complete_quiesced(&first) },
        Err(EndpointRingRefusal::StaleCompletion)
    );
    assert_eq!(cursor.ensure_idle(), Err(EndpointRingRefusal::Pending));
}

#[test]
fn changed_completion_coordinates_and_sequence_exhaustion_preserve_ownership() {
    let mut cursor = EndpointRingCursor::new(64).unwrap();
    let mut reservation = cursor.reserve(8).unwrap();
    reservation.slot += 1;
    assert_eq!(
        cursor.actual(&reservation, 0),
        Err(EndpointRingRefusal::StaleCompletion)
    );
    assert_eq!(
        unsafe { cursor.complete_quiesced(&reservation) },
        Err(EndpointRingRefusal::StaleCompletion)
    );
    reservation.slot -= 1;
    reservation.length += 1;
    assert_eq!(
        unsafe { cursor.complete_quiesced(&reservation) },
        Err(EndpointRingRefusal::StaleCompletion)
    );
    reservation.length -= 1;
    reservation.cycle ^= 1;
    assert_eq!(
        unsafe { cursor.complete_quiesced(&reservation) },
        Err(EndpointRingRefusal::StaleCompletion)
    );
    reservation.cycle ^= 1;
    assert_eq!(cursor.ensure_idle(), Err(EndpointRingRefusal::Pending));
    unsafe { cursor.complete_quiesced(&reservation) }.unwrap();
    cursor.sequence = u64::MAX;
    assert_eq!(cursor.reserve(8), Err(EndpointRingRefusal::Exhausted));
    assert_eq!(cursor.ensure_idle(), Ok(()));
}

#[test]
fn unacknowledged_configuration_never_admits_a_transfer_or_a_second_configuration() {
    let mut cursor = EndpointRingCursor::new(64).unwrap();
    cursor.begin_configuration().unwrap();
    assert_eq!(cursor.ensure_idle(), Err(EndpointRingRefusal::Uncertain));
    assert_eq!(cursor.reserve(8), Err(EndpointRingRefusal::Uncertain));
    assert_eq!(
        cursor.begin_configuration(),
        Err(EndpointRingRefusal::Uncertain)
    );
    // SAFETY: inert fixture acknowledges its own exact configuration.
    unsafe { cursor.complete_configuration() }.unwrap();
    assert!(unsafe { cursor.complete_configuration() }.is_err());
    let transfer = cursor.reserve(8).unwrap();
    assert!(unsafe { cursor.complete_configuration() }.is_err());
    assert_eq!(cursor.ensure_idle(), Err(EndpointRingRefusal::Pending));
    unsafe { cursor.complete_quiesced(&transfer) }.unwrap();
    assert_eq!(
        cursor.begin_configuration(),
        Err(EndpointRingRefusal::Geometry)
    );
}
