use conduit_body_invitation_plot::InvitationFaceRefusal;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn invitation_face_refusal_has_native_identity_and_exact_round_trips() {
    for refusal in [
        InvitationFaceRefusal::InvalidInvitation,
        InvitationFaceRefusal::Face,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(
            InvitationFaceRefusal::from_structured(structured).unwrap(),
            refusal
        );
    }
}
