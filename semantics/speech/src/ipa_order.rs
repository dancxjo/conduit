//! Shared finite ordering law for phonetic and profile-scoped IPA notation.
use crate::ipa_unicode::UnitKind;

#[derive(Default)]
pub(crate) struct NotationOrder {
    previous: Option<UnitKind>,
    pending_stress: bool,
}
impl NotationOrder {
    pub(crate) fn admit(&mut self, kind: UnitKind) -> Result<(), ()> {
        match kind {
            UnitKind::Segment => self.pending_stress = false,
            UnitKind::PrimaryStress | UnitKind::SecondaryStress => {
                if self.pending_stress {
                    return Err(());
                }
                self.pending_stress = true;
            }
            UnitKind::Length if self.previous != Some(UnitKind::Segment) => return Err(()),
            UnitKind::SyllableBoundary
                if self.pending_stress
                    || !matches!(self.previous, Some(UnitKind::Segment | UnitKind::Length)) =>
            {
                return Err(())
            }
            _ => (),
        }
        self.previous = Some(kind);
        Ok(())
    }
    pub(crate) fn is_complete(&self) -> bool {
        !self.pending_stress && matches!(self.previous, Some(UnitKind::Segment | UnitKind::Length))
    }
}
