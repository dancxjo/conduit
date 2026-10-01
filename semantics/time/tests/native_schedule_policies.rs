use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::{ClockChangeBehavior, SuspendBehavior};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn schedule_policies_round_trip_through_their_exact_native_types() {
    for value in [
        SuspendBehavior::ClockIncludesSuspend,
        SuspendBehavior::ClockExcludesSuspend,
        SuspendBehavior::RefuseAfterSuspend,
    ] {
        assert_round_trip(value);
    }
    for value in [
        ClockChangeBehavior::ReevaluateWindow,
        ClockChangeBehavior::RefuseAfterChange,
    ] {
        assert_round_trip(value);
    }
}
