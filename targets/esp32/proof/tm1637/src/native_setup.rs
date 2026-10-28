//! One admitted Root construction workspace. Acknowledged setup input remains
//! queued until native custody releases it or an independent workspace is admitted.
//! Recovery always retires selected ownership and commits evidence before retry.
use super::NativeRoot;
#[cfg(feature = "local-appliance")]
#[path = "native_network_workspace_reserve.rs"]
mod floor_reserve;
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
// The fixed linked appliance was measured under actual retired Mask custody:
// 7,564 retained bytes / 44,776us, with an 8KiB contiguous allocation still
// available afterward. Round construction plus the existing 16KiB post-install
// reserve up to 24KiB. This admits only that pinned construction path; the rich
// planner has no corresponding target bound. The post-install check below and
// actual affine ownership checks remain mandatory before releasing the kernel.
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
// Exact immutable status payloads live in flash once. These diagnostics never
// run in ROM flash callbacks or while the flash cache is disabled.
#[used]
#[link_section = ".rodata.native_network_status"]
static NETWORK_PREPARED: [u8; 8] = *b"prepared";
#[used]
#[link_section = ".rodata.native_network_status"]
static NETWORK_RECOVERY_REFUSED: [u8; 16] = *b"recovery-refused";
#[used]
#[link_section = ".rodata.native_network_status"]
static NETWORK_RETRY_SOURCE_REFUSED: [u8; 32] = *b"retry-source-preparation-refused";
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
        if self.play.attempted() {
            return self.play.status().as_bytes();
        }
        if self.native_workspace_held() && !window.is_some_and(|window| window.held()) {
            return b"awaiting-native-retirement";
        }
        if self.network.is_ready() {
            return &NETWORK_PREPARED;
        }
        let now = embassy_time::Instant::now().as_millis();
        if self.network.recovery.access_point_only() {
            let acknowledged = self
                .credential_writer
                .try_lock()
                .is_ok_and(|mailbox| mailbox.has_acknowledged());
            if !acknowledged
                && crate::native_credentials::committed_revision()
                    <= self.network.recovery.revision()
            {
                return b"waiting";
            }
            let configuration =
                match crate::native_credentials::prepare_configuration(&mut self.credentials) {
                    Ok(configuration) => configuration,
                    Err("credential-unavailable" | "credential-not-acknowledged") => {
                        return b"waiting";
                    }
                    Err(_) => return b"refused",
                };
            let revision = match crate::native_credentials::committed_revision() {
                0 => match self.network.recovery.revision().checked_add(1) {
                    Some(revision) => revision,
                    None => return b"configuration-exhausted",
                },
                revision => revision,
            };
            if self.network.stage(configuration).is_err() {
                return b"refused";
            }
            if self.network.recovery.configuration(revision, now).is_err() {
                return b"configuration-refused";
            }
        }
        if !self.network.recovery.awaiting_attempt() {
            return b"waiting";
        }
        // The previous selected read consumed its snapshot. Re-freeze SAME
        // chosen credentials only after real owner/evidence return, before
        // reserving a retry. New queued AP input cannot alter retained SDK config.
        if crate::native_credentials::prepare_retry(&mut self.credentials).is_err() {
            return &NETWORK_RETRY_SOURCE_REFUSED;
        }
        let generation = match self.network.recovery.poll(now, true, false) {
            Ok(Action::PrepareAttempt { generation, .. }) => generation,
            Ok(_) => return b"retry-backoff",
            Err(_) => return &NETWORK_RECOVERY_REFUSED,
        };
        if generation != self.network.generation() {
            return b"attachment-generation-refused";
        }
        let Some(selected_generation) = self.offer_generation.checked_add(2) else {
            return b"offer-generation-exhausted";
        };
        if self.network.prepare(&self.host, &self.boot).is_err() {
            return b"refused";
        }
        self.offer_generation = selected_generation - 1;
        #[cfg(feature = "flat-network-appliance")]
        let result = {
            let image = crate::native_source::load();
            let result = match backing {
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
    #[inline(never)]
    #[cold]
    fn trace_network_workspace(&mut self, stage: &'static str, outcome: &'static str) {
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
    fn trace_network_preparation(&mut self, outcome: &'static [u8]) {
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
    fn trace_network_phase(&self, phase: &'static str, baseline_used: usize) {
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
            phase, conduit_network_join_call::PreparedNetworkKernelStorage::allocation_bytes(), baseline_used,
            memory.current_usage, memory.max_usage, memory.size.saturating_sub(memory.current_usage), banks[0], banks[1], banks[2]);
    }
    #[cfg(feature = "local-appliance")]
    fn advance_network_workspace(&mut self) {
        let Some(minimum) = independent_workspace_minimum() else {
            self.pending_network_workspace = None;
            self.trace_network_workspace("eligibility", "profile-unbounded");
            return;
        };
        if !self.native_workspace_held()
            || self.play.attempted()
            || self.network.retirement.is_some()
        {
            self.pending_network_workspace = None;
            return;
        }
        let eligible = self.network.recovery.awaiting_attempt()
            || (self.network.recovery.access_point_only()
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
        let now = embassy_time::Instant::now().as_millis();
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
        #[cfg(feature = "flat-network-appliance")]
        let backing_bytes =
            conduit_network_join_call::PreparedNetworkKernelStorage::allocation_bytes();
        #[cfg(not(feature = "flat-network-appliance"))]
        let backing_bytes = 0;
        let Some(required) = minimum.checked_add(backing_bytes) else {
            return;
        };
        if memory.size.saturating_sub(memory.current_usage) < required {
            self.trace_network_workspace("capacity", "aggregate-construction-refused");
            return;
        }
        if !self.network_workspace_scope_current() {
            self.trace_network_workspace("scope", "current-body-refused");
            return;
        }
        // Charge all persistent execution storage (kernel and empty Sign log)
        // plus the unchanged 8KiB preparation envelope and 16KiB release floor.
        #[cfg(feature = "flat-network-appliance")]
        let backing = match crate::native_play::NativePlay::reserve_linked_backing() {
            Ok(backing) => Some(backing),
            Err(()) => {
                self.trace_network_workspace("reserve", "kernel-backing-refused");
                return;
            }
        };
        #[cfg(not(feature = "flat-network-appliance"))]
        let backing = None;
        #[cfg(feature = "flat-network-appliance")]
        self.trace_network_phase("backing-reserved", baseline_used);
        if esp_alloc::HEAP.free() < minimum
            || !permit.held()
            || !self.network_workspace_scope_current()
        {
            self.trace_network_workspace("reserve", "remaining-workspace-refused");
            return;
        }
        // The constructor makes multiple allocations. Its unchanged aggregate
        // envelope is not a request for one contiguous 24KiB block. Protect the
        // actual 16KiB release floor across small regions. The same reserved
        // final backing initializes after temporary planner descriptions retire.
        let reserve = match floor_reserve::reserve(|bytes| {
            let mut block = alloc::vec::Vec::<u8>::new();
            block.try_reserve_exact(bytes).map(|()| block)
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
        if !self.play.installed() {
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
                    action = self
                        .network
                        .recovery
                        .failed(generation)
                        .unwrap_or(Action::None);
                }
                _ => (),
            }
        }
        if action == Action::None {
            let workspace = !self.native_workspace_held();
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
            match self.play.take_retired() {
                Ok(retired) => {
                    self.network.retirement = Some(alloc::boxed::Box::new(retired));
                    let _ = self.finish_network_retirement();
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
    fn finish_network_retirement(&mut self) -> Result<(), &'static str> {
        #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
        {
            if !self.network.can_reclaim_restored() {
                return Err("network-reclaim-preflight");
            }
            let retired = self
                .network
                .retirement
                .as_mut()
                .ok_or("network-retirement-unavailable")?;
            if !retired.owners_recoverable() {
                return Err("network-retirement-generation-exhausted");
            }
            retired.commit_evidence()?;
            self.issuer
                .finish_network_play(retired)
                .map_err(|_| "network-retirement-issuer")?;
            let (provider, source) = retired
                .take_owners()
                .expect("durably committed actual owners");
            self.network
                .reclaim_restored(provider)
                .unwrap_or_else(|_| unreachable!("checked restored AP owner"));
            self.credentials = Some(
                crate::native_credentials::reprepare(source, &self.host, &self.boot)
                    .unwrap_or_else(|_| unreachable!("checked returned private source")),
            );
            // Mandatory evidence is now readback-committed in append-only flash.
            self.network.retirement = None;
            let generation = self.network.recovery.generation();
            self.network
                .recovery
                .restored(generation, embassy_time::Instant::now().as_millis(), true)
                .map_err(|_| "network-recovery-transition")?;
            Ok(())
        }
        #[cfg(not(any(feature = "local-evidence-store", feature = "network-credential-store")))]
        {
            Err("network-recovery-durable-custody-required")
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
