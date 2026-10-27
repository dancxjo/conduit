//! Passive, one-shot target scheduling observation. No construction or admission.
use super::{NativeRoot, PendingWorkspace};
use core::sync::atomic::{AtomicBool, Ordering};
static DONE: AtomicBool = AtomicBool::new(false);
const TIMEOUT_MS: u64 = 4000;

impl NativeRoot {
    pub(super) fn check_network_workspace(&mut self) {
        if DONE.load(Ordering::Relaxed) {
            return;
        }
        let now = embassy_time::Instant::now().as_millis();
        if self.pending_network_workspace.is_none() {
            if !self
                .worker
                .as_ref()
                .is_some_and(|w| w.status().source_sequence >= 1)
            {
                return;
            }
            let Some(worker) = &self.local_mask_worker else {
                return;
            };
            let Ok(request) = worker.request_workspace() else {
                return;
            };
            let Some(deadline_ms) = now.checked_add(TIMEOUT_MS) else {
                return;
            };
            self.pending_network_workspace = Some(PendingWorkspace {
                request,
                deadline_ms,
            });
        }
        let pending = self
            .pending_network_workspace
            .as_ref()
            .expect("pending diagnostic");
        let requested = pending.deadline_ms - TIMEOUT_MS;
        if now >= pending.deadline_ms {
            // Drop cancels only this exact generation; printing follows release.
            drop(self.pending_network_workspace.take());
            DONE.store(true, Ordering::Relaxed);
            esp_println::println!("{{\"event\":\"network-workspace-check\",\"outcome\":\"timeout\",\"latency_ms\":{}}}", now - requested);
            return;
        }
        if !pending.request.ready() {
            return;
        }
        let pending = self
            .pending_network_workspace
            .take()
            .expect("ready diagnostic");
        let Ok(permit) = pending.request.claim() else {
            DONE.store(true, Ordering::Relaxed);
            esp_println::println!(
                "{{\"event\":\"network-workspace-check\",\"outcome\":\"claim-refused\"}}"
            );
            return;
        };
        let entered = embassy_time::Instant::now().as_micros();
        let memory = esp_alloc::HEAP.stats();
        let free = memory.size.saturating_sub(memory.current_usage);
        let banks = memory.region_stats.map(|s| s.map_or(0, |s| s.free));
        let source = self
            .worker
            .as_ref()
            .map_or(0, |w| w.status().source_sequence);
        let mask = self.local_mask_worker.as_ref().map(|w| w.status());
        let completed = mask.map_or(0, |s| s.completed);
        let current = mask.is_some_and(|s| s.task_active && s.phase == "running");
        let held = permit.held();
        drop(permit);
        let elapsed = embassy_time::Instant::now()
            .as_micros()
            .saturating_sub(entered);
        DONE.store(true, Ordering::Relaxed);
        esp_println::println!("{{\"event\":\"network-workspace-check\",\"outcome\":\"observed\",\"latency_ms\":{},\"held_us\":{},\"root_size\":{},\"root_used\":{},\"root_free\":{},\"root_peak\":{},\"bank_free\":[{},{},{}],\"source_sequence\":{},\"mask_completed\":{},\"held\":{},\"current\":{}}}", now - requested, elapsed, memory.size, memory.current_usage, free, memory.max_usage, banks[0], banks[1], banks[2], source, completed, held, current);
    }
}
