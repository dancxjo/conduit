//! CPU construction/rollback fixtures, not checked Source or execution authority.
use super::*;
use crate::member_return_tests::return_allocation_probe as allocation;
use conduit_environment_runtime::{
    publication::PublicationSequence, publication_face_custody::Publisher,
};
use core::cell::{Cell, RefCell};

pub(super) fn zero() -> usize {
    0
}
pub(super) fn now() -> u64 {
    1
}
fn never() -> usize {
    panic!("refusal must precede allocator measurement")
}
pub(super) fn identity(node: u16) -> OtherNodeIdentity {
    OtherNodeIdentity {
        node,
        kind: conduit_core::AssignedIdentity::from_text("fixture/software"),
        implementation: conduit_core::AssignedIdentity::from_text("fixture/implementation"),
        artifact: conduit_core::AssignedIdentity::from_text("fixture/artifact"),
    }
}
fn wait() -> conduit_kernel::ValueRef {
    conduit_kernel::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    }
}
pub(super) fn empty<const N: usize>(inventory: &SoftwareInventory<'_, N>) {
    assert!(inventory.slots.iter().all(Option::is_none));
    assert!(inventory.identities().next().is_none());
    assert_eq!(inventory.backing, 0);
    assert!(inventory.configuration.is_empty());
}

#[test]
fn in_place_and_compatibility_constructors_agree_without_inventory_allocations() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(4);
    let mut placed = SoftwareInventory::<3>::empty();
    // SAFETY: this CPU fixture supplies the same independent definitions/pins
    // to both paths. Both Trigger owners allocate nothing during measured construction.
    let (result, cost) = allocation::observe(|| unsafe {
        placed.initialize_into(
            [
                Some(SoftwareDefinition::Trigger),
                None,
                Some(SoftwareDefinition::Trigger),
            ],
            [Some(identity(0)), None, Some(identity(2))],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    });
    assert_eq!(result, Ok(()));
    assert_eq!(
        cost.allocations, 0,
        "in-place transfer must not allocate an inventory"
    );
    let returned = unsafe {
        SoftwareInventory::initialize(
            [
                Some(SoftwareDefinition::Trigger),
                None,
                Some(SoftwareDefinition::Trigger),
            ],
            [Some(identity(0)), None, Some(identity(2))],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
        )
    }
    .unwrap();
    let scoped = unsafe {
        SoftwareInventory::initialize_with_recording_scope(
            [
                Some(SoftwareDefinition::Trigger),
                None,
                Some(SoftwareDefinition::Trigger),
            ],
            [Some(identity(0)), None, Some(identity(2))],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    }
    .unwrap();
    assert_eq!(placed.identities, returned.identities);
    assert_eq!(placed.identities, scoped.identities);
    assert_eq!(placed.backing, returned.backing);
    assert!(matches!(placed.slots[0], Some(SoftwareBack::Trigger(_))));
    assert!(matches!(placed.slots[2], Some(SoftwareBack::Trigger(_))));
}

#[test]
fn occupied_destination_refuses_before_touching_owners_or_measurement() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let mut inventory = SoftwareInventory::<1>::empty();
    unsafe {
        inventory.initialize_into(
            [Some(SoftwareDefinition::Trigger)],
            [Some(identity(0))],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    }
    .unwrap();
    let original = inventory.identities;
    let original_slot = inventory.slots.as_ptr();
    let result = unsafe {
        inventory.initialize_into(
            [Some(SoftwareDefinition::Tick {
                wait: wait(),
                count: u64::MAX,
            })],
            [None],
            &publisher,
            sequence.clone(),
            now,
            never,
            &[],
            None,
        )
    };
    assert_eq!(result, Err("software-inventory-occupied"));
    assert_eq!(inventory.identities, original);
    assert_eq!(inventory.slots.as_ptr(), original_slot);
    assert!(matches!(inventory.slots[0], Some(SoftwareBack::Trigger(_))));
}

#[test]
fn late_constructor_refusal_drops_allocated_backs_and_allows_retry_in_same_slot() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(2);
    let mut inventory = SoftwareInventory::<2>::empty();
    // The malformed second constructor runs after the allocated observation
    // owner. No measured backing/identity may survive this late refusal.
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_into(
            [
                Some(SoftwareDefinition::Dht11Observation),
                Some(SoftwareDefinition::Tick {
                    wait: wait(),
                    count: u64::MAX,
                }),
            ],
            [Some(identity(0)), Some(identity(1))],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    });
    assert_eq!(result, Err("tick-count"));
    assert!(cost.allocations > 0);
    assert_eq!(
        cost.allocations, cost.deallocations,
        "every partial allocation was released"
    );
    empty(&inventory);
    unsafe {
        inventory.initialize_into(
            [Some(SoftwareDefinition::Trigger), None],
            [Some(identity(0)), None],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    }
    .unwrap();
    assert_eq!(inventory.identities().count(), 1);
}

#[test]
fn late_identity_refusal_clears_all_owners_without_publishing_metadata() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(2);
    let mut inventory = SoftwareInventory::<2>::empty();
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_into(
            [
                Some(SoftwareDefinition::Dht11Observation),
                Some(SoftwareDefinition::Trigger),
            ],
            [Some(identity(0)), Some(identity(9))],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    });
    assert_eq!(result, Err("software-inventory"));
    assert!(cost.allocations > 0);
    assert_eq!(cost.allocations, cost.deallocations);
    empty(&inventory);
}

std::thread_local! {static READS:Cell<usize>=const {Cell::new(0)};}
fn declining_measurement() -> usize {
    READS.with(|reads| {
        let n = reads.get();
        reads.set(n + 1);
        100 - n
    })
}
#[test]
fn allocator_measurement_refusal_drops_constructed_owners() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let mut inventory = SoftwareInventory::<1>::empty();
    READS.with(|reads| reads.set(0));
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_into(
            [Some(SoftwareDefinition::Dht11Observation)],
            [Some(identity(0))],
            &publisher,
            sequence.clone(),
            now,
            declining_measurement,
            &[],
            None,
        )
    });
    assert_eq!(result, Err("allocator-measurement"));
    assert_eq!(READS.with(Cell::get), 2);
    assert!(cost.allocations > 0);
    assert_eq!(cost.allocations, cost.deallocations);
    empty(&inventory);
}

#[test]
fn multiple_recording_owners_still_refuse_before_construction() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let mut inventory = SoftwareInventory::<2>::empty();
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_into(
            [
                Some(SoftwareDefinition::RecordingLog { maximum_samples: 1 }),
                Some(SoftwareDefinition::RecordingLog { maximum_samples: 1 }),
            ],
            [Some(identity(0)), Some(identity(1))],
            &publisher,
            sequence.clone(),
            now,
            never,
            &[],
            None,
        )
    });
    assert_eq!(result, Err("recording-scope-multiple-owners"));
    assert_eq!(cost.allocations, 0);
    empty(&inventory);
}

#[test]
fn invalid_original_recording_scope_cannot_publish_an_unbound_partial_owner() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let host = "host/fixture".into();
    let boot = "boot/fixture".into();
    let plan = "plan/fixture".into();
    let play = "play/fixture".into();
    let scope = crate::execution_software::RecordingLogScope {
        context: conduit_environment_runtime::recording::LogContext {
            body: "body/fixture",
            producer: conduit_environment_runtime::recording::RecordingContext {
                host: &host,
                boot: &boot,
                plan: &plan,
                play: &play,
            },
            stream: [1; 32],
        },
        first_sequence: 0,
    };
    let mut inventory = SoftwareInventory::<2>::empty();
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_into(
            [
                Some(SoftwareDefinition::Trigger),
                Some(SoftwareDefinition::RecordingLog { maximum_samples: 1 }),
            ],
            [Some(identity(0)), Some(identity(1))],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            Some(scope),
        )
    });
    assert_eq!(result, Err("recording-scope"));
    assert!(cost.allocations > 0);
    assert_eq!(cost.allocations, cost.deallocations);
    empty(&inventory);
}

#[test]
fn malformed_composite_image_refuses_after_prior_owner_and_restores_empty() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let mut inventory = SoftwareInventory::<2>::empty();
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_into(
            [
                Some(SoftwareDefinition::Dht11Observation),
                Some(SoftwareDefinition::MergeStatic {
                    schema: b"invalid",
                    maximum_bytes: 1,
                }),
            ],
            [Some(identity(0)), Some(identity(1))],
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    });
    assert_eq!(result, Err("composite-type-image"));
    assert!(cost.allocations > 0);
    assert_eq!(cost.allocations, cost.deallocations);
    empty(&inventory);
}
