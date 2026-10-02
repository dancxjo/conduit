use conduit_body_invitation_plot::InvitationPresentationRefusal;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn invitation_presentation_refusal_has_native_identity_and_exact_round_trips() {
    for refusal in [
        InvitationPresentationRefusal::InvalidInvitation,
        InvitationPresentationRefusal::Presentation,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(
            InvitationPresentationRefusal::from_structured(structured).unwrap(),
            refusal
        );
    }
}
