use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{
    ApplicationComponent, ApplicationComponentCode, ApplicationEventKind, ApplicationEventKindCode,
    ApplicationNodeState, ApplicationNodeStateCode, ApplicationViewRefusal,
};

fn assert_round_trip<T>(value: T)
where
    T: NativeRustBinding + Copy + core::fmt::Debug + PartialEq,
{
    let structured = value.into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn application_view_wire_tags_belong_to_native_types() {
    assert_eq!(
        ApplicationComponentCode::encode(ApplicationComponent::Shell),
        [1]
    );
    assert_eq!(
        ApplicationComponentCode::encode(ApplicationComponent::Separator),
        [47]
    );
    assert_eq!(
        ApplicationComponentCode::decode(&[47]),
        Ok(ApplicationComponent::Separator)
    );
    assert!(ApplicationComponentCode::decode(&[0]).is_err());

    for (kind, tag) in [
        (ApplicationEventKind::Activate, 1),
        (ApplicationEventKind::Change, 2),
        (ApplicationEventKind::Input, 3),
        (ApplicationEventKind::Toggle, 4),
        (ApplicationEventKind::Submit, 5),
    ] {
        assert_eq!(ApplicationEventKindCode::encode(kind), [tag]);
        assert_eq!(ApplicationEventKindCode::decode(&[tag]), Ok(kind));
    }

    for (state, tag) in [
        (ApplicationNodeState::Ready, 1),
        (ApplicationNodeState::Busy, 2),
        (ApplicationNodeState::Unavailable, 3),
    ] {
        assert_eq!(ApplicationNodeStateCode::encode(state), [tag]);
        assert_eq!(ApplicationNodeStateCode::decode(&[tag]), Ok(state));
    }
}

#[test]
fn application_view_refusals_are_native_values() {
    for refusal in [
        ApplicationViewRefusal::Empty,
        ApplicationViewRefusal::InvalidControlValue,
        ApplicationViewRefusal::MalformedEncoding,
        ApplicationViewRefusal::QueuePressure,
    ] {
        assert_round_trip(refusal);
    }
}
