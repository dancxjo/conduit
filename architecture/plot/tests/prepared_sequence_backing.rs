//! Ownership and allocator boundaries for future generated sequence staging.
use conduit_plot::rust_binding::BoundedSequence;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn quiet(o: allocation_probe::Observation) {
    assert_eq!(o.allocations, 0);
    assert_eq!(o.reallocations, 0);
    assert_eq!(o.deallocations, 0);
}

#[test]
fn adoption_and_physical_pressure_retain_all_backing_and_pending_ownership() {
    let values = vec![String::from("active")];
    let pointer = values.as_ptr();
    let (adopted, counts) = allocation_probe::observe(|| {
        BoundedSequence::<String, 4>::try_from_preallocated_vec(values)
    });
    quiet(counts);
    let mut sequence = adopted.unwrap();
    assert_eq!(sequence.as_slice().as_ptr(), pointer);
    assert_eq!(sequence.capacity(), 4);
    assert_eq!(sequence.physical_capacity(), 1);
    let pending = String::from("pending");
    let pending_pointer = pending.as_ptr();
    let (refused, counts) = allocation_probe::observe(|| sequence.push_preallocated(pending));
    quiet(counts);
    let returned = refused.unwrap_err();
    assert_eq!(returned, "pending");
    assert_eq!(returned.as_ptr(), pending_pointer);
    assert_eq!(sequence.as_slice(), &["active"]);
    assert_eq!(sequence.as_slice().as_ptr(), pointer);
}

#[test]
fn excessive_lengths_and_physical_profiles_return_the_original_vec() {
    for length in [1, 3] {
        let mut values = Vec::with_capacity(3);
        for index in 0..length {
            values.push(index);
        }
        let pointer = values.as_ptr();
        let capacity = values.capacity();
        let (result, counts) = allocation_probe::observe(|| {
            BoundedSequence::<usize, 2>::try_from_preallocated_vec(values)
        });
        quiet(counts);
        let returned = result.unwrap_err();
        assert_eq!(returned.as_ptr(), pointer);
        assert_eq!(returned.capacity(), capacity);
        assert_eq!(returned.len(), length);
    }
}

#[test]
fn checked_push_preserves_semantic_maximum_even_with_legacy_spare_capacity() {
    let mut sequence = BoundedSequence::<u8, 3>::new();
    sequence.push(1).unwrap(); // Existing allocating convenience is preparation only.
    let capacity = sequence.physical_capacity();
    assert!(capacity >= 3);
    let ((second, third, excess), counts) = allocation_probe::observe(|| {
        (
            sequence.push_preallocated(2),
            sequence.push_preallocated(3),
            sequence.push_preallocated(4),
        )
    });
    quiet(counts);
    assert_eq!((second, third, excess), (Ok(()), Ok(()), Err(4)));
    assert_eq!(sequence.as_slice(), &[1, 2, 3]);
    assert_eq!(sequence.capacity(), 3);
    assert_eq!(sequence.physical_capacity(), capacity);
}

#[test]
fn zero_capacity_and_zero_sized_elements_keep_distinct_physical_and_semantic_limits() {
    let mut empty = BoundedSequence::<u8, 3>::try_from_preallocated_vec(Vec::new()).unwrap();
    let (result, counts) = allocation_probe::observe(|| empty.push_preallocated(7));
    quiet(counts);
    assert_eq!(result, Err(7));
    let (result, counts) = allocation_probe::observe(|| {
        let mut units = BoundedSequence::<(), 2>::try_from_preallocated_vec(Vec::new()).unwrap();
        let results = (
            units.push_preallocated(()),
            units.push_preallocated(()),
            units.push_preallocated(()),
        );
        (units, results)
    });
    quiet(counts);
    assert_eq!(result.0.len(), 2);
    assert_eq!(result.0.capacity(), 2);
    assert_eq!(result.1, (Ok(()), Ok(()), Err(())));
}
