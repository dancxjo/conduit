use conduit_plot::rust_binding::NativeRustBinding;
use conduit_time::{CivilFoldPolicy, CivilGapPolicy, CivilResolutionChoice, CivilResolutionPolicy};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn civil_resolution_vocabulary_round_trips_through_exact_native_types() {
    for policy in [
        CivilGapPolicy::Skip,
        CivilGapPolicy::UseBefore,
        CivilGapPolicy::UseAfter,
        CivilGapPolicy::Refuse,
    ] {
        assert_round_trip(policy);
    }
    for policy in [
        CivilFoldPolicy::Earlier,
        CivilFoldPolicy::Later,
        CivilFoldPolicy::Both,
        CivilFoldPolicy::Refuse,
    ] {
        assert_round_trip(policy);
    }
    for choice in [
        CivilResolutionChoice::Unique,
        CivilResolutionChoice::GapBefore,
        CivilResolutionChoice::GapAfter,
        CivilResolutionChoice::FoldEarlier,
        CivilResolutionChoice::FoldLater,
    ] {
        assert_round_trip(choice);
    }

    let policy = CivilResolutionPolicy::new(CivilFoldPolicy::Both, CivilGapPolicy::Skip).unwrap();
    let structured = policy.into_structured().unwrap();
    assert_eq!(
        CivilResolutionPolicy::from_structured(structured).unwrap(),
        policy
    );
}
