//! One-shot target measurement using acknowledged private input and real owners.
//! Selected construction remains held forever in this diagnostic Boot. No Step,
//! radio call, production admission, ownership rollback or synthetic credentials.
use super::{NativeRoot, PendingWorkspace};
use core::sync::atomic::{AtomicBool, Ordering};
static DONE: AtomicBool = AtomicBool::new(false);
const TIMEOUT_MS: u64 = 4000;

impl NativeRoot {
    pub(super) fn check_network_workspace(&mut self) {
        if DONE.load(Ordering::Relaxed) {
            return;
        }
        // Wait for actual response-acknowledged setup. Do not manufacture a
        // private source or silently measure an unrelated persisted credential.
        if !self
            .credential_writer
            .try_lock()
            .is_ok_and(|mailbox| mailbox.has_acknowledged())
        {
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
            esp_println::println!(
                "{{\"event\":\"network-workspace-check\",\"outcome\":\"timeout\",\"latency_ms\":{}}}",
                now - requested
            );
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
        let held = permit.held();
        let before = esp_alloc::HEAP.stats();
        let banks_before = before.region_stats.map(|s| s.map_or(0, |s| s.free));
        let progress_before = self.play.progress();
        let entered = embassy_time::Instant::now().as_micros();
        let result = if held {
            self.prepare_network_configuration_mode(Some(&permit), false)
        } else {
            b"workspace-refused"
        };
        let elapsed = embassy_time::Instant::now()
            .as_micros()
            .saturating_sub(entered);
        let after = esp_alloc::HEAP.stats();
        let banks_after = after.region_stats.map(|s| s.map_or(0, |s| s.free));
        let progress_after = self.play.progress();
        let no_dispatch = progress_before == progress_after;
        let probes = contiguous_probes();
        let source = self
            .worker
            .as_ref()
            .map_or(0, |w| w.status().source_sequence);
        let mask = self.local_mask_worker.as_ref().map(|w| w.status());
        let completed = mask.map_or(0, |s| s.completed);
        let current = mask.is_some_and(|s| s.task_active && s.phase == "running");
        drop(permit);
        DONE.store(true, Ordering::Relaxed);
        let outcome = core::str::from_utf8(result).unwrap_or("invalid-status");
        esp_println::println!(
            "{{\"event\":\"network-workspace-check\",\"outcome\":\"{}\",\"latency_ms\":{},\"construction_us\":{},\"root_size\":{},\"before_used\":{},\"after_used\":{},\"before_peak\":{},\"after_peak\":{},\"bank_free_before\":[{},{},{}],\"bank_free_after\":[{},{},{}],\"contiguous_probe_mask\":{},\"source_sequence\":{},\"mask_completed\":{},\"held\":{},\"current\":{},\"no_dispatch\":{},\"play_phase\":\"{}\"}}",
            outcome,
            now - requested,
            elapsed,
            after.size,
            before.current_usage,
            after.current_usage,
            before.max_usage,
            after.max_usage,
            banks_before[0],
            banks_before[1],
            banks_before[2],
            banks_after[0],
            banks_after[1],
            banks_after[2],
            probes,
            source,
            completed,
            held,
            current,
            no_dispatch,
            self.play.status()
        );
    }
}
/// Bits 0..5 report successful contiguous allocations of 256..8192 bytes at
/// alignment 8, immediately returned. This is a fragmentation lower bound, not
/// an exact largest-block measurement. Probes preserve an 8KiB free reserve;
/// they run after construction timing/peak snapshots, never as admission.
fn contiguous_probes() -> u8 {
    use core::alloc::{GlobalAlloc, Layout};
    let mut mask = 0;
    for (bit, bytes) in [256, 512, 1024, 2048, 4096, 8192].into_iter().enumerate() {
        if esp_alloc::HEAP.free() < bytes + 8192 {
            continue;
        }
        let layout = Layout::from_size_align(bytes, 8).expect("fixed probe layout");
        // SAFETY: use the same allocator/layout for allocation and immediate
        // deallocation. No reference, value or pointer escapes the probe.
        let pointer = unsafe { esp_alloc::HEAP.alloc(layout) };
        if !pointer.is_null() {
            mask |= 1 << bit;
            unsafe { esp_alloc::HEAP.dealloc(pointer, layout) };
        }
    }
    mask
}
