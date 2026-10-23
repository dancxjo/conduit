use super::*;
type Fixture = Slot<4096, 4096, 4096, 4096, 4096, 4096, 2>;
const COUNTS: [u64; 12] = [
    4096, 4096, 4096, 8192, 4096, 4096, 4096, 4096, 4096, 4096, 4096, 16384,
];
fn poison(slot: &mut Fixture, region: usize, position: usize) {
    match region {
        0 => slot.pml4.0[position] = 0x100,
        1 => slot.pdpt.0[position] = 0x100,
        2 => slot.pd.0[position] = 0x100,
        3 => slot.pt[position / 512].0[position % 512] = 0x100,
        _ => {
            let mut view = slot.view();
            let data = match region {
                4 => &mut view.image,
                5 => &mut view.exchange,
                6 => &mut view.output,
                7 => &mut view.stack,
                8 => &mut view.metadata,
                9 => &mut view.pixels,
                10 => &mut view.scratch,
                11 => &mut view.trap,
                _ => unreachable!(),
            };
            data[position] = 0x80;
        }
    }
}
#[test]
fn every_table_and_data_region_is_checked_at_first_middle_last_without_short_circuit() {
    let mut slot = Fixture::empty();
    for region in 0..ZERO_REGIONS {
        let length = if region < 4 {
            COUNTS[region] as usize / 8
        } else {
            COUNTS[region] as usize
        };
        for position in [0, length / 2, length - 1] {
            slot.view().clear();
            poison(&mut slot, region, position);
            let observed = scan(&slot);
            for (index, result) in observed.iter().enumerate() {
                assert_eq!(result.expected_bytes, COUNTS[index]);
                assert_eq!(result.checked_bytes, COUNTS[index]);
                assert_eq!(result.all_zero, index != region);
            }
            assert_eq!(
                observed.iter().map(|r| r.checked_bytes).sum::<u64>(),
                size_of::<Fixture>() as u64
            );
        }
    }
    // Multiple simultaneous nonzero regions still report each whole region.
    for region in 0..ZERO_REGIONS {
        poison(&mut slot, region, 0);
    }
    assert!(scan(&slot).iter().all(|r| !r.all_zero));
    slot.view().clear();
    assert!(scan(&slot).iter().all(|r| r.all_zero));
}
#[test]
fn every_real_profile_defined_fields_equal_reserved_storage_without_padding_claims() {
    for profile in [
        ServiceProfile::Compositor,
        ServiceProfile::Draft,
        ServiceProfile::Home,
        ServiceProfile::Runtime,
        ServiceProfile::Application,
        ServiceProfile::Network,
        ServiceProfile::OrdinaryRuntime,
        ServiceProfile::Mask,
        ServiceProfile::Tls,
    ] {
        let layout = profile.layout();
        let tables = 3 + (layout.end() as usize).div_ceil(2 * 1024 * 1024);
        let defined = tables as u64 * 4096
            + layout.image().bytes()
            + layout.exchange().bytes()
            + layout.output().bytes()
            + layout.stack().bytes()
            + layout.metadata().bytes()
            + layout.pixels().map_or(0, |r| r.bytes())
            + layout.scratch().map_or(0, |r| r.bytes())
            + 16384;
        assert_eq!(defined, u64::from(reserved(profile)), "{profile:?}");
    }
    type EmptyOptional = Slot<4096, 4096, 4096, 4096, 0, 0, 1>;
    let slot = EmptyOptional::empty();
    let result = scan(&slot);
    assert_eq!(
        result[9],
        ZeroRegionResult {
            expected_bytes: 0,
            checked_bytes: 0,
            all_zero: true
        }
    );
    assert_eq!(result[10], result[9]);
}
#[test]
fn actual_lease_clear_before_release_is_correlated_once_and_foreign_profiles_do_not_consume() {
    assert_eq!(
        arm_service_zero_witness(ServiceProfile::Compositor, 0),
        Err(ZeroObserverError::InvalidIdentity)
    );
    let token = arm_service_zero_witness(ServiceProfile::Compositor, 42).unwrap();
    assert!(token.ordinal() > 0);
    assert_eq!(
        arm_service_zero_witness(ServiceProfile::Compositor, 43),
        Err(ZeroObserverError::Busy)
    );
    assert_eq!(
        take_service_zero_witness(ServiceZeroToken(token.0 + 1)),
        Err(ZeroObserverError::WrongToken)
    );
    observe_release(ServiceProfile::Draft); // No matching profile: no read of that store.
    let mut lease = Lease::claim(ServiceProfile::Compositor).unwrap();
    {
        let view = lease.view();
        view.image[0] = 1;
        view.exchange[4095] = 1;
        view.trap[16383] = 1;
    }
    drop(lease);
    assert_eq!(
        take_service_zero_witness(ServiceZeroToken(token.0 + 1)),
        Err(ZeroObserverError::WrongToken)
    );
    let witness = take_service_zero_witness(token).unwrap();
    assert_eq!(witness.profile, ServiceProfile::Compositor);
    assert_eq!(witness.instance, 42);
    assert_eq!(witness.token, token);
    assert!(witness.counts_valid && witness.all_zero);
    assert_eq!(witness.checked_bytes, witness.storage_bytes);
    assert!(!owned(ServiceProfile::Compositor).load(Ordering::Acquire));
    assert_eq!(
        take_service_zero_witness(token),
        Err(ZeroObserverError::Missing)
    );
    // No hidden view dirtied the clean marker; release permits a new owner.
    let next = Lease::claim(ServiceProfile::Compositor).unwrap();
    assert!(next.cleared, "optimized clean claim remains intact");
    drop(next);
    let token = arm_service_zero_witness(ServiceProfile::Compositor, 44).unwrap();
    assert_eq!(
        take_service_zero_witness(token),
        Err(ZeroObserverError::Incomplete)
    );
    assert_eq!(
        take_service_zero_witness(token),
        Err(ZeroObserverError::Missing)
    );
}
#[test]
fn missing_wrong_and_duplicate_tokens_never_destroy_another_pending_result() {
    let token = ServiceZeroToken(7);
    let arm = Arm {
        token,
        profile: ServiceProfile::Tls,
        instance: 8,
    };
    let mut state = State {
        next: 8,
        arm: Some(arm),
        result: None,
    };
    assert_eq!(
        take(&mut state, ServiceZeroToken(9)),
        Err(ZeroObserverError::WrongToken)
    );
    assert!(state.arm.is_some());
    assert_eq!(take(&mut state, token), Err(ZeroObserverError::Incomplete));
    assert_eq!(take(&mut state, token), Err(ZeroObserverError::Missing));
    assert!(SERVICE_ZERO_OBSERVER_BYTES <= 512);
}
