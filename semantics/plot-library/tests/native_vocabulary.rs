use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot_library::{LibraryAvailability, LibraryRefusal};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn plot_library_vocabulary_has_native_identity_and_exact_round_trips() {
    round_trip(LibraryAvailability::Available);
    round_trip(LibraryAvailability::needs_capability("camera/input@1".into()).unwrap());
    round_trip(LibraryRefusal::InvalidInventory);
    round_trip(LibraryRefusal::SearchBound);
    round_trip(LibraryRefusal::Presentation);
}

#[test]
fn capability_reason_is_nonempty_and_bounded_by_the_type() {
    assert!(LibraryAvailability::needs_capability(String::new()).is_err());
    assert!(LibraryAvailability::needs_capability("x".repeat(513)).is_err());
}
