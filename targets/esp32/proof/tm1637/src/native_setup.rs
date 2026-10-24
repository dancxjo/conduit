//! One admitted Root construction workspace. Acknowledged setup input remains
//! queued until native custody actually releases it; this performs no retry.
use super::NativeRoot;
use conduit_core as c;
impl NativeRoot {
    /// Preparation never consumes the selected source or changes radio mode.
    /// Once configured, subsequent offers reuse the retained SDK configuration.
    pub fn prepare_network_configuration(&mut self) -> &'static [u8] {
        use conduit_environment_http::network_setup::NetworkSetupRefusal as E;
        if self.play.attempted() {
            return self.play.status().as_bytes();
        }
        if self.native_workspace_held() {
            return b"awaiting-native-retirement";
        }
        if self.network.is_ready() {
            return b"prepared";
        }
        if self.network.has_staged_configuration() {
            // A refused preparation still retains its initialized owner. The
            // one-submission diagnostic never overwrites or silently retries it.
            return b"refused";
        }
        let configuration = {
            let Ok(mailbox) = self.credential_writer.try_lock() else {
                return b"busy";
            };
            mailbox
                .prepare_acknowledged(conduit_esp32_network_switch::prepare_station_configuration)
        };
        match configuration {
            Ok(Ok(config)) => {
                self.network.stage(config);
                if self.network.prepare(&self.host, &self.boot).is_err() {
                    return b"refused";
                }
                self.offer_generation += 1;
                // The retained provider changes the truthful offer generation.
                // Preparation is fresh; old refusal/Plan material is discarded.
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
                // Selected owners and kernel retain their exact bindings. The
                // rich construction Plan/lowering is not continuing execution
                // custody and is released after installation, on either result.
                let result = self.play.install(
                    &prepared,
                    &mut self.credentials,
                    self.network.ready_slot(),
                    &mut self.issuer,
                );
                self.network.mark_selected();
                self.offer_generation += 1;
                if result.is_err() {
                    return b"installation-refused";
                }
                b"installed"
            }
            Ok(Err(_)) => b"refused",
            Err(E::NoSubmission | E::NotAcknowledged) => b"waiting",
            Err(_) => b"refused",
        }
    }
    /// Poll independently of UART management. HTTP acknowledgement is required
    /// before preparation, selection or the control Play can consume input.
    pub async fn advance_setup(&mut self) {
        self.prepare_network_configuration();
        self.play.run().await;
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
