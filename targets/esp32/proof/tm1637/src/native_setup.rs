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
#[path = "native_setup_workspace.rs"]
mod workspace;
#[cfg(feature = "local-appliance")]
pub(super) use workspace::PendingWorkspace;
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
        if self.membership_busy() {
            return b"body-membership-busy";
        }
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
    /// Cold synchronous preparation finishes before the execution/recovery
    /// future is polled. No future frame for retirement or SDK dispatch remains
    /// on the planner's stack; the same held installation guards still apply.
    #[inline(never)]
    pub fn prepare_setup(&mut self) {
        if self.membership_busy() {
            return;
        }
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
        if self.membership_busy() {
            return;
        }
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
        if let Err(reason) = self.network.restore_failed_preparation_service().await {
            #[cfg(feature = "local-appliance")]
            self.trace_network_workspace("preparation-services", reason);
            #[cfg(not(feature = "local-appliance"))]
            let _ = reason;
        }
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
