//! Add the encounter Back only before this Body has proposed or started Play.
use super::Owner;
use conduit_core::ResourceOffer;
use conduit_std_host::CrecheAttachmentRefusal;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OwnerCrecheAttachmentRefusal {
    BodyActive,
    Host(CrecheAttachmentRefusal),
}
impl Owner {
    /// Change only the current idle Host's offers. The Body, biography, workset,
    /// initialized equipment and existing Host custody remain retained. This
    /// grants no source, make, save, display, planning or execution authority.
    #[allow(dead_code)] // Native application composition supplies the admitted slot.
    pub(crate) fn attach_creche_encounter(
        &mut self,
        slot: ResourceOffer,
    ) -> Result<(), OwnerCrecheAttachmentRefusal> {
        if self.host.is_playing() || self.session.realization().is_some() {
            return Err(OwnerCrecheAttachmentRefusal::BodyActive);
        }
        self.host
            .attach_idle_creche(slot)
            .map_err(OwnerCrecheAttachmentRefusal::Host)
    }
}

#[cfg(test)]
#[path = "creche_attachment/tests.rs"]
mod tests;
