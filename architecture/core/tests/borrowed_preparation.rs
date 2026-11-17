//! Borrowed preparation retains the original verifier and Fore semantics.
use conduit_core::*;
#[path = "common/sealed_state.rs"]
mod fixture;

#[test]
fn duplicate_default_and_explicit_last_contract_match_checked_front() {
    let mut gear = fixture::fragment().placements.remove(0);
    gear.inputs.clear();
    gear.outputs = ["value/text", "value/bytes"]
        .into_iter()
        .map(|kind| PortDescriptor {
            port_id: port_id("same"),
            value_kind: kind_id(kind),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: Some(kind_id(kind)),
        })
        .collect();
    for explicit in [false, true] {
        if explicit {
            gear.semantic_contract = KindSemanticContract {
                configuration: vec![],
                laws: vec![KindSemanticLaw::ValueContracts(vec![
                    FrontValueContract {
                        location: FrontValueLocation::Output(port_id("same")),
                        contract: CheckedValueContract::new(kind_id("value/text"), 100, vec![])
                            .unwrap(),
                    },
                    FrontValueContract {
                        location: FrontValueLocation::Output(port_id("same")),
                        contract: CheckedValueContract::new(kind_id("value/text"), 200, vec![])
                            .unwrap(),
                    },
                ])],
            };
        }
        for abnormal in [false, true] {
            let location = if abnormal {
                FrontValueLocation::OutputAbnormal(port_id("same"))
            } else {
                FrontValueLocation::Output(port_id("same"))
            };
            let old = gear
                .checked_port_front()
                .value_contract(&location)
                .map(|contract| u64::from(contract.maximum_bytes));
            assert_eq!(
                gear.port_value_maximum_bytes(PortDirection::Output, &port_id("same"), abnormal),
                old
            );
        }
    }
}

#[test]
fn no_shared_pool_does_not_make_orphan_pool_references_valid() {
    let mut fragment = fixture::fragment();
    assert!(fragment.shared_pools.is_empty());
    assert!(verify_plan(&fixture::seal(fragment.clone())));
    fragment.placements[0]
        .pool_references
        .push(SharedPoolId::from("orphan"));
    let plan = fixture::seal(fragment);
    assert!(!verify_plan(&plan));
}

mod allocation {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };
    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
        static LIVE: Cell<usize> = const { Cell::new(0) };
        static PEAK: Cell<usize> = const { Cell::new(0) };
    }
    struct Allocator;
    fn add(bytes: usize) {
        if ACTIVE.try_with(Cell::get).unwrap_or(false) {
            LIVE.with(|live| {
                let next = live.get() + bytes;
                live.set(next);
                PEAK.with(|peak| peak.set(peak.get().max(next)));
            });
        }
    }
    fn remove(bytes: usize) {
        if ACTIVE.try_with(Cell::get).unwrap_or(false) {
            LIVE.with(|live| live.set(live.get() - bytes));
        }
    }
    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                add(layout.size());
            }
            pointer
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                add(layout.size());
            }
            pointer
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            remove(layout.size());
            unsafe {
                System.dealloc(pointer, layout);
            }
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, bytes: usize) -> *mut u8 {
            let next = unsafe { System.realloc(pointer, layout, bytes) };
            if !next.is_null() {
                remove(layout.size());
                add(bytes);
            }
            next
        }
    }
    #[global_allocator]
    static ALLOCATOR: Allocator = Allocator;
    pub fn measure(work: impl FnOnce() -> bool) -> (bool, usize) {
        LIVE.with(|live| live.set(0));
        PEAK.with(|peak| peak.set(0));
        ACTIVE.with(|active| active.set(true));
        let valid = work();
        ACTIVE.with(|active| active.set(false));
        (valid, PEAK.with(Cell::get))
    }
}

#[test]
fn borrowed_verifier_descriptor_covers_actual_multi_fragment_fore_scratch() {
    let mut fragments = Vec::new();
    for index in 0..2 {
        let mut part = fixture::fragment();
        part.states.clear();
        part.sign_storage_budget = mandatory_sign_storage_requirement(&part.expected_sign).unwrap();
        part.host_id = format!("host/{index}").into();
        part.placements[0].host_id = part.host_id.clone();
        part.fore_ports.push(PlannedForePort {
            front_port_id: port_id("result"),
            direction: PortDirection::Output,
            placement_id: part.placements[0].placement_id.clone(),
            gear_port_id: port_id("current"),
            value_kind: part.placements[0].outputs[0].value_kind.clone(),
            value_contract: None,
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
            selected_line: None,
        });
        fragments.push(part);
    }
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: "source".into(),
            checked_plot_id: "checked".into(),
            expanded_plot_id: "expanded".into(),
        },
        fragments,
    );
    let expected = local_plan_verification_backing_bytes(&plan).unwrap();
    let (valid, peak) = allocation::measure(|| verify_plan(&plan));
    assert!(valid);
    assert!(peak > 0);
    assert!(peak <= expected, "{peak} > {expected}");
}
