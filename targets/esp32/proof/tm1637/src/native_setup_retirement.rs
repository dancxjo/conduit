//! Actual committed control evidence and returned source/provider handoff.
use super::NativeRoot;
impl NativeRoot {
    pub(super) fn finish_network_retirement(&mut self) -> Result<(), &'static str> {
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
            retired.settle_profile_outcome(embassy_time::Instant::now().as_millis())?;
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
}
