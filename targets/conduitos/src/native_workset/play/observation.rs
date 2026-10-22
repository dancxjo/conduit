//! Retain the failed boundary before the ordinary fail-closed cleanup.
pub(super) fn refused(stage: &str, detail: &impl core::fmt::Debug) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    {
        use core::fmt::Write;
        struct Output;
        impl core::fmt::Write for Output {
            fn write_str(&mut self, value: &str) -> core::fmt::Result {
                crate::arch::early_write(value.as_bytes());
                Ok(())
            }
        }
        let _ = writeln!(
            Output,
            "CONDUIT_NATIVE_PLAY_REFUSAL stage={stage} detail={detail:?}"
        );
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = (stage, detail);
}
