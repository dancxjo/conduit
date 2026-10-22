//! Retain the failed boundary before the ordinary fail-closed cleanup.
pub(super) fn refused(stage: &str, detail: &impl core::fmt::Debug) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    {
        use core::fmt::Write;
        let _ = writeln!(
            crate::arch::Serial::new(),
            "CONDUIT_NATIVE_PLAY_REFUSAL stage={stage} detail={detail:?}"
        );
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = (stage, detail);
}
