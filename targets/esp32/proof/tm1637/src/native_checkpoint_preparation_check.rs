//! Opt-in cold measurement before local Play. No writes, admission, quiet
//! permit, timing qualification or allocator-grant change follows this result.
use super::NativeRoot;
impl NativeRoot {
    pub(crate) fn measure_checkpoint_preparation(&self) {
        let Some(owner) = self.body.as_deref() else {
            esp_println::println!("{{\"event\":\"checkpoint-preparation-probe\",\"error\":\"body-not-installed\",\"timing_qualified\":false}}");
            return;
        };
        let result = crate::native_restart::owned_checkpoint::probe_frozen(
            owner,
            &self.admission.verifying_key(),
            || embassy_time::Instant::now().as_micros(),
            || {
                let s = esp_alloc::HEAP.stats();
                [s.current_usage, s.max_usage]
            },
        );
        match result {
            Ok(r) => esp_println::println!(
                "{{\"event\":\"checkpoint-preparation-probe\",\"phase\":\"readonly-frozen-prepare-capture\",\"source_generation\":{},\"source_bytes\":{},\"output_bytes\":{},\"prepare_us\":{},\"capture_us\":{},\"retained_cache_bytes\":{},\"heap_before\":{:?},\"heap_after_prepare\":{:?},\"heap_after_capture\":{:?},\"heap_after_release\":{:?},\"heap_pair\":\"root_current_boot_lifetime_peak\",\"physical_operations\":0,\"membership_staged\":false,\"start_measured\":false,\"chunk_measured\":false,\"finish_measured\":false,\"phase_peak_measured\":false,\"total_ram_measured\":false,\"during_local_play\":false,\"timing_qualified\":false}}",
                r.source_generation, r.source_bytes, r.output_bytes, r.prepare_us, r.capture_us,
                r.retained_cache_bytes, r.heap_before, r.heap_after_prepare,
                r.heap_after_capture, r.heap_after_release,
            ),
            Err(reason) => esp_println::println!(
                "{{\"event\":\"checkpoint-preparation-probe\",\"phase\":\"readonly-frozen-prepare-capture\",\"error\":\"{}\",\"physical_operations\":0,\"timing_qualified\":false}}", reason,
            ),
        }
    }
}
