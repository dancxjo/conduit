//! One admitted Root construction workspace. Acknowledged setup input remains
//! queued until native custody releases it or an independent workspace is admitted.
//! Recovery always retires selected ownership and commits evidence before retry.
use super::NativeRoot;
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
pub(super) struct PendingWorkspace {
    request: crate::native_display::task::WorkspaceRequest<'static>,
    deadline_ms: u64,
}
// Deliberately absent until target peak, fragmentation and duration are proven.
// CPU allocation measurements cannot authorize concurrent native construction.
#[cfg(feature = "local-appliance")]
fn independent_workspace_minimum() -> Option<usize> {
    None
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
            return b"prepared";
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
                match crate::native_credentials::prepare_configuration(&self.credential_writer) {
                    Ok(configuration) => configuration,
                    Err("credential-unavailable" | "credential-not-acknowledged") => {
                        return b"waiting"
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
        let generation = match self.network.recovery.poll(now, true, false) {
            Ok(Action::PrepareAttempt { generation, .. }) => generation,
            Ok(_) => return b"retry-backoff",
            Err(_) => return b"recovery-refused",
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
        let result = if window.is_some() {
            self.play.install_owned_held(
                prepared,
                &mut self.credentials,
                self.network.ready_slot(),
                &mut self.issuer,
            )
        } else {
            self.play.install_owned(
                prepared,
                &mut self.credentials,
                self.network.ready_slot(),
                &mut self.issuer,
            )
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
            if self.play.release_held().is_err() {
                return b"installed-held-release-refused";
            }
        }
        b"installed"
    }
    #[cfg(feature = "local-appliance")]
    fn advance_network_workspace(&mut self) {
        let Some(minimum) = independent_workspace_minimum() else {
            self.pending_network_workspace = None;
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
            return;
        }
        let now = embassy_time::Instant::now().as_millis();
        if self
            .pending_network_workspace
            .as_ref()
            .is_some_and(|p| now >= p.deadline_ms)
        {
            self.pending_network_workspace = None;
            return;
        }
        if self.pending_network_workspace.is_none() {
            let Some(worker) = &self.local_mask_worker else {
                return;
            };
            let Ok(request) = worker.request_workspace() else {
                return;
            };
            let Some(deadline_ms) = now.checked_add(4000) else {
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
            return;
        }
        let pending = self
            .pending_network_workspace
            .take()
            .expect("checked pending workspace");
        let Ok(permit) = pending.request.claim() else {
            return;
        };
        let memory = esp_alloc::HEAP.stats();
        if permit.held() && memory.size.saturating_sub(memory.current_usage) >= minimum {
            self.prepare_network_configuration_in(Some(&permit));
        }
        // No await while holding construction custody. Drop precedes run().
        drop(permit);
    }
    /// Independent control polling. Current interface/service evidence controls
    /// availability; a receipt from an old successful join is insufficient.
    pub async fn advance_setup(&mut self) {
        if self.network.retirement.is_some() {
            let _ = self.finish_network_retirement();
            return;
        }
        #[cfg(feature = "local-appliance")]
        self.advance_network_workspace();
        self.prepare_network_configuration();
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
