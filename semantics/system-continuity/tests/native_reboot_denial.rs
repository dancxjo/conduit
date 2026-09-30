use conduit_form::rust_binding::NativeRustBinding;
use conduit_system_continuity::RebootDenial;

#[test]
fn reboot_denial_is_native_semantic_info_with_stable_rust_serde() {
    for denial in [
        RebootDenial::MalformedRequest,
        RebootDenial::Unsupported,
        RebootDenial::Unauthorized,
        RebootDenial::StaleTargetBoot,
        RebootDenial::SessionMismatch,
        RebootDenial::Replay,
        RebootDenial::AttemptLimitReached,
        RebootDenial::TransactionPending,
    ] {
        let structured = denial.into_structured().unwrap();
        assert_eq!(RebootDenial::from_structured(structured).unwrap(), denial);
        let encoded = serde_json::to_vec(&denial).unwrap();
        assert_eq!(
            serde_json::from_slice::<RebootDenial>(&encoded).unwrap(),
            denial
        );
    }
}
