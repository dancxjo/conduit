//! Read-only gate before consuming actual native executor custody. Passing this
//! gate never replaces the retained execution/provider retirement preflight.
pub fn retirement_ready(
    phase: &str,
    task_active: bool,
    progress_current: bool,
    failure: bool,
    cleanup_failure: bool,
    native_lifecycles: Option<[&str; 2]>,
    native_in_flight: Option<[u16; 2]>,
) -> bool {
    matches!(phase, "quiesced" | "drained")
        && !task_active
        && progress_current
        && !failure
        && !cleanup_failure
        && native_lifecycles == Some(["revoked", "revoked"])
        && native_in_flight == Some([0, 0])
}
#[cfg(test)]
mod tests {
    use super::retirement_ready;
    #[test]
    fn finite_completion_waits_for_actual_cleanup_and_executor_return() {
        // The small finite run has consumed its operations. A status label alone
        // cannot transfer its providers while the future or calls still own them.
        assert!(!retirement_ready(
            "drained",
            true,
            true,
            false,
            false,
            Some(["revoked", "revoked"]),
            Some([0, 0])
        ));
        assert!(!retirement_ready(
            "drained",
            false,
            true,
            false,
            false,
            Some(["issued", "revoked"]),
            Some([0, 0])
        ));
        assert!(!retirement_ready(
            "drained",
            false,
            true,
            false,
            false,
            Some(["revoked", "revoked"]),
            Some([0, 1])
        ));
        assert!(retirement_ready(
            "drained",
            false,
            true,
            false,
            false,
            Some(["revoked", "revoked"]),
            Some([0, 0])
        ));
        assert!(retirement_ready(
            "quiesced",
            false,
            true,
            false,
            false,
            Some(["revoked", "revoked"]),
            Some([0, 0])
        ));
    }
    #[test]
    fn error_or_stale_status_never_masquerades_as_successful_retirement() {
        for phase in [
            "running",
            "failed",
            "cleanup-refused",
            "retired",
            "ready-held",
        ] {
            assert!(!retirement_ready(
                phase,
                false,
                true,
                false,
                false,
                Some(["revoked", "revoked"]),
                Some([0, 0])
            ));
        }
        for (current, failed, cleanup) in [
            (false, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            assert!(!retirement_ready(
                "drained",
                false,
                current,
                failed,
                cleanup,
                Some(["revoked", "revoked"]),
                Some([0, 0])
            ));
        }
        assert!(!retirement_ready(
            "drained",
            false,
            true,
            false,
            false,
            None,
            Some([0, 0])
        ));
        assert!(!retirement_ready(
            "drained",
            false,
            true,
            false,
            false,
            Some(["revoked", "revoked"]),
            None
        ));
    }
}
