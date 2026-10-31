//! Optional capacity-return preflight and descriptive handoff diagnostics.
//! Unknown headroom never revokes the currently running local instrument.
use super::*;
impl NativeRoot {
    pub(super) fn local_capacity_returned(&self) -> bool {
        if !crate::native_observed_archive::recovery_capacity_available() {
            return false;
        }
        let Some(sampler) = self.worker.as_ref().map(|w| w.status()) else {
            return false;
        };
        let Some(mask) = self.local_mask_worker.as_ref().map(|w| w.status()) else {
            return false;
        };
        sampler.failure.is_none()
            && !sampler.cleanup_failure
            && matches!(sampler.phase, "running" | "drained" | "quiesced")
            && mask.failure.is_none()
            && mask.cleanup_refusal.is_none()
            && mask.activated
            && mask.completed > 0
            && mask.completed <= crate::native_display::MAXIMUM_CHILDREN
            && matches!(mask.phase, "running" | "quiesced")
    }
    #[inline(never)]
    pub(super) fn capacity_checkpoint_preflight(&mut self) -> bool {
        let extent = crate::native_restart::checkpoint_extent().ok();
        if policy::early_checkpoint_safe(extent) {
            return true;
        }
        let reason = if extent.is_some() {
            "renewal-checkpoint-headroom"
        } else {
            "renewal-checkpoint-unavailable"
        };
        if self.local_renewal_archive_refusal != Some(reason) {
            let (generation, bytes) = extent.unwrap_or((0, 0));
            esp_println::println!("{{\"event\":\"local-recording-recovery-refused\",\"reason\":\"{}\",\"checkpoint_generation\":{},\"checkpoint_bytes\":{},\"maximum_checkpoint_bytes\":{}}}", reason, generation, bytes, policy::MAXIMUM_EARLY_CHECKPOINT_BYTES);
        }
        self.local_renewal_archive_refusal = Some(reason);
        false
    }
}
#[inline(never)]
pub(super) fn report_completion(renewal: &Renewal) {
    let elapsed = embassy_time::Instant::now()
        .as_millis()
        .saturating_sub(renewal.started_ms);
    let last_reading_age = renewal.last_reading_ms.map(|time| {
        embassy_time::Instant::now()
            .as_millis()
            .saturating_sub(time)
    });
    if let Some(age) = last_reading_age {
        esp_println::println!("{{\"event\":\"local-finite-renewed\",\"reason\":\"{}\",\"old_sampler_sequence\":{},\"handoff_ms\":{},\"last_reading_age_ms\":{}}}",renewal.reason.name(),renewal.old_sampler.play_sequence,elapsed,age);
    } else {
        esp_println::println!("{{\"event\":\"local-finite-renewed\",\"reason\":\"{}\",\"old_sampler_sequence\":{},\"handoff_ms\":{},\"last_reading_age_ms\":null}}",renewal.reason.name(),renewal.old_sampler.play_sequence,elapsed);
    }
}
