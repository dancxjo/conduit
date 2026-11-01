//! Opt-in cold measurement before local Play. No writes, admission, quiet
//! permit, timing qualification or allocator-grant change follows this result.
use super::NativeRoot;
impl NativeRoot {
    pub(crate) fn measure_checkpoint_preparation(&self) {
        let Some(owner) = self.body.as_deref() else {
            esp_println::println!("{{\"event\":\"checkpoint-preparation-probe\",\"error\":\"body-not-installed\",\"timing_qualified\":false}}");
            return;
        };
        let before = esp_alloc::HEAP.stats();
        let started = embassy_time::Instant::now();
        let result =
            crate::native_restart::owned_checkpoint::probe(owner, &self.admission.verifying_key());
        let elapsed = embassy_time::Instant::now()
            .as_micros()
            .saturating_sub(started.as_micros());
        let after = esp_alloc::HEAP.stats();
        match result {
            Ok(report) => esp_println::println!(
                "{{\"event\":\"checkpoint-preparation-probe\",\"phase\":\"cold-replay\",\"source_generation\":{},\"source_bytes\":{},\"output_bytes\":{},\"elapsed_us\":{},\"root_used_before\":{},\"root_used_after\":{},\"root_lifetime_peak_before\":{},\"root_lifetime_peak_after\":{},\"phase_peak_measured\":false,\"timing_qualified\":false}}",
                report.source_generation, report.source_bytes, report.output_bytes,
                elapsed, before.current_usage, after.current_usage, before.max_usage, after.max_usage,
            ),
            Err(reason) => esp_println::println!(
                "{{\"event\":\"checkpoint-preparation-probe\",\"phase\":\"cold-replay\",\"error\":\"{}\",\"elapsed_us\":{},\"timing_qualified\":false}}",
                reason, elapsed,
            ),
        }
    }
}
