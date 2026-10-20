//! Deferred field-copy from an opaque current owner origin into reserved buffers.
use super::*;
impl Machine {
    pub fn reserve_binding(&mut self) {
        self.prior_beam.reserve_exact(BYTES - self.prior_beam.len());
        self.initial_raw
            .reserve_exact(BYTES - self.initial_raw.len());
    }
    pub fn bind_current(
        &mut self,
        canonical: &[u8],
        material: &mut Materializer,
        identity: u64,
        invocation: u64,
    ) -> R<()> {
        if !matches!(self.phase, Phase::Seed) {
            return Err(CursorRefusal::Lifecycle);
        }
        // Every field comes from the exact retained current Native representation.
        // The ordinary seed/input Native checks and Source programs still execute.
        put(&mut self.prior_beam, canonical)?;
        let names = ["candidate0", "candidate1", "candidate2", "candidate3"];
        for (index, name) in names.iter().enumerate() {
            material.raw_hypothesis(member(canonical, &[name])?, &mut self.slots[index])?;
        }
        material.record(
            "LanguageAnalysisInitialRawBeam",
            &[
                (names[0], view(&self.slots[0])?),
                (names[1], view(&self.slots[1])?),
                (names[2], view(&self.slots[2])?),
                (names[3], view(&self.slots[3])?),
            ],
            &mut self.initial_raw,
        )?;
        self.identity = identity;
        self.invocation = invocation;
        Ok(())
    }
}
