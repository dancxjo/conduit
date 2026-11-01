//! The existing fixed child window, with failure-only scalar diagnostics.
//! This check cannot alter cadence, issue a lease, or grant additional work.
pub(super) fn check(site: &'static str, now: u64, due: u64) -> Result<(), &'static str> {
    if now < due || now.saturating_sub(due) >= 3000 {
        #[cfg(target_os = "none")]
        esp_println::println!(
            "{{\"event\":\"mask-child-window-refused\",\"site\":\"{}\",\"now_ms\":{},\"due_ms\":{},\"late_ms\":{},\"early\":{}}}",
            site, now, due, now.saturating_sub(due), now < due
        );
        #[cfg(not(target_os = "none"))]
        let _ = site;
        Err("mask-child-window")
    } else {
        Ok(())
    }
}
