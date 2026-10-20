//! Private immutable Beam sharing. Allocations occur only during preparation.
//! Slot membership and generated Native values do not grant Current authority.
use super::{EpochShared, LanguageParserScorerBeam};
use core::ops::Deref;
struct BeamSlot {
    value: Option<LanguageParserScorerBeam>,
}
/// Future installed pool entries are allocated empty, before Play.
#[derive(Clone)]
pub(super) struct PreparedBeamSlot(EpochShared<BeamSlot>);
/// Only a successfully filled slot exposes a Native Beam reference.
#[derive(Clone)]
pub(super) struct SharedBeam(PreparedBeamSlot);
impl PreparedBeamSlot {
    pub(super) fn prepare_empty() -> Self {
        Self(EpochShared::new(BeamSlot { value: None }))
    }
    // Failure returns the exact Native and slot; it never clones or allocates.
    #[allow(clippy::result_large_err)]
    pub(super) fn fill(
        mut self,
        value: LanguageParserScorerBeam,
    ) -> Result<SharedBeam, (Self, LanguageParserScorerBeam)> {
        let Some(slot) = EpochShared::get_mut(&mut self.0) else {
            return Err((self, value));
        };
        if slot.value.is_some() {
            return Err((self, value));
        }
        slot.value = Some(value);
        Ok(SharedBeam(self))
    }
}
impl SharedBeam {
    /// Existing hosted/initial preparation boundary; never called by apply.
    pub(super) fn prepare(value: LanguageParserScorerBeam) -> Self {
        match PreparedBeamSlot::prepare_empty().fill(value) {
            Ok(slot) => slot,
            Err(_) => unreachable!("fresh unique empty Beam slot"),
        }
    }
    pub(super) fn get(&self) -> &LanguageParserScorerBeam {
        self.0
             .0
            .value
            .as_ref()
            .expect("filled immutable Beam slot")
    }
}
impl Deref for SharedBeam {
    type Target = LanguageParserScorerBeam;
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    // This fixture tests ownership only; it is not Source execution evidence.
    fn beam() -> LanguageParserScorerBeam {
        let relation = LanguageParserRelation::new(
            LanguageUniversalDependencyRelation::Dep,
            LanguageParserSubtype::new("".into()).unwrap(),
        )
        .unwrap();
        let state = LanguageParserState::new(
            LanguageParserBasis::new(
                LanguageAnalysisRevisionId::new("analysis/slot".into()).unwrap(),
                LanguageTextRevisionId::new("source/slot".into()).unwrap(),
                LanguageTextId::new("text/slot".into()).unwrap(),
            )
            .unwrap(),
            0,
            1,
            [5, 5, 5, 5, 4],
            relation.clone(),
            relation.clone(),
            relation.clone(),
            relation,
            [4; 5],
            1,
            0,
        )
        .unwrap();
        let parser = LanguageParserHypothesis::new(false, 0, 0, state).unwrap();
        let slot = LanguageParserScorerHypothesis::new([0; 4], parser, 0).unwrap();
        LanguageParserScorerBeam::new(slot.clone(), slot.clone(), slot.clone(), slot).unwrap()
    }
    #[test]
    fn only_unique_empty_slot_moves_native_and_shared_occupied_refusals_return_it() {
        let expected = beam();
        let empty = PreparedBeamSlot::prepare_empty();
        let alias = empty.clone();
        let (empty, value) = match empty.fill(expected.clone()) {
            Err(value) => value,
            Ok(_) => panic!("shared slot filled"),
        };
        assert_eq!(value, expected);
        drop(alias);
        let filled = match empty.fill(value) {
            Ok(value) => value,
            Err(_) => panic!("unique empty slot refused"),
        };
        let retained = filled.clone();
        assert!(core::ptr::eq(filled.get(), retained.get()));
        assert_eq!(filled.get(), &expected);
        let occupied = PreparedBeamSlot(EpochShared::new(BeamSlot {
            value: Some(expected.clone()),
        }));
        let (occupied, value) = match occupied.fill(expected.clone()) {
            Err(value) => value,
            Ok(_) => panic!("occupied slot overwritten"),
        };
        assert_eq!(value, expected);
        assert_eq!(occupied.0.value.as_ref(), Some(&expected));
    }
}
