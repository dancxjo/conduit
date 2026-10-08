use conduit_composite::FlowZipBack;
use conduit_core::*;
use std::alloc::{GlobalAlloc, Layout, System};
#[derive(Clone, Copy)]
struct Counts {
    enabled: bool,
    requested: usize,
    live: usize,
    peak: usize,
    calls: usize,
}
std::thread_local! {static COUNTS:std::cell::Cell<Counts>=const{std::cell::Cell::new(Counts{enabled:false,requested:0,live:0,peak:0,calls:0})};}
fn update(f: impl FnOnce(&mut Counts)) {
    let _ = COUNTS.try_with(|cell| {
        let mut c = cell.get();
        if c.enabled {
            f(&mut c);
            cell.set(c);
        }
    });
}
struct Alloc;
unsafe impl GlobalAlloc for Alloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        charge(l.size());
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        charge(l.size());
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        update(|c| c.live -= l.size());
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        charge(n);
        update(|c| c.live -= l.size());
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static ALLOC: Alloc = Alloc;
fn charge(n: usize) {
    update(|c| {
        c.calls += 1;
        c.requested += n;
        c.live += n;
        c.peak = c.peak.max(c.live);
    });
}
fn measure<T>(f: impl FnOnce() -> T) -> (T, usize, usize, usize) {
    COUNTS.with(|c| {
        c.set(Counts {
            enabled: true,
            requested: 0,
            live: 0,
            peak: 0,
            calls: 0,
        })
    });
    let value = f();
    let counts = COUNTS.with(|c| {
        let mut v = c.get();
        v.enabled = false;
        c.set(v);
        v
    });
    (value, counts.requested, counts.peak, counts.calls)
}
#[test]
fn typed_zip_admits_all_preparation_requests_and_refuses_before_hashing() {
    let leaf = StructuredInfoType::leaf(KindId::new("value/count")).unwrap();
    let nominal = StructuredInfoType::nominal(KindId::new("test/number"), leaf.clone()).unwrap();
    for left_type in [&leaf, &nominal] {
        for right_type in [&leaf, &nominal] {
            let contract = |ty: &StructuredInfoType| {
                CheckedValueContract::new(
                    match ty.shape() {
                        StructuredInfoTypeShape::Leaf(k) => k.clone(),
                        _ => ty.profile().unwrap().value_kind().clone(),
                    },
                    4096,
                    vec![],
                )
                .unwrap()
            };
            let left = contract(left_type);
            let right = contract(right_type);
            let (r, _, _, calls) = measure(|| {
                FlowZipBack::typed_storage_reservation(&left, left_type, &right, right_type)
                    .unwrap()
            });
            assert_eq!(calls, 0);
            let ((back, receipt), requests, peak, _) = measure(|| {
                FlowZipBack::prepare_typed_with_storage_limits(
                    &left,
                    left_type,
                    &right,
                    right_type,
                    r.preparation_requested_bytes_bound,
                    r.retained_heap_bytes_bound,
                )
                .unwrap()
            });
            assert_eq!(r, receipt);
            assert!(requests <= r.preparation_requested_bytes_bound);
            assert!(peak <= r.preparation_requested_bytes_bound);
            assert!(back.typed_owned_heap_bytes().unwrap() <= r.retained_heap_bytes_bound);
            let ordinary =
                FlowZipBack::prepare_typed(&left, left_type.clone(), &right, right_type.clone())
                    .unwrap();
            use conduit_kernel::scheduler::StepBack;
            assert_eq!(
                <FlowZipBack as StepBack<2>>::terminal_transductions(&back),
                <FlowZipBack as StepBack<2>>::terminal_transductions(&ordinary)
            );
            for (prep, retained) in [
                (
                    r.preparation_requested_bytes_bound - 1,
                    r.retained_heap_bytes_bound,
                ),
                (
                    r.preparation_requested_bytes_bound,
                    r.retained_heap_bytes_bound - 1,
                ),
            ] {
                let (result, requested, _, calls) = measure(|| {
                    FlowZipBack::prepare_typed_with_storage_limits(
                        &left, left_type, &right, right_type, prep, retained,
                    )
                });
                assert!(matches!(
                    result,
                    Err(PreparedStructuredCompositionStorageRefusal::Capacity)
                ));
                assert_eq!((requested, calls), (0, 0));
            }
            let mut wrong = left.clone();
            wrong.value_kind = KindId::new("foreign/transport");
            assert!(matches!(
                FlowZipBack::prepare_typed_with_storage_limits(
                    &wrong,
                    left_type,
                    &right,
                    right_type,
                    r.preparation_requested_bytes_bound,
                    r.retained_heap_bytes_bound
                ),
                Err(PreparedStructuredCompositionStorageRefusal::Structured(
                    StructuredInfoRefusal::WrongType
                ))
            ));
        }
    }
}
#[test]
fn bounded_modes_preserve_pair_bytes_and_closure_under_output_pressure() {
    use conduit_kernel::{
        scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
        PortId, ValueRef,
    };
    let ty = StructuredInfoType::leaf(KindId::new("value/count")).unwrap();
    let contract = CheckedValueContract::new(KindId::new("value/count"), 8, vec![]).unwrap();
    let r = FlowZipBack::typed_storage_reservation(&contract, &ty, &contract, &ty).unwrap();
    let left = encode_count(7);
    let right = encode_count(8);
    let mut expected_encoder =
        PreparedTypedTuplePairEncoder::new(ty.clone(), 8, ty.clone(), 8).unwrap();
    let expected = expected_encoder.encode(&left, &right).unwrap().to_vec();
    for mode in 0..3 {
        let (mut back, _) = match mode {
            0 => FlowZipBack::prepare_typed_with_storage_limits(
                &contract,
                &ty,
                &contract,
                &ty,
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound,
            ),
            1 => FlowZipBack::prepare_typed_finite_with_storage_limits(
                &contract,
                &ty,
                &contract,
                &ty,
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound,
            ),
            _ => FlowZipBack::prepare_typed_feedback_with_storage_limits(
                &contract,
                &ty,
                &contract,
                &ty,
                r.preparation_requested_bytes_bound,
                r.retained_heap_bytes_bound,
            ),
        }
        .unwrap();
        let (_, requested, _, calls) = measure(|| {
            for (side, bytes) in [(0, left.as_slice()), (1, right.as_slice())] {
                let mut refs = [None; 2];
                let mut inputs = [None; 2];
                refs[side] = Some(ValueRef {
                    slot: side as u16,
                    generation: 1,
                    byte_len: bytes.len() as u32,
                });
                inputs[side] = Some(bytes);
                let mut io = StepIo::test_frame(refs, [false; 2], [None; 2], None, 16);
                assert_eq!(
                    back.step(&mut io, &StepInputBytes::test_frame(inputs, None)),
                    StepOutcome::Progress
                );
                <FlowZipBack as StepBack<2>>::step_committed(&mut back);
            }
            for _ in 0..1000 {
                let mut io = StepIo::test_frame([None; 2], [mode != 2, false], [None; 2], None, 16);
                assert_eq!(
                    back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                    StepOutcome::Await
                );
                assert!(!io.test_consumed_closed(PortId(0)));
                assert!(<FlowZipBack as StepBack<2>>::prepared_output(&back, PortId(0)).is_none());
            }
            let mut io =
                StepIo::test_frame([None; 2], [mode != 2, false], [Some(4096), None], None, 16);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Progress
            );
            assert_eq!(
                <FlowZipBack as StepBack<2>>::prepared_output(&back, PortId(0)),
                Some(expected.as_slice())
            );
            <FlowZipBack as StepBack<2>>::step_committed(&mut back);
            if mode == 2 {
                let mut io = StepIo::test_frame([None; 2], [false, true], [None; 2], None, 16);
                assert_eq!(
                    back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                    StepOutcome::Progress
                );
                let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
                assert_eq!(
                    back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                    StepOutcome::Await
                );
                let mut io = StepIo::test_frame(
                    [
                        Some(ValueRef {
                            slot: 2,
                            generation: 1,
                            byte_len: 8,
                        }),
                        None,
                    ],
                    [false; 2],
                    [None; 2],
                    None,
                    16,
                );
                assert_eq!(
                    back.step(
                        &mut io,
                        &StepInputBytes::test_frame([Some(&left), None], None)
                    ),
                    StepOutcome::Progress
                );
                <FlowZipBack as StepBack<2>>::step_committed(&mut back);
            }
            let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
                StepOutcome::Complete
            );
        });
        assert_eq!((requested, calls), (0, 0));
    }
}
