use conduit_plot::rust_binding::NativeRustBinding;
use conduit_robotics::{
    NavigationIdentity64, NavigationTraversability4x4, NavigationTraversabilityCell,
    NavigationTraversabilityCells, NavigationValidity,
};

fn identity(value: &str) -> NavigationIdentity64 {
    NavigationIdentity64::new(value.into()).unwrap()
}

#[test]
fn navigation_identity_preserves_exact_bounds_and_native_round_trip() {
    let identity = NavigationIdentity64::new("x".repeat(64)).unwrap();
    let structured = identity.clone().into_structured().unwrap();
    assert_eq!(
        NavigationIdentity64::from_structured(structured).unwrap(),
        identity
    );
    assert!(NavigationIdentity64::new(String::new()).is_err());
    assert!(NavigationIdentity64::new("x".repeat(65)).is_err());
}

#[test]
fn navigation_grid_enforces_signed_origin_unsigned_extent_invariant() {
    let cells =
        NavigationTraversabilityCells::new([NavigationTraversabilityCell::Free; 16]).unwrap();
    let validity = NavigationValidity::new(10, 20).unwrap();
    let grid = NavigationTraversability4x4::new(
        1,
        1,
        cells.clone(),
        identity("clock"),
        identity("map"),
        i32::MAX - 4,
        i32::MAX - 4,
        1,
        identity("sensor"),
        validity.clone(),
    )
    .unwrap();
    let structured = grid.clone().into_structured().unwrap();
    assert_eq!(
        NavigationTraversability4x4::from_structured(structured).unwrap(),
        grid
    );

    assert!(NavigationTraversability4x4::new(
        1,
        u32::MAX,
        cells,
        identity("clock"),
        identity("map"),
        i32::MAX,
        0,
        1,
        identity("sensor"),
        validity,
    )
    .is_err());
}
