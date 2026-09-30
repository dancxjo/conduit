use conduit_alife::{LeniaBoundary, ReactionDiffusionBoundaryEdge};
use conduit_form::rust_binding::NativeRustBinding;

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn artificial_life_boundaries_round_trip_through_exact_native_types() {
    assert_round_trip(LeniaBoundary::Wrap);
    for edge in [
        ReactionDiffusionBoundaryEdge::North,
        ReactionDiffusionBoundaryEdge::South,
        ReactionDiffusionBoundaryEdge::West,
        ReactionDiffusionBoundaryEdge::East,
    ] {
        assert_round_trip(edge);
    }
}
