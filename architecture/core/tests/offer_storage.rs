use conduit_core::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static REQUESTS: Cell<usize> = const { Cell::new(0) };
}
struct Probe;
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ENABLED.with(|enabled| {
            if enabled.get() {
                REQUESTS.with(|n| n.set(n.get() + layout.size()));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ENABLED.with(|enabled| {
            if enabled.get() {
                REQUESTS.with(|n| n.set(n.get() + size));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;

#[test]
fn original_profile_shape_validates_and_moves_offer_without_allocating() {
    let kind = Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("test/profile"),
        kind_contract_revision: KindIdentity::from("v1"),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            direction: PortDirection::Input,
            value_kind: kind_id("value/count"),
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: vec![],
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: FrontValueLocation::Input(port_id("input")),
            contract: CheckedValueContract::new(kind_id("value/count"), 8, vec![]).unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
    };
    let clone_reservation = kind_clone_storage_reservation(&kind).unwrap();
    REQUESTS.with(|n| n.set(0));
    ENABLED.with(|n| n.set(true));
    let cloned = kind.clone();
    ENABLED.with(|n| n.set(false));
    assert!(REQUESTS.with(Cell::get) <= clone_reservation.preparation_requested_bytes_bound);
    assert!(kind_owned_heap_bytes(&cloned).unwrap() <= clone_reservation.retained_heap_bytes_bound);
    drop(cloned);
    let original_laws = kind.semantic_laws.as_ptr();
    let back = Back {
        capability_id: CapabilityId::from("cap"),
        execution_profile_id: ExecutionProfileId::from("native"),
        implementation_id: ImplementationId::from("impl"),
        artifact_id: ArtifactId::from("artifact"),
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
    };
    REQUESTS.with(|n| n.set(0));
    ENABLED.with(|n| n.set(true));
    let result = BackOfferBuilder::try_new(kind, back).map(BackOfferBuilder::build);
    ENABLED.with(|n| n.set(false));
    let offer = result.unwrap();
    assert_eq!(REQUESTS.with(Cell::get), 0);
    assert_eq!(offer.semantic_contract.laws.as_ptr(), original_laws);
    assert!(capability_offer_owned_heap_bytes(&offer).unwrap() > 0);
}
