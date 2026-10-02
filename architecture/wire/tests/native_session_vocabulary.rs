use conduit_plot::rust_binding::NativeRustBinding;
use conduit_wire::{
    SessionRole, SessionTerminalDisposition, SessionTerminalDispositionForm,
    SessionTransferCheckpoint,
};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn session_roles_terminals_and_transfer_checkpoints_are_native() {
    round_trip(SessionRole::Source);
    round_trip(SessionRole::Sink);

    for (terminal, tag) in [
        (SessionTerminalDisposition::Completed, 0),
        (SessionTerminalDisposition::Cancelled, 1),
        (SessionTerminalDisposition::Failed, 2),
    ] {
        round_trip(terminal);
        assert_eq!(SessionTerminalDispositionForm::encode(terminal), [tag]);
        assert_eq!(SessionTerminalDispositionForm::decode(&[tag]), Ok(terminal));
    }
    assert!(SessionTerminalDispositionForm::decode(&[3]).is_err());

    for checkpoint in [
        SessionTransferCheckpoint::None,
        SessionTransferCheckpoint::offered(0).unwrap(),
        SessionTransferCheckpoint::accepted(u64::MAX).unwrap(),
    ] {
        round_trip(checkpoint);
    }
}
