use conduit_body_invitation_form::InvitationPresentationRefusal;
use conduit_form::rust_binding::NativeRustBinding;

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
