//! Ordinal callbacks construct directly into guarded owner slots, never arrays.
use super::tests::{empty, identity, now, zero};
use super::*;
use crate::member_return_tests::return_allocation_probe as allocation;
use conduit_environment_runtime::{
    publication::PublicationSequence, publication_face_custody::Publisher,
};
use core::cell::{Cell, RefCell};

#[test]
fn ordinal_callbacks_run_once_per_index_without_staging_allocations() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let definitions: [Cell<usize>; 3] = core::array::from_fn(|_| Cell::new(0));
    let identities: [Cell<usize>; 3] = core::array::from_fn(|_| Cell::new(0));
    let mut inventory = SoftwareInventory::<3>::empty();
    // SAFETY: the test's independently supplied pin/constructor pairs select
    // only allocation-free Trigger owners; no execution authority is asserted.
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_by(
            |index| {
                definitions[index].set(definitions[index].get() + 1);
                Ok((index != 1).then_some(SoftwareDefinition::Trigger))
            },
            |index| {
                identities[index].set(identities[index].get() + 1);
                Ok((index != 1).then(|| identity(index as u16)))
            },
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    });
    assert_eq!(result, Ok(()));
    assert!(definitions
        .iter()
        .chain(&identities)
        .all(|count| count.get() == 1));
    assert_eq!(cost.allocations, 0);
    assert_eq!(
        inventory
            .identities()
            .map(|pin| pin.node)
            .collect::<std::vec::Vec<_>>(),
        [0, 2]
    );
}

#[test]
fn zero_length_inventory_invokes_neither_accessor() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let mut inventory = SoftwareInventory::<0>::empty();
    let result = unsafe {
        inventory.initialize_by(
            |_| panic!("no definition exists"),
            |_| panic!("no identity exists"),
            &publisher,
            PublicationSequence::prepare(1),
            now,
            zero,
            &[],
            None,
        )
    };
    assert_eq!(result, Ok(()));
    empty(&inventory);
}

#[test]
fn late_definition_accessor_failure_drops_previous_owner_and_stops_callbacks() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let definitions = Cell::new(0);
    let identities = Cell::new(0);
    let mut inventory = SoftwareInventory::<3>::empty();
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_by(
            |index| {
                definitions.set(definitions.get() + 1);
                match index {
                    0 => Ok(Some(SoftwareDefinition::Dht11Observation)),
                    1 => Err("definition-refused"),
                    _ => panic!("stop after refusal"),
                }
            },
            |index| {
                identities.set(identities.get() + 1);
                assert_eq!(index, 0);
                Ok(Some(identity(0)))
            },
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    });
    assert_eq!(result, Err("definition-refused"));
    assert_eq!((definitions.get(), identities.get()), (2, 1));
    assert!(cost.allocations > 0);
    assert_eq!(cost.allocations, cost.deallocations);
    empty(&inventory);
}

#[test]
fn late_identity_accessor_failure_clears_already_stored_identity_and_backs() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let definitions = Cell::new(0);
    let identities = Cell::new(0);
    let mut inventory = SoftwareInventory::<3>::empty();
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_by(
            |index| {
                definitions.set(definitions.get() + 1);
                match index {
                    0 => Ok(Some(SoftwareDefinition::Dht11Observation)),
                    1 => Ok(Some(SoftwareDefinition::Trigger)),
                    _ => panic!("stop after refusal"),
                }
            },
            |index| {
                identities.set(identities.get() + 1);
                match index {
                    0 => Ok(Some(identity(0))),
                    1 => Err("identity-refused"),
                    _ => panic!("stop after refusal"),
                }
            },
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    });
    assert_eq!(result, Err("identity-refused"));
    assert_eq!((definitions.get(), identities.get()), (2, 2));
    assert!(cost.allocations > 0);
    assert_eq!(cost.allocations, cost.deallocations);
    empty(&inventory);
}

#[test]
fn late_second_recorder_never_publishes_two_stream_owners() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let definitions = Cell::new(0);
    let identities = Cell::new(0);
    let mut inventory = SoftwareInventory::<4>::empty();
    let (result, cost) = allocation::observe(|| unsafe {
        inventory.initialize_by(
            |index| {
                definitions.set(definitions.get() + 1);
                match index {
                    0 | 2 => Ok(Some(SoftwareDefinition::RecordingLog {
                        maximum_samples: 1,
                    })),
                    1 => Ok(Some(SoftwareDefinition::Trigger)),
                    _ => panic!("stop after second recorder"),
                }
            },
            |index| {
                identities.set(identities.get() + 1);
                assert!(index < 2);
                Ok(Some(identity(index as u16)))
            },
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    });
    assert_eq!(result, Err("recording-scope-multiple-owners"));
    assert_eq!((definitions.get(), identities.get()), (3, 2));
    assert!(cost.allocations > 0);
    assert_eq!(cost.allocations, cost.deallocations);
    empty(&inventory);
}

#[test]
fn occupied_destination_invokes_no_accessors_and_keeps_original_owner() {
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let sequence = PublicationSequence::prepare(1);
    let mut inventory = SoftwareInventory::<1>::empty();
    unsafe {
        inventory.initialize_by(
            |_| Ok(Some(SoftwareDefinition::Trigger)),
            |_| Ok(Some(identity(0))),
            &publisher,
            sequence.clone(),
            now,
            zero,
            &[],
            None,
        )
    }
    .unwrap();
    let result = unsafe {
        inventory.initialize_by(
            |_| panic!("occupied definition accessor"),
            |_| panic!("occupied identity accessor"),
            &publisher,
            sequence,
            now,
            || panic!("occupied allocator callback"),
            &[],
            None,
        )
    };
    assert_eq!(result, Err("software-inventory-occupied"));
    assert!(matches!(inventory.slots[0], Some(SoftwareBack::Trigger(_))));
    assert_eq!(inventory.identities[0], Some(identity(0)));
}
