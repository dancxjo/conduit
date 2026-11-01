//! Bounded native network workspace and terminal-outcome diagnostics only.
use super::NativeRoot;

// Exact immutable status payloads live in flash once. These diagnostics never
// run in ROM flash callbacks or while the flash cache is disabled.
#[used]
#[link_section = ".rodata.native_network_status"]
pub(super) static NETWORK_RECOVERY_REFUSED: [u8; 16] = *b"recovery-refused";
#[used]
#[link_section = ".rodata.native_network_status"]
pub(super) static NETWORK_RETRY_SOURCE_REFUSED: [u8; 32] = *b"retry-source-preparation-refused";
#[cfg(feature = "local-appliance")]
#[used]
#[link_section = ".rodata.native_network_status"]
static NETWORK_NON_ASCII: [u8; 16] = *b"non-ascii-status";
// Diagnostic suppression only: a collision may omit a line, never affect
// admission, ownership or execution. No Root field or received authority.
#[cfg(feature = "local-appliance")]
static WORKSPACE_TRACE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "flat-network-appliance")]
static WORKSPACE_PHASE_LINES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "local-appliance")]
#[inline(never)]
#[cold]
fn changed_workspace_outcome(stage: &str, outcome: &str) -> bool {
    let mut fingerprint = 2166136261u32;
    for byte in stage
        .bytes()
        .chain(core::iter::once(0))
        .chain(outcome.bytes())
    {
        fingerprint = (fingerprint ^ u32::from(byte)).wrapping_mul(16777619);
    }
    WORKSPACE_TRACE.swap(fingerprint, core::sync::atomic::Ordering::Relaxed) != fingerprint
}
impl NativeRoot {
    #[cfg(feature = "local-appliance")]
    #[inline(never)]
    #[cold]
    pub(super) fn trace_network_workspace(&mut self, stage: &'static str, outcome: &'static str) {
        if !changed_workspace_outcome(stage, outcome) {
            return;
        }
        let memory = esp_alloc::HEAP.stats();
        let banks = core::array::from_fn::<_, 3, _>(|i| {
            memory.region_stats[i].as_ref().map_or(0, |r| r.free)
        });
        esp_println::println!(
            "{{\"event\":\"network-workspace-outcome\",\"stage\":\"{}\",\"outcome\":\"{}\",\"free_bytes\":{},\"bank_free\":[{},{},{}],\"scope_current\":{},\"credential_revision\":{},\"recovery_revision\":{}}}",
            stage, outcome, memory.size.saturating_sub(memory.current_usage), banks[0], banks[1], banks[2],
            self.network_workspace_scope_current(), crate::native_credentials::committed_revision(), self.network.recovery.revision()
        );
    }
    #[cfg(feature = "local-appliance")]
    #[inline(never)]
    #[cold]
    pub(super) fn trace_network_floor_refusal(&self, backed: usize, required: usize) {
        esp_println::println!(
            "{{\"event\":\"network-floor-refused\",\"backed_bytes\":{},\"required_bytes\":{}}}",
            backed,
            required
        );
    }
    #[cfg(feature = "local-appliance")]
    #[inline(never)]
    #[cold]
    pub(super) fn trace_network_preparation(&mut self, outcome: &'static [u8]) {
        // Keep status bytes borrowed from flash through one shared conversion;
        // specializing UTF-8 checks at each caller materializes RAM constants.
        let outcome = core::str::from_utf8(outcome).unwrap_or_else(|_| {
            // SAFETY: this private immutable payload is the ASCII literal above.
            unsafe { core::str::from_utf8_unchecked(&NETWORK_NON_ASCII) }
        });
        self.trace_network_workspace("prepare", outcome);
    }
    #[cfg(feature = "flat-network-appliance")]
    #[inline(never)]
    #[cold]
    pub(super) fn trace_network_phase(&self, phase: &'static str, baseline_used: usize) {
        // Diagnostic quota only. It cannot grant capacity or suppress checks.
        let count = WORKSPACE_PHASE_LINES.load(core::sync::atomic::Ordering::Relaxed);
        if count >= 12
            || WORKSPACE_PHASE_LINES
                .compare_exchange(
                    count,
                    count + 1,
                    core::sync::atomic::Ordering::Relaxed,
                    core::sync::atomic::Ordering::Relaxed,
                )
                .is_err()
        {
            return;
        }
        let memory = esp_alloc::HEAP.stats();
        let banks = core::array::from_fn::<_, 3, _>(|i| {
            memory.region_stats[i].as_ref().map_or(0, |r| r.free)
        });
        esp_println::println!("{{\"event\":\"network-workspace-phase\",\"phase\":\"{}\",\"backing_bytes\":{},\"baseline_used\":{},\"used\":{},\"peak\":{},\"free_bytes\":{},\"bank_free\":[{},{},{}]}}",
            phase, if phase == "preparation-ready" { conduit_network_join_call::PreparedNetworkKernelBacking::allocation_bytes() } else { conduit_network_join_call::PreparedNetworkKernelStorage::allocation_bytes() }, baseline_used,
            memory.current_usage, memory.max_usage, memory.size.saturating_sub(memory.current_usage), banks[0], banks[1], banks[2]);
    }
    /// Called only in the joining failure branch immediately before failed()
    /// changes this exact recovery generation to Restoring. No per-poll log.
    #[cfg(not(feature = "network-workspace-check"))]
    #[inline(never)]
    #[cold]
    pub(super) fn trace_network_terminal_failure(&self, generation: u64) {
        if let Some(observation) = self.play.station_attempt_inspection() {
            let progress = observation.progress;
            let polls = crate::network::runner_polls();
            esp_println::println!(
                "{{\"event\":\"network-station-attempt-observed\",\"generation\":{},\"substage\":\"{:?}\",\"attempt_started_ms\":{},\"stage_started_ms\":{},\"observed_at_ms\":{},\"link_up\":{},\"address_up\":{},\"services_ready\":{},\"sdk_error\":\"{:?}\",\"runner_polls\":[{},{}]}}",
                generation, progress.stage, progress.attempt_started_ms.unwrap_or(0),
                progress.stage_started_ms, progress.observed_at_ms, progress.link_up,
                progress.address_up, progress.services_ready, observation.sdk_error,
                polls[0], polls[1]
            );
        }
        let (steps, dispatches) = self.play.progress();
        esp_println::println!(
            "{{\"event\":\"network-play-failed\",\"status\":\"{}\",\"failure_reason\":\"{}\",\"failure_phase\":\"{}\",\"generation\":{},\"steps\":{},\"dispatches\":[{},{},{}],\"receipt_received\":{}}}",
            self.play.status(), self.play.failure_reason(), self.play.failure_phase(),
            generation, steps, dispatches[0], dispatches[1], dispatches[2],
            self.play.receipt_received()
        );
    }
}

/// Final physical backing failed before any attempt or issuer reservation.
#[cfg(feature = "local-appliance")]
#[cold]
#[inline(never)]
pub(super) fn trace_floor_reproof_refusal(backed: usize, required: usize) {
    esp_println::println!(
        "{{\"event\":\"network-floor-reproof-refused\",\"backed_bytes\":{},\"required_bytes\":{}}}",
        backed,
        required
    );
}
