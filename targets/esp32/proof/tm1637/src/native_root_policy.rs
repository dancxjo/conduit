//! Actual initialized-owner policy and readonly scope access, shared with the CPU proof.
use super::{c, NativeRoot};
impl NativeRoot {
    pub fn prepare_appliance_kernel(&mut self) -> Result<(), &'static str> {
        if self.appliance_kernel.is_some() {
            return Err("already-prepared");
        }
        if self.owner_reservation.is_some() {
            return Err("owner-reserved");
        }
        self.appliance_kernel = Some(crate::native_kernel::prepare(
            &mut self.software,
            &mut self.assignment,
        )?);
        self.assignment_seal = None;
        Ok(())
    }

    pub fn offer_scope(&self) -> (&c::HostId, &c::BootId, c::OfferGeneration) {
        (
            &self.host,
            &self.boot,
            c::OfferGeneration(self.offer_generation),
        )
    }
    pub fn software(&self) -> Option<&crate::native_software::Software> {
        self.software.as_deref()
    }
    /// Root policy over still-retained providers. A projected grant is not a
    /// locally issued capability, Play reservation or Body start.
    pub fn appliance_policy_grants(&self) -> Result<[c::AuthorityGrant; 2], ()> {
        self.software.as_ref().ok_or(())?;
        let wait = self.wait.as_ref().ok_or(())?;
        let (_, cadence) = crate::native_software::source_facts();
        if cadence.count == 0 {
            return Err(());
        }
        Ok([
            self.issuer
                .grant_pulse(self.pulse.as_ref().ok_or(())?.native_identity())?,
            self.issuer.grant_wait(wait.native_identity())?,
        ])
    }
    #[cfg(feature = "native-face-root")]
    pub fn face_inspection(
        &self,
        now: u64,
    ) -> Result<Option<crate::native_face::Inspection>, &'static str> {
        self.face_reader
            .as_ref()
            .map(|reader| reader.inspect(now))
            .transpose()
    }
}
