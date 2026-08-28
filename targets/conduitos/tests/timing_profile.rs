use conduitos::{
    identity::BootIdentities,
    offer::{CpuFeatures, HostOffer},
    timing_profile::{
        Injection, PROOF_CLASS, Refusal, TimingOffer, TimingOutcome, TimingRequirement, admit,
        execute,
    },
};

mod allocation_probe {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };

    pub struct TrackingAllocator;

    thread_local! {
        static TRACKING: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    fn record() {
        let _ = TRACKING.try_with(|tracking| {
            if tracking.get() {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get().saturating_add(1)));
            }
        });
    }

    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) };
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let pointer = unsafe { System.realloc(pointer, layout, size) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }
    }

    #[global_allocator]
    static ALLOCATOR: TrackingAllocator = TrackingAllocator;

    pub fn measure(action: impl FnOnce()) -> usize {
        ALLOCATIONS.with(|count| count.set(0));
        TRACKING.with(|tracking| tracking.set(true));
        action();
        TRACKING.with(|tracking| tracking.set(false));
        ALLOCATIONS.with(Cell::get)
    }
}

fn fixture() -> (BootIdentities, HostOffer<'static>, TimingOffer) {
    let identities = BootIdentities {
        host: [7; 32],
        boot: [8; 32],
    };
    let host = HostOffer::new(
        &identities,
        "timing-build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    let timing = TimingOffer::deterministic(&host, 42);
    (identities, host, timing)
}

fn admitted(deadline_us: u32) -> (conduitos::timing_profile::AdmittedTimingPlan, TimingOffer) {
    let (identities, host, timing) = fixture();
    let plan = admit(
        &identities,
        &host,
        timing,
        TimingRequirement { deadline_us },
        "timing-build",
    )
    .unwrap();
    (plan, timing)
}

#[test]
fn form_requirement_is_platform_neutral_and_exact_plan_seals_every_cost() {
    let (plan, _) = admitted(1_000);
    assert_eq!(plan.basis.proof_class, PROOF_CLASS);
    assert!(plan.basis.proven_worst_case_us <= plan.basis.deadline_us);
    assert!(plan.basis.arena_bytes > 0);
    assert_eq!((plan.basis.cord_items, plan.basis.cord_bytes), (1, 8));
    assert_eq!((plan.basis.wake_slots, plan.basis.timer_slots), (1, 1));
    assert!(plan.basis.base_scratch_bytes > 0);
    assert!(plan.basis.mandatory_sign_items > 0);
    assert!(plan.basis.mandatory_sign_bytes > 0);
    assert!(plan.basis.fault_reserve_bytes > 0);
    assert!(!plan.basis.inspection_included);
    let source = conduitos::ordinary_plan::ORDINARY_FORM_SOURCE;
    for forbidden in [
        "QEMU",
        "x86",
        "Linux",
        "timer device",
        "scheduler quantum",
        "clock implementation",
    ] {
        assert!(!source.contains(forbidden));
    }
}

#[test]
fn unschedulable_request_is_refused_before_any_play_exists() {
    let (identities, host, timing) = fixture();
    assert!(matches!(
        admit(
            &identities,
            &host,
            timing,
            TimingRequirement { deadline_us: 100 },
            "timing-build"
        ),
        Err(Refusal::Unschedulable {
            required_us: 730,
            deadline_us: 100
        })
    ));
}

#[test]
fn strict_kernel_path_reports_met_missed_loss_cancel_and_stale_distinctly() {
    let (mut plan, timing) = admitted(1_000);
    let sign = execute(&mut plan, timing, Injection::None);
    assert!(!sign.plan_id.is_empty() && !sign.active_play_id.is_empty());
    assert!(matches!(sign.outcome, TimingOutcome::DeadlineMet { .. }));

    let (mut plan, timing) = admitted(1_000);
    assert_eq!(
        execute(&mut plan, timing, Injection::Overrun).outcome,
        TimingOutcome::DeadlineMiss {
            elapsed_us: 1_001,
            deadline_us: 1_000
        }
    );

    let (mut plan, timing) = admitted(1_000);
    assert_eq!(
        execute(&mut plan, timing, Injection::TimerBaseLoss).outcome,
        TimingOutcome::TimerBaseLoss
    );

    let (mut plan, timing) = admitted(1_000);
    assert_eq!(
        execute(&mut plan, timing, Injection::Cancel).outcome,
        TimingOutcome::Cancelled
    );

    let (mut plan, mut timing) = admitted(1_000);
    timing.maximum_wake_latency_us += 1;
    assert_eq!(
        execute(&mut plan, timing, Injection::None).outcome,
        TimingOutcome::StaleTimingBasis
    );
}

#[test]
fn missing_finite_reserves_refuse_during_planning() {
    let (identities, host, mut timing) = fixture();
    timing.fault_reserve_bytes = 0;
    assert!(matches!(
        admit(
            &identities,
            &host,
            timing,
            TimingRequirement { deadline_us: 1_000 },
            "timing-build"
        ),
        Err(Refusal::ResourceCapacity)
    ));
}

#[test]
fn admitted_strict_play_has_zero_successful_heap_allocations() {
    let (mut plan, timing) = admitted(1_000);
    let allocations = allocation_probe::measure(|| {
        let sign = execute(&mut plan, timing, Injection::None);
        assert!(matches!(sign.outcome, TimingOutcome::DeadlineMet { .. }));
    });
    assert_eq!(allocations, 0);
}
