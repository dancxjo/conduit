use super::*;
use crate::allocation_probe::observe;
#[test]
fn exact_source_offer_reservation_precedes_all_allocations() {
    for definition in [
        "type EvenPeriod = U16 in 32..=255 where . % 2 == 0\ntype Allowed = {\n value: EvenPeriod\n}\n",
        "type Allowed = {\n a: U16\n b: U16\n where .a <= .b\n}\n",
    ] {
        let profile = PreparedNativeProfile::check_definition(definition, "Allowed").unwrap();
        for flow in [false, true] {
            let reservation = profile.offer_storage_reservation(flow).unwrap();
            for (p, r) in [
                (
                    reservation.preparation_requested_bytes_bound - 1,
                    reservation.retained_heap_bytes_bound,
                ),
                (
                    reservation.preparation_requested_bytes_bound,
                    reservation.retained_heap_bytes_bound - 1,
                ),
            ] {
                let (result, observed) = observe(|| profile.offer_with_storage_limits(flow, p, r));
                assert!(matches!(result, Err(NativeOfferStorageRefusal::Capacity)));
                assert_eq!((observed.allocations, observed.reallocations), (0, 0));
            }
            let (result, observed) = observe(|| {
                profile.offer_with_storage_limits(
                    flow,
                    reservation.preparation_requested_bytes_bound,
                    reservation.retained_heap_bytes_bound,
                )
            });
            let (offer, receipt) = result.unwrap();
            assert!(
                observed.requested_bytes <= receipt.preparation_requested_bytes_bound,
                "{} > {}",
                observed.requested_bytes,
                receipt.preparation_requested_bytes_bound
            );
            assert!(observed.peak_bytes <= receipt.preparation_requested_bytes_bound);
            assert_eq!(
                conduit_core::capability_offer_owned_heap_bytes(&offer).unwrap(),
                receipt.retained_heap_bytes
            );
            assert_eq!(observed.live_bytes, receipt.retained_heap_bytes);
            std::eprintln!(
                "offer flow={flow} requested={} live={} prep_bound={} retained_bound={}",
                observed.requested_bytes,
                observed.live_bytes,
                receipt.preparation_requested_bytes_bound,
                receipt.retained_heap_bytes_bound
            );
            assert_eq!(offer, profile.offer(flow).unwrap());
        }
    }
}
#[test]
fn bounded_native_back_retains_full_source_guards_and_refuses_without_consumption() {
    let profile=PreparedNativeProfile::check_definition("type EvenPeriod = U16 in 32..=255 where . % 2 == 0\ntype Allowed = {\n value: EvenPeriod\n}\n","Allowed").unwrap();
    let limits = NativeProfilePreparationLimits {
        maximum_preparation_requested_bytes: 64 * 1024 * 1024,
        maximum_retained_heap_bytes: 32 * 1024 * 1024,
        maximum_decoded_program_bytes: 8 * 1024 * 1024,
    };
    let (refusal, observed) = observe(|| {
        NativeProfileBack::prepare_selected_with_storage_limits(
            &profile,
            true,
            NativeProfilePreparationLimits {
                maximum_preparation_requested_bytes: 0,
                ..limits
            },
        )
    });
    assert!(matches!(
        refusal,
        Err(NativeProfilePreparationRefusal::Capacity)
    ));
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    let (result, observed) =
        observe(|| NativeProfileBack::prepare_selected_with_storage_limits(&profile, true, limits));
    let (mut back, receipt) = result.unwrap();
    assert!(
        observed.requested_bytes <= receipt.preparation_requested_bytes_bound,
        "requested {} > bound {}",
        observed.requested_bytes,
        receipt.preparation_requested_bytes_bound
    );
    assert!(observed.peak_bytes <= receipt.preparation_requested_bytes_bound);
    assert!(observed.live_bytes <= receipt.retained_accounted_heap_bytes);
    std::eprintln!(
        "back requested={} live={} prep_bound={} retained_bound={}",
        observed.requested_bytes,
        observed.live_bytes,
        receipt.preparation_requested_bytes_bound,
        receipt.retained_heap_bytes_bound
    );
    assert_eq!(
        receipt.retained_accounted_heap_bytes,
        back.owned_heap_bytes()
    );
    assert!(receipt.retained_accounted_heap_bytes <= receipt.retained_heap_bytes_bound);
    for (value, valid) in [(31u16, false), (33, false), (64, true), (256, false)] {
        let StructuredInfoTypeShape::Record { fields, .. } = profile.candidate_type().shape()
        else {
            panic!("original structured candidate required")
        };
        let field =
            StructuredInfoValue::leaf(fields[0].value_type().clone(), value.to_le_bytes().to_vec())
                .unwrap();
        let bytes = StructuredInfoValue::record(
            profile.candidate_type().clone(),
            vec![StructuredFieldValue::new("value", field).unwrap()],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let reference = conduit_kernel::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        };
        let mut io: StepIo<4> = StepIo::test_frame(
            [Some(reference), None, None, None],
            [false; 4],
            [Some(4096), None, None, None],
            None,
            2,
        );
        let inputs = StepInputBytes::test_frame([Some(bytes.as_slice()), None, None, None], None);
        let (out, observed) = observe(|| back.step(&mut io, &inputs));
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        if valid {
            assert_eq!(out, StepOutcome::Progress);
            let output =
                <NativeProfileBack as StepBack<4>>::prepared_output(&back, PortId(0)).unwrap();
            assert_eq!(
                StructuredInfoValue::from_canonical_bytes(output)
                    .unwrap()
                    .value_type(),
                profile.value_type()
            );
            <NativeProfileBack as StepBack<4>>::step_committed(&mut back);
        } else {
            assert!(matches!(out, StepOutcome::Fail(_)));
            assert!(!io.test_consumed(PortId(0)));
            assert!(
                <NativeProfileBack as StepBack<4>>::prepared_output(&back, PortId(0)).is_none()
            );
        }
    }
}

#[test]
fn nested_variant_collection_back_has_measured_complete_payload_inventory() {
    let profile=PreparedNativeProfile::check_definition("type Period = U16 in 32..=255 where . % 2 == 0\ntype Finite = F32 finite\ntype Choice =\n idle\n | active Period\ntype Allowed = {\n period: Period\n values: collection Finite = 2\n choice: Choice\n limit: U16\n where .period + 0 <= .limit\n}\n","Allowed").unwrap();
    let limits = NativeProfilePreparationLimits {
        maximum_preparation_requested_bytes: 64 * 1024 * 1024,
        maximum_retained_heap_bytes: 32 * 1024 * 1024,
        maximum_decoded_program_bytes: 8 * 1024 * 1024,
    };
    let (result, observed) =
        observe(|| NativeProfileBack::prepare_selected_with_storage_limits(&profile, true, limits));
    let (back, receipt) = result.unwrap();
    assert!(
        observed.requested_bytes <= receipt.preparation_requested_bytes_bound,
        "requested {} > {}",
        observed.requested_bytes,
        receipt.preparation_requested_bytes_bound
    );
    assert!(observed.live_bytes <= back.owned_heap_bytes());
    assert_eq!(
        back.owned_heap_bytes(),
        receipt.retained_accounted_heap_bytes
    );
    assert!(receipt.retained_accounted_heap_bytes <= receipt.retained_heap_bytes_bound);
    std::eprintln!(
        "nested back requested={} live={} prep_bound={} retained_bound={}",
        observed.requested_bytes,
        observed.live_bytes,
        receipt.preparation_requested_bytes_bound,
        receipt.retained_heap_bytes_bound
    );
}
