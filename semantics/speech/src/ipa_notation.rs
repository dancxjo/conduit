//! Declared IPA syntax with exact profile custody and original source spelling.
//! Preparation is an ordinary allocating path, not a bounded Flow execution.
use crate::{
    ipa_partition::{partition, PartitionRefusal, UnitSpelling},
    ipa_unicode::{supported_unit, UnitKind},
    semantic::*,
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};

#[derive(Debug)]
pub enum IpaNotationRefusal {
    InvalidProfile,
    UnsupportedSpelling,
    DisplayDelimiters,
    UnsupportedTranscription,
    AmbiguousTranscription,
    Capacity,
    SuprasegmentalOrder,
    Native(NativeBindingRefusal),
}
fn kind(value: &SpeechIpaUnitKind) -> UnitKind {
    match value {
        SpeechIpaUnitKind::Segment => UnitKind::Segment,
        SpeechIpaUnitKind::PrimaryStress => UnitKind::PrimaryStress,
        SpeechIpaUnitKind::SecondaryStress => UnitKind::SecondaryStress,
        SpeechIpaUnitKind::Length => UnitKind::Length,
        SpeechIpaUnitKind::SyllableBoundary => UnitKind::SyllableBoundary,
    }
}
/// Exact complete supplied profile, never a global registry or identity lookup.
pub struct PreparedIpaNotationProfile<'a> {
    profile: &'a SpeechIpaNotationProfile,
    spellings: Vec<UnitSpelling<'a>>,
    unit_syntax: Vec<SpeechIpaUnitSyntaxAdmission>,
}
impl<'a> PreparedIpaNotationProfile<'a> {
    pub fn unit_syntax(&self) -> &[SpeechIpaUnitSyntaxAdmission] {
        &self.unit_syntax
    }
    pub fn profile(&self) -> &'a SpeechIpaNotationProfile {
        self.profile
    }
    pub fn prepare(profile: &'a SpeechIpaNotationProfile) -> Result<Self, IpaNotationRefusal> {
        use IpaNotationRefusal::*;
        let mut spellings = Vec::new();
        let mut unit_syntax = Vec::new();
        for (index, unit) in profile.units().as_slice().iter().enumerate() {
            if profile.units().as_slice()[..index].iter().any(|prior| {
                prior.identity() == unit.identity() || prior.spelling() == unit.spelling()
            }) {
                return Err(InvalidProfile);
            }
            if !supported_unit(unit.spelling().get().as_str(), kind(unit.kind())) {
                return Err(UnsupportedSpelling);
            }
            unit_syntax.push(SpeechIpaUnitSyntaxAdmission::new(unit.clone()).map_err(Native)?);
            spellings.push(UnitSpelling {
                spelling: unit.spelling().get().as_str(),
                unit: index,
            });
        }
        for alias in profile.aliases().as_slice() {
            let index = profile
                .units()
                .as_slice()
                .iter()
                .position(|unit| unit.identity() == alias.target())
                .ok_or(InvalidProfile)?;
            let canonical = &profile.units()[index];
            let source = alias.source().get().as_str();
            // Version1 supports only the explicit precomposed/decomposed nasal
            // spelling pair. Even that equivalence needs an authored alias.
            if !matches!(
                (source, canonical.spelling().get().as_str()),
                ("ã", "ã") | ("ã", "ã")
            ) || !supported_unit(source, kind(canonical.kind()))
                || spellings.iter().any(|prior| prior.spelling == source)
            {
                return Err(InvalidProfile);
            }
            spellings.push(UnitSpelling {
                spelling: source,
                unit: index,
            });
        }
        Ok(Self {
            profile,
            spellings,
            unit_syntax,
        })
    }
    /// Delimiters are presentation only and are retained in original material.
    /// This operation creates no phoneme/phone references or speaking authority.
    pub fn parse(
        &self,
        original: alloc::string::String,
        display: SpeechIpaDisplayKind,
        provenance: SpeechEvidenceProvenance,
    ) -> Result<SpeechIpaProfileMatch, IpaNotationRefusal> {
        use IpaNotationRefusal::*;
        let (open, close) = match display {
            SpeechIpaDisplayKind::Phonemic => ('/', '/'),
            SpeechIpaDisplayKind::Phonetic => ('[', ']'),
        };
        if original.len() < 3 || !original.starts_with(open) || !original.ends_with(close) {
            return Err(DisplayDelimiters);
        }
        if original.len() > 4096 {
            return Err(Capacity);
        }
        let content = &original[1..original.len() - 1];
        let spans = partition(content, &self.spellings).map_err(|refusal| match refusal {
            PartitionRefusal::Ambiguous => AmbiguousTranscription,
            PartitionRefusal::Bound => Capacity,
            PartitionRefusal::InvalidProfile => InvalidProfile,
            _ => UnsupportedTranscription,
        })?;
        let mut previous_kind = None;
        let mut pending_stress = false;
        let mut occurrences = Vec::new();
        for span in spans {
            let unit = &self.profile.units()[span.unit];
            match kind(unit.kind()) {
                UnitKind::Segment => {
                    pending_stress = false;
                }
                UnitKind::PrimaryStress | UnitKind::SecondaryStress => {
                    if pending_stress {
                        return Err(SuprasegmentalOrder);
                    }
                    pending_stress = true;
                }
                UnitKind::Length => {
                    if previous_kind != Some(UnitKind::Segment) {
                        return Err(SuprasegmentalOrder);
                    }
                }
                UnitKind::SyllableBoundary => {
                    if pending_stress
                        || !matches!(previous_kind, Some(UnitKind::Segment | UnitKind::Length))
                    {
                        return Err(SuprasegmentalOrder);
                    }
                }
            }
            previous_kind = Some(kind(unit.kind()));
            occurrences.push(
                SpeechIpaUnitOccurrence::new(
                    (span.end + 1) as u64,
                    SpeechIpaSpelling::new(content[span.start..span.end].into()).map_err(Native)?,
                    (span.start + 1) as u64,
                    unit.identity().clone(),
                )
                .map_err(Native)?,
            );
        }
        if pending_stress
            || !matches!(
                occurrences
                    .last()
                    .and_then(|last| self
                        .profile
                        .units()
                        .as_slice()
                        .iter()
                        .find(|unit| unit.identity() == last.unit()))
                    .map(|unit| kind(unit.kind())),
                Some(UnitKind::Segment | UnitKind::Length)
            )
        {
            return Err(SuprasegmentalOrder);
        }
        let transcription = SpeechIpaTranscription::new(
            display,
            self.profile.inventory_id().clone(),
            original,
            self.profile.identity().clone(),
            self.profile.revision().clone(),
            provenance,
            BoundedSequence::try_from_iter(occurrences).map_err(|_| Capacity)?,
            self.profile.variety().clone(),
        )
        .map_err(Native)?;
        SpeechIpaProfileMatch::new(self.profile.clone(), transcription).map_err(Native)
    }
}
