//! Signed Prepare gate for the finite, dormant display construction diagnostic.
use super::super::NativeRoot;
impl NativeRoot {
    pub(super) async fn prepare_display_kernel(
        &mut self,
        permit: crate::native_owner_reservation::PreparationPermit,
        wire: &[u8],
    ) -> Result<(), &'static str> {
        let reservation = self.owner_reservation.as_ref().ok_or("owner-unreserved")?;
        if !permit.matches(reservation)
            || conduit_core::assigned_plan_payload_digest(wire) != reservation.assignment_seal
            || self.assignment_seal != Some(reservation.assignment_seal)
        {
            return Err("assignment-seal");
        }
        use crate::native_display::CONSTRUCTION_FRAMES;
        // Binding's temporary contract/record storage must leave before the
        // kernel's persistent stores are allocated. Both owners stay dormant;
        // a later refusal retains actual custody for retry or explicit cleanup.
        if !self.display.native_bound() {
            let withdrawn = self
                .offer_generation
                .checked_add(1)
                .ok_or("offer-generation-exhausted")?;
            let stats = esp_alloc::HEAP.stats();
            self.display.bind_native(
                &mut self.issuer,
                reservation,
                CONSTRUCTION_FRAMES,
                stats.size.saturating_sub(stats.current_usage),
            )?;
            self.offer_generation = withdrawn;
        }
        if !self.display.kernel_prepared() {
            let withdrawn = self
                .offer_generation
                .checked_add(1)
                .ok_or("offer-generation-exhausted")?;
            let stats = esp_alloc::HEAP.stats();
            self.display.prepare_kernel(
                conduit_environment_runtime::display_output::AssignedDisplayScope {
                    host: &self.host,
                    boot: &self.boot,
                    generation: conduit_core::OfferGeneration(reservation.offer_generation),
                },
                CONSTRUCTION_FRAMES,
                stats.size.saturating_sub(stats.current_usage),
            )?;
            self.offer_generation = withdrawn;
        }
        if self.display.context_bytes().is_none() {
            let stats = esp_alloc::HEAP.stats();
            self.display
                .prepare_context(stats.size.saturating_sub(stats.current_usage))?;
        }
        let spawner = self.worker_spawner.ok_or("worker-executor-unavailable")?;
        self.display.register_task(spawner).await?;
        Ok(())
    }
}

#[cfg(feature = "display-ready-root")]
impl NativeRoot {
    pub fn display_task_status(&self) -> Option<crate::native_display::DisplayTaskStatus> {
        self.display.task_status()
    }
}
