use conduit_plot::rust_binding::NativeRustBinding;
use conduit_presentation::{
    ApplicationComponent, ApplicationComponentForm, ApplicationEventKind, ApplicationEventKindForm,
    ApplicationNodeState, ApplicationNodeStateForm, ApplicationViewRefusal,
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
        ApplicationComponentForm::encode(ApplicationComponent::Shell),
        [1]
    );
    assert_eq!(
        ApplicationComponentForm::encode(ApplicationComponent::Separator),
        [47]
    );
    assert_eq!(
        ApplicationComponentForm::decode(&[47]),
        Ok(ApplicationComponent::Separator)
    );
    assert!(ApplicationComponentForm::decode(&[0]).is_err());

    for (kind, tag) in [
        (ApplicationEventKind::Activate, 1),
        (ApplicationEventKind::Change, 2),
        (ApplicationEventKind::Input, 3),
        (ApplicationEventKind::Toggle, 4),
        (ApplicationEventKind::Submit, 5),
    ] {
        assert_eq!(ApplicationEventKindForm::encode(kind), [tag]);
        assert_eq!(ApplicationEventKindForm::decode(&[tag]), Ok(kind));
    }

    for (state, tag) in [
        (ApplicationNodeState::Ready, 1),
        (ApplicationNodeState::Busy, 2),
        (ApplicationNodeState::Unavailable, 3),
    ] {
        assert_eq!(ApplicationNodeStateForm::encode(state), [tag]);
        assert_eq!(ApplicationNodeStateForm::decode(&[tag]), Ok(state));
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
