//! Checked declaration metadata follows the ordinary advertisement generation.
impl crate::StdHost {
    pub fn set_declared_name(
        &mut self,
        name: conduit_core::HostDeclaredName,
    ) -> Result<(), String> {
        if self.advertisement.declared_name.as_ref() == Some(&name) {
            return Ok(());
        }
        if !self.kernel_resources.is_idle() {
            return Err("Host name cannot change during active reservations".into());
        }
        let next = self
            .advertisement
            .offer_generation
            .0
            .checked_add(1)
            .ok_or("Host offer generation exhausted")?;
        self.advance_retained_spoken_offer_generation(conduit_core::OfferGeneration(next))?;
        self.advertisement.declared_name = Some(name);
        self.advertisement.offer_generation = conduit_core::OfferGeneration(next);
        Ok(())
    }
}
