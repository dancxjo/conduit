//! Cumulative Root preparation high-water mark with the complete record retained.
//! This is measurement, not memory admission or a private-service heap claim.
pub(super) fn record(prefix: &[u8], limit: usize, bytes: usize) {
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    if prefix == b"CONDUIT_NATIVE_FACE " && limit > 96 * 1024 {
        use core::fmt::Write;
        let arena = &crate::allocation::BOOT_ARENA;
        let mut line = crate::sign_format::FixedText::new();
        if writeln!(line,
            "CONDUIT_NATIVE_COLLECTION_MEMORY {{\"scope\":\"cumulative-root-preparation-with-complete-observer-record\",\"peak_live_bytes\":{},\"live_bytes\":{},\"capacity_bytes\":{},\"observer_bytes\":{}}}",
            arena.used(), arena.live_bytes(), arena.capacity(), bytes,
        ).is_ok() {
            crate::arch::early_write(line.as_bytes());
        }
    }
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    let _ = (prefix, limit, bytes);
}
