//! Signed Prepare gate for the finite, dormant display construction diagnostic.
use super::super::NativeRoot;
impl NativeRoot {
    pub(super) fn prepare_display_kernel(
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
        const CONSTRUCTION_FRAMES: u32 = 2;
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
        if !self.display.native_bound() {
            let stats = esp_alloc::HEAP.stats();
            self.display.bind_native(
                &mut self.issuer,
                reservation,
                CONSTRUCTION_FRAMES,
                stats.size.saturating_sub(stats.current_usage),
            )?;
        }
        let stats = esp_alloc::HEAP.stats();
        self.display
            .prepare_context(stats.size.saturating_sub(stats.current_usage))?;
        Ok(())
    }
}
