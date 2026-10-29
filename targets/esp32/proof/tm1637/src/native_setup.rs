//! One admitted Root construction workspace. Acknowledged setup input remains
//! queued until native custody releases it or an independent workspace is admitted.
//! Recovery always retires selected ownership and commits evidence before retry.
use super::NativeRoot;
#[path = "native_setup_diagnostics.rs"]
mod diagnostics;
#[cfg(feature = "local-appliance")]
#[path = "native_network_workspace_reserve.rs"]
mod floor_reserve;
#[path = "native_setup_input.rs"]
mod input;
#[path = "native_setup_retirement.rs"]
mod retirement;
#[path = "native_network_retirement_backing.rs"]
mod retirement_backing;
#[cfg(all(feature = "network-scan", feature = "local-appliance"))]
#[path = "native_setup_scan.rs"]
pub(super) mod scan;
#[cfg(feature = "network-workspace-check")]
#[path = "native_setup_workspace_check.rs"]
mod workspace_check;
#[cfg(feature = "local-appliance")]
use crate::native_display::task::WorkspacePermit;
use crate::native_network::recovery::Action;
#[cfg(feature = "local-appliance")]
use conduit_body::BodyPlanEvidence;
use conduit_core as c;
#[cfg(not(feature = "local-appliance"))]
struct WorkspacePermit<'a>(core::marker::PhantomData<&'a ()>);
#[cfg(not(feature = "local-appliance"))]
impl WorkspacePermit<'_> {
    fn held(&self) -> bool {
        false
    }
}
#[cfg(feature = "local-appliance")]
pub(super) struct PendingWorkspace {
    request: crate::native_display::task::WorkspaceRequest<'static>,
    deadline_ms: u64,
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
    /// Preparation never consumes the selected source or changes radio mode.
    /// Once configured, subsequent offers reuse the retained SDK configuration.
    pub fn prepare_network_configuration(&mut self) -> &'static [u8] {
        self.prepare_network_configuration_in(None)
    }
    fn prepare_network_configuration_in(
        &mut self,
        window: Option<&WorkspacePermit<'_>>,
    ) -> &'static [u8] {
        self.prepare_network_configuration_mode(window, true)
    }
    fn prepare_network_configuration_mode(
        &mut self,
        window: Option<&WorkspacePermit<'_>>,
        release: bool,
    ) -> &'static [u8] {
        self.prepare_network_configuration_backed(window, release, None)
    }
    fn prepare_network_configuration_backed(
        &mut self,
        window: Option<&WorkspacePermit<'_>>,
        release: bool,
        backing: Option<conduit_network_join_call::PreparedNetworkKernelStorage>,
    ) -> &'static [u8] {
        #[cfg(not(feature = "flat-network-appliance"))]
        let _ = backing;
        if self.network.retirement.is_some() {
            return b"retirement-evidence-pending";
        }
        if !self.play.permits_unselected_configuration() {
            return self.play.status().as_bytes();
        }
        // Mandatory evidence must have capacity before private input is staged,
        // provider ownership moves, or a recovery attempt can be consumed.
        #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
        if let Err(reason) = crate::native_play::journal::admission_status() {
            return reason.as_bytes();
        }
        if self.native_workspace_held() && !window.is_some_and(|window| window.held()) {
            return b"awaiting-native-retirement";
        }
        let now = embassy_time::Instant::now().as_millis();
        if let Err(reason) = self.accept_acknowledged_network_configuration(now) {
            return reason;
        }
        if !self.network.recovery.attempt_due(now) {
            return b"waiting";
        }
        // Actual old owners/evidence returned before changing a saved profile.
        // This does not consume a later AP input or change recovery revision.
        if let Err(reason) = self.stage_saved_network_retry() {
            #[cfg(all(feature = "network-scan", feature = "local-appliance"))]
            if reason == "credential-policy-exhausted" {
                drop(backing);
                return self.prepare_recovery_scan(window);
            }
            return reason.as_bytes();
        }
        let candidate = match self.network.recovery.preview_attempt(now) {
            Ok(Some(candidate)) => candidate,
            Ok(None) => return b"retry-backoff",
            Err(_) => return &diagnostics::NETWORK_RECOVERY_REFUSED,
        };
        if candidate.generation() != self.network.generation() {
            return b"attachment-generation-refused";
        }
        let Some(selected_generation) = self.offer_generation.checked_add(2) else {
            return b"offer-generation-exhausted";
        };
        if let Err(reason) = self.network.prepare(&self.host, &self.boot) {
            return reason.as_bytes();
        }
        self.offer_generation = selected_generation - 1;
        #[cfg(feature = "flat-network-appliance")]
        let result = {
            let image = crate::native_source::load();
            let result = if window.is_some() {
                let (ready, recovery) = self.network.ready_and_recovery();
                let admit = || {
                    if !window.is_some_and(|window| window.held()) {
                        return Err(());
                    }
                    recovery
                        .begin_attempt(candidate, embassy_time::Instant::now().as_millis())
                        .map(|_| ())
                        .map_err(|_| ())
                };
                match backing {
                    Some(storage) => self.play.install_linked_phased_held_with_storage(
                        &image,
                        &self.network_source,
                        c::OfferGeneration(self.offer_generation),
                        &mut self.credentials,
                        ready,
                        &mut self.issuer,
                        storage,
                        admit,
                    ),
                    None => self.play.install_linked_phased_held(
                        &image,
                        &self.network_source,
                        c::OfferGeneration(self.offer_generation),
                        &mut self.credentials,
                        ready,
                        &mut self.issuer,
                        admit,
                    ),
                }
            } else {
                if self
                    .network
                    .recovery
                    .begin_attempt(candidate, embassy_time::Instant::now().as_millis())
                    .is_err()
                {
                    return &diagnostics::NETWORK_RECOVERY_REFUSED;
                }
                match backing {
                    Some(backing) => self.play.install_linked_reserved_held(
                        &image,
                        &self.network_source,
                        c::OfferGeneration(self.offer_generation),
                        &mut self.credentials,
                        self.network.ready_slot(),
                        &mut self.issuer,
                        backing,
                    ),
                    None => self.play.install_linked_held(
                        &image,
                        &self.network_source,
                        c::OfferGeneration(self.offer_generation),
                        &mut self.credentials,
                        self.network.ready_slot(),
                        &mut self.issuer,
                    ),
                }
            };
            if result.is_ok() && window.is_none() && release {
                self.play.release_held()
            } else {
                result
            }
        };
        #[cfg(not(feature = "flat-network-appliance"))]
        let result = {
            if self.prepare_network_plan().is_err() {
                return b"refused";
            }
            let prepared = match self.network_preparation.take() {
                Some(Ok(prepared)) => prepared,
                refusal => {
                    self.network_preparation = refusal;
                    return b"planning-refused";
                }
            };
            if self
                .network
                .recovery
                .begin_attempt(candidate, embassy_time::Instant::now().as_millis())
                .is_err()
            {
                self.network_preparation = Some(Ok(prepared));
                return &diagnostics::NETWORK_RECOVERY_REFUSED;
            }
            let result = if window.is_some() || !release {
                self.play.install_verified_owned_held(
                    prepared,
                    &mut self.credentials,
                    self.network.ready_slot(),
                    &mut self.issuer,
                )
            } else {
                self.play.install_verified_owned(
                    prepared,
                    &mut self.credentials,
                    self.network.ready_slot(),
                    &mut self.issuer,
                )
            };
            result
        };
        self.network.mark_selected();
        self.offer_generation = selected_generation;
        if result.is_err() {
            return b"installation-refused";
        }
        if let Some(window) = window {
            let memory = esp_alloc::HEAP.stats();
            if !window.held() || memory.size.saturating_sub(memory.current_usage) < 8192 + 8192 {
                return b"installed-held-pressure";
            }
            if release && self.play.release_held().is_err() {
                return b"installed-held-release-refused";
            }
        }
        if release {
            b"installed"
        } else {
            b"installed-held"
        }
    }
    #[cfg(feature = "local-appliance")]
    fn advance_network_workspace(&mut self) {
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
        // Checked preparation and actual execution backing share the unchanged
        // 8KiB envelope above the physically protected 16KiB floor.
        if memory.size.saturating_sub(memory.current_usage) < minimum {
            self.trace_network_workspace("capacity", "aggregate-construction-refused");
            return;
        }
        if !self.network_workspace_scope_current() {
            self.trace_network_workspace("scope", "current-body-refused");
            return;
        }
        // Protect both actual execution allocations before the floor blocks can
        // fragment their holes. The unique storage conveys no execution authority.
        #[cfg(feature = "flat-network-appliance")]
        let backing = match crate::native_play::NativePlay::reserve_linked_backing() {
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
            conduit_network_join_call::PreparedNetworkKernelStorage::allocation_bytes()
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
        let reserve = match floor_reserve::reserve(|bytes| {
            if bytes != floor_reserve::BLOCK_BYTES {
                return Err(());
            }
            type Block = [core::mem::MaybeUninit<u8>; floor_reserve::BLOCK_BYTES];
            let layout = core::alloc::Layout::new::<Block>();
            // SAFETY: the exact nonzero Block layout is used for allocation and
            // Box deallocation. MaybeUninit bytes need no initialization, are
            // never read, and have no element destructor. Null retains refusal;
            // only a successful unique allocation becomes an owned thin Box.
            let raw = unsafe { alloc::alloc::alloc(layout) }.cast::<Block>();
            if raw.is_null() {
                Err(())
            } else {
                Ok(unsafe { alloc::boxed::Box::from_raw(raw) })
            }
        }) {
            Ok(reserve) => reserve,
            Err(_) => {
                self.trace_network_workspace("reserve", "floor-backing-refused");
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
    fn network_workspace_scope_current(&self) -> bool {
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
    /// Cold synchronous preparation finishes before the execution/recovery
    /// future is polled. No future frame for retirement or SDK dispatch remains
    /// on the planner's stack; the same held installation guards still apply.
    #[inline(never)]
    pub fn prepare_setup(&mut self) {
        #[cfg(not(feature = "network-workspace-check"))]
        {
            if self.network.retirement.is_some() {
                return;
            }
            #[cfg(all(feature = "network-scan", feature = "local-appliance"))]
            if self.scan.is_some() {
                return;
            }
            #[cfg(feature = "local-appliance")]
            self.advance_network_workspace();
            self.prepare_network_configuration();
        }
    }
    /// Compatibility entrance for control callers without a separate prepare
    /// phase. The appliance loop uses the phase-separated entrances directly.
    pub async fn advance_setup(&mut self) {
        self.prepare_setup();
        self.advance_setup_prepared().await;
    }
    /// Independent control polling. Current interface/service evidence controls
    /// availability; a receipt from an old successful join is insufficient.
    pub async fn advance_setup_prepared(&mut self) {
        #[cfg(feature = "network-workspace-check")]
        {
            self.check_network_workspace();
            // This diagnostic may retain selected affine owners and a kernel,
            // but never releases, Steps, runs or restores that held selection.
            return;
        }
        #[cfg(not(feature = "network-workspace-check"))]
        self.advance_setup_enabled().await;
    }
    #[cfg(not(feature = "network-workspace-check"))]
    async fn advance_setup_enabled(&mut self) {
        #[cfg(all(feature = "network-scan", feature = "local-appliance"))]
        if self.scan.is_some() {
            self.advance_recovery_scan().await;
            return;
        }
        if self.network.retirement.is_some() {
            let _ = self.finish_network_retirement();
            return;
        }
        self.play.run().await;
        let generation = self.network.recovery.generation();
        let mut action = Action::None;
        if self.network.recovery.is_joining() {
            match self.play.status() {
                "attached" => {
                    let _ = self.network.recovery.attached(generation);
                }
                "access-point-restored"
                | "cleanup-failed"
                | "setup-denied"
                | "installation-refused" => {
                    self.trace_network_terminal_failure(generation);
                    action = self
                        .network
                        .recovery
                        .failed(generation)
                        .unwrap_or(Action::None);
                }
                _ => (),
            }
        }
        if action == Action::None
            && self
                .credential_writer
                .try_lock()
                .is_ok_and(|mailbox| mailbox.has_acknowledged())
        {
            // A flushed response queues retirement, not configuration acceptance.
            // The mailbox and revision remain intact until owners return below.
            action = self
                .network
                .recovery
                .configuration_retirement(embassy_time::Instant::now().as_millis())
                .unwrap_or(Action::None);
        }
        if action == Action::None {
            // Monitoring may request restoration, never spend an attempt.
            // Cold preparation supplies the actual admitted workspace.
            let workspace = false;
            action = self
                .network
                .recovery
                .poll(
                    embassy_time::Instant::now().as_millis(),
                    workspace,
                    self.play.station_services_available(),
                )
                .unwrap_or(Action::None);
        }
        // Recovery emits restoration once. After actual cleanup, storage pressure
        // retries only custody transfer, without another cleanup or attempt.
        if action == Action::None
            && self.network.recovery.is_restoring()
            && self.play.status() == "access-point-restored"
        {
            action = Action::RestoreAccessPoint { generation };
        }
        if let Action::RestoreAccessPoint { generation } = action {
            if self.play.status() != "access-point-restored"
                && self.play.restore_for_retirement().await.is_err()
            {
                let _ = self.network.recovery.restored(
                    generation,
                    embassy_time::Instant::now().as_millis(),
                    false,
                );
                return;
            }
            match retirement_backing::retain_restored(&mut self.play) {
                Ok(retired) => {
                    self.network.retirement = Some(retired);
                    let _ = self.finish_network_retirement();
                }
                Err("network-retirement-capacity") => {
                    #[cfg(feature = "local-appliance")]
                    self.trace_network_workspace("retirement", "network-retirement-capacity");
                }
                Err(_) => {
                    let _ = self.network.recovery.restored(
                        generation,
                        embassy_time::Instant::now().as_millis(),
                        false,
                    );
                }
            }
        }
    }
    pub fn control_binding(&self) -> Option<&c::ActivePlayIdentity> {
        self.play.active()
    }
    pub fn control_progress(&self) -> (u16, [u8; 3], bool) {
        let (steps, calls) = self.play.progress();
        (steps, calls, self.play.receipt_received())
    }
    pub fn control_installed(&self) -> bool {
        self.play.installed()
    }
    pub fn control_status(&self) -> &'static str {
        if self.network.retirement.is_some() {
            #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
            if crate::native_play::journal::exhausted() {
                return crate::native_play::journal::capacity_refusal();
            }
            return "retirement-evidence-pending";
        }
        if self.network.recovery.awaiting_attempt() && !self.play.attempted() {
            return "retry-backoff";
        }
        if !self.play.attempted()
            && self.native_workspace_held()
            && self
                .credential_writer
                .try_lock()
                .is_ok_and(|m| m.has_acknowledged())
        {
            return "awaiting-native-retirement";
        }
        self.play.status()
    }
    fn native_workspace_held(&self) -> bool {
        self.appliance_kernel_prepared()
            || (self.owner_reservation.is_some() && self.retired_native.is_none())
    }
    pub fn control_failure(&self) -> (&'static str, &'static str) {
        (self.play.failure_phase(), self.play.failure_reason())
    }
    pub fn network_ready(&self) -> bool {
        self.network.is_ready()
    }
}
