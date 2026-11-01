//! Finite independent workspace admission beside retained local execution.
//! Scheduling permits do not enlarge capacity or establish execution authority.
use super::{floor_reserve, NativeRoot};
use conduit_body::BodyPlanEvidence;

#[cfg(feature = "local-appliance")]
pub(in super::super) struct PendingWorkspace {
    pub(super) request: crate::native_display::task::WorkspaceRequest<'static>,
    pub(super) deadline_ms: u64,
}
// Historical pre-staging target evidence was 7,564 retained bytes / 44,776us,
// with an 8KiB contiguous allocation afterward; it does not qualify this path.
// Keep the existing 8KiB construction envelope + 16KiB release floor = 24KiB.
// The revised staged constructor's CPU64 peak is 8,131 bytes. This is fixture
// evidence, not a target-wide scope bound. Initial Ready preparation,
// credential staging, target allocator overhead and stack still require actual
// target qualification. The rich planner has no corresponding bounded profile.
// Post-install capacity and actual affine custody checks remain mandatory.
#[cfg(feature = "local-appliance")]
fn independent_workspace_minimum() -> Option<usize> {
    #[cfg(feature = "flat-network-appliance")]
    {
        Some(24 * 1024)
    }
    #[cfg(not(feature = "flat-network-appliance"))]
    {
        None
    }
}
impl NativeRoot {
    #[cfg(feature = "local-appliance")]
    pub(super) fn advance_network_workspace(&mut self) {
        let Some(minimum) = independent_workspace_minimum() else {
            self.pending_network_workspace = None;
            self.trace_network_workspace("eligibility", "profile-unbounded");
            return;
        };
        if !self.native_workspace_held()
            || !self.play.permits_unselected_configuration()
            || self.network.retirement.is_some()
        {
            self.pending_network_workspace = None;
            return;
        }
        // Refuse before requesting or claiming construction workspace. Keep
        // acknowledged input and independently installed local Play untouched.
        #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
        if let Err(reason) = crate::native_play::journal::admission_status() {
            self.pending_network_workspace = None;
            self.trace_network_workspace("eligibility", reason);
            return;
        }
        let now = embassy_time::Instant::now().as_millis();
        let eligible = self.network.recovery.attempt_due(now)
            || (self.network.recovery.accepts_configuration()
                && (self
                    .credential_writer
                    .try_lock()
                    .is_ok_and(|m| m.has_acknowledged())
                    || crate::native_credentials::committed_revision()
                        > self.network.recovery.revision()));
        if !eligible {
            self.pending_network_workspace = None;
            self.trace_network_workspace("eligibility", "not-eligible");
            return;
        }
        if self
            .pending_network_workspace
            .as_ref()
            .is_some_and(|p| now >= p.deadline_ms)
        {
            self.pending_network_workspace = None;
            self.trace_network_workspace("request", "timeout");
            return;
        }
        if self.pending_network_workspace.is_none() {
            let Some(worker) = &self.local_mask_worker else {
                self.trace_network_workspace("request", "worker-absent");
                return;
            };
            let request = worker
                .request_workspace()
                .or_else(|_| worker.request_terminal_workspace());
            let request = match request {
                Ok(request) => request,
                Err(reason) => {
                    self.trace_network_workspace("request", reason);
                    return;
                }
            };
            let Some(deadline_ms) = now.checked_add(4000) else {
                self.trace_network_workspace("request", "clock-exhausted");
                return;
            };
            self.pending_network_workspace = Some(PendingWorkspace {
                request,
                deadline_ms,
            });
        }
        if !self
            .pending_network_workspace
            .as_ref()
            .is_some_and(|p| p.request.ready())
        {
            self.trace_network_workspace("request", "waiting");
            return;
        }
        let pending = self
            .pending_network_workspace
            .take()
            .expect("checked pending workspace");
        let Ok(permit) = pending.request.claim() else {
            self.trace_network_workspace("request", "claim-refused");
            return;
        };
        let memory = esp_alloc::HEAP.stats();
        #[cfg(feature = "flat-network-appliance")]
        let baseline_used = memory.current_usage;
        #[cfg(feature = "flat-network-appliance")]
        self.trace_network_phase("permit", baseline_used);
        if !permit.held() {
            self.trace_network_workspace("scope", "permit-revoked");
            return;
        }
        // Checked preparation and complete execution backing share the unchanged
        // 8KiB envelope above the physically protected 16KiB floor.
        if memory.size.saturating_sub(memory.current_usage) < minimum {
            self.trace_network_workspace("capacity", "aggregate-construction-refused");
            return;
        }
        if !self.network_workspace_scope_current() {
            self.trace_network_workspace("scope", "current-body-refused");
            return;
        }
        // Protect the unique kernel allocation before floor blocks can fragment
        // its hole. Unchanged Sign backing is allocated after checked temporary
        // descriptions retire, while this same floor remains held. No authority
        // is conveyed by uninitialized backing.
        #[cfg(feature = "flat-network-appliance")]
        let backing = match crate::native_play::NativePlay::reserve_linked_kernel_backing() {
            Ok(storage) => Some(storage),
            Err(()) => {
                self.trace_network_workspace("reserve", "execution-backing-refused");
                return;
            }
        };
        #[cfg(not(feature = "flat-network-appliance"))]
        let backing = None;
        // Initial admission already charged the unchanged complete minimum.
        // Only its now-owned exact backing is subtracted to avoid double count.
        let Some(remaining_minimum) = minimum.checked_sub(backing.as_ref().map_or(0, |_| {
            conduit_network_join_call::PreparedNetworkKernelBacking::allocation_bytes()
        })) else {
            self.trace_network_workspace("reserve", "execution-backing-exceeds-workspace");
            return;
        };
        #[cfg(feature = "flat-network-appliance")]
        self.trace_network_phase("preparation-ready", baseline_used);
        if esp_alloc::HEAP.free() < remaining_minimum
            || !permit.held()
            || !self.network_workspace_scope_current()
        {
            self.trace_network_workspace("reserve", "remaining-workspace-refused");
            return;
        }
        // The constructor makes multiple allocations. Its unchanged aggregate
        // envelope is not a request for one contiguous 24KiB block. Protect the
        // actual 16KiB release floor across small regions while both bounded
        // preparation and execution-storage admission run above it.
        let mut floor_allocator = floor_reserve::Global;
        let reserve = match floor_reserve::reserve(&mut floor_allocator) {
            Ok(reserve) => reserve,
            Err(refused) => {
                self.trace_network_workspace("reserve", "floor-backing-refused");
                self.trace_network_floor_refusal(refused.backed_bytes, refused.required_bytes);
                return;
            }
        };
        let prepared = self.prepare_network_configuration_backed(Some(&permit), false, backing);
        #[cfg(feature = "flat-network-appliance")]
        self.trace_network_phase("prepared-floor-held", baseline_used);
        drop(reserve);
        #[cfg(feature = "network-scan")]
        let scan_prepared = self.scan.is_some();
        #[cfg(not(feature = "network-scan"))]
        let scan_prepared = false;
        if !self.play.installed() && !scan_prepared {
            self.trace_network_preparation(prepared);
            return;
        }
        let memory = esp_alloc::HEAP.stats();
        if !permit.held() || !self.network_workspace_scope_current() {
            self.trace_network_workspace("release", "current-scope-refused");
            return;
        }
        if memory.size.saturating_sub(memory.current_usage) < 8192 + 8192 {
            self.trace_network_workspace("release", "floor-refused");
            return;
        }
        #[cfg(feature = "network-scan")]
        if scan_prepared {
            let outcome = if self.release_recovery_scan().is_ok() {
                "scan-released"
            } else {
                "scan-held-release-refused"
            };
            self.trace_network_workspace("release", outcome);
            return;
        }
        let outcome = if self.play.release_held().is_ok() {
            "released"
        } else {
            "held-release-refused"
        };
        self.trace_network_workspace("release", outcome);
        // No await while holding construction custody. Drop precedes run().
        drop(permit);
    }
    #[cfg(feature = "local-appliance")]
    pub(super) fn network_workspace_scope_current(&self) -> bool {
        let Some(body) = self.body.as_ref() else {
            return false;
        };
        let Some(current) = body.realization() else {
            return false;
        };
        let Some(play) = current.play.as_ref() else {
            return false;
        };
        let Some(active) = self.issuer.mask_issued_active() else {
            return false;
        };
        play.validate_for(&current.plan)
            && current.plan.validate_for(&current.wake).is_ok()
            && active.host_id == self.host
            && active.boot_id == self.boot
            && matches!(&body.evidence().body.state, conduit_body::BodyState::Awake { wake_id } if wake_id == current.plan.wake_id())
            && current
                .plan
                .masks()
                .iter()
                .flat_map(|m| m.chains())
                .any(|chain| chain.plan().plan_id() == &active.plan_id)
            && body
                .evidence()
                .membership
                .parts
                .iter()
                .filter(|part| part.state == conduit_body::MembershipState::Admitted)
                .filter_map(|part| part.current.as_ref())
                .any(|host| host.host_id == self.host && host.boot_id == self.boot)
    }
}
