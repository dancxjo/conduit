//! Inventory-independent, finite phonetic notation preparation.
use crate::{
    ipa_notation::IpaNotationRefusal,
    ipa_partition::{partition_located, PartitionRefusal, UnitSpelling},
    ipa_unicode::{supported_unit, UnitKind},
    semantic::*,
};
use alloc::{string::String, vec::Vec};
use conduit_plot::rust_binding::BoundedSequence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpaSourceSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub scalar_start: usize,
    pub scalar_end: usize,
}
impl IpaSourceSpan {
    pub(crate) fn new(text: &str, start: usize, end: usize) -> Self {
        Self {
            byte_start: start,
            byte_end: end,
            scalar_start: text[..start].chars().count(),
            scalar_end: text[..end].chars().count(),
        }
    }
}
#[derive(Debug)]
pub struct LocatedIpaRefusal {
    pub reason: IpaNotationRefusal,
    pub span: IpaSourceSpan,
}
fn unit_kind(kind: &SpeechIpaUnitKind) -> UnitKind {
    match kind {
        SpeechIpaUnitKind::Segment => UnitKind::Segment,
        SpeechIpaUnitKind::PrimaryStress => UnitKind::PrimaryStress,
        SpeechIpaUnitKind::SecondaryStress => UnitKind::SecondaryStress,
        SpeechIpaUnitKind::Length => UnitKind::Length,
        SpeechIpaUnitKind::SyllableBoundary => UnitKind::SyllableBoundary,
    }
}
pub struct PreparedPhoneticIpaProfile<'a> {
    profile: &'a SpeechPhoneticIpaProfile,
    spellings: Vec<UnitSpelling<'a>>,
}
impl<'a> PreparedPhoneticIpaProfile<'a> {
    pub fn prepare(profile: &'a SpeechPhoneticIpaProfile) -> Result<Self, IpaNotationRefusal> {
        use IpaNotationRefusal::*;
        let mut spellings = Vec::new();
        for (index, unit) in profile.units().iter().enumerate() {
            if profile.units().as_slice()[..index].iter().any(|prior| {
                prior.identity() == unit.identity() || prior.spelling() == unit.spelling()
            }) {
                return Err(InvalidProfile);
            }
            if !supported_unit(unit.spelling().get(), unit_kind(unit.kind())) {
                return Err(UnsupportedSpelling);
            }
            SpeechIpaUnitSyntaxAdmission::new(unit.clone()).map_err(Native)?;
            spellings.push(UnitSpelling {
                spelling: unit.spelling().get(),
                unit: index,
            });
        }
        for alias in profile.aliases() {
            let index = profile
                .units()
                .iter()
                .position(|unit| unit.identity() == alias.target())
                .ok_or(InvalidProfile)?;
            let canonical = &profile.units()[index];
            let source = alias.source().get().as_str();
            if !matches!(
                (source, canonical.spelling().get().as_str()),
                ("ã", "ã") | ("ã", "ã")
            ) || spellings.iter().any(|item| item.spelling == source)
            {
                return Err(InvalidProfile);
            }
            spellings.push(UnitSpelling {
                spelling: source,
                unit: index,
            });
        }
        Ok(Self { profile, spellings })
    }
    pub fn profile(&self) -> &SpeechPhoneticIpaProfile {
        self.profile
    }
    /// Accepts whole un-delimited quoted text; delimiters grant no identity.
    pub fn phonetic_from_ipa(
        &self,
        original: String,
        provenance: SpeechEvidenceProvenance,
    ) -> Result<AdmittedPhoneticIpaTranscription, LocatedIpaRefusal> {
        use IpaNotationRefusal::*;
        let refusal = |reason, start, end| LocatedIpaRefusal {
            reason,
            span: IpaSourceSpan::new(&original, start, end),
        };
        let spans =
            partition_located(&original, &self.spellings).map_err(|(error, start, end)| {
                let reason = match error {
                    PartitionRefusal::Ambiguous => AmbiguousTranscription,
                    PartitionRefusal::Bound => Capacity,
                    PartitionRefusal::InvalidProfile => InvalidProfile,
                    _ => UnsupportedTranscription,
                };
                refusal(reason, start, end)
            })?;
        let mut previous = None;
        let mut stress = false;
        let mut occurrences = Vec::new();
        for span in spans {
            let unit = &self.profile.units()[span.unit];
            let kind = unit_kind(unit.kind());
            let invalid = match kind {
                UnitKind::Segment => {
                    stress = false;
                    false
                }
                UnitKind::PrimaryStress | UnitKind::SecondaryStress => {
                    let prior = stress;
                    stress = true;
                    prior
                }
                UnitKind::Length => previous != Some(UnitKind::Segment),
                UnitKind::SyllableBoundary => {
                    stress || !matches!(previous, Some(UnitKind::Segment | UnitKind::Length))
                }
            };
            if invalid {
                return Err(refusal(SuprasegmentalOrder, span.start, span.end));
            }
            previous = Some(kind);
            occurrences.push(
                SpeechIpaUnitOccurrence::new(
                    span.end as u64,
                    SpeechIpaSpelling::new(original[span.start..span.end].into())
                        .map_err(|e| refusal(Native(e), span.start, span.end))?,
                    span.start as u64,
                    unit.identity().clone(),
                )
                .map_err(|e| refusal(Native(e), span.start, span.end))?,
            );
        }
        if stress || !matches!(previous, Some(UnitKind::Segment | UnitKind::Length)) {
            let last = occurrences.last().expect("nonempty partition");
            return Err(refusal(
                SuprasegmentalOrder,
                *last.start() as usize,
                *last.end() as usize,
            ));
        }
        let units = BoundedSequence::try_from_iter(occurrences)
            .map_err(|_| refusal(Capacity, 0, original.len()))?;
        let checked = SpeechPhoneticIpaTranscription::new(
            original.clone(),
            self.profile.clone(),
            provenance,
            units,
        )
        .map_err(|e| refusal(Native(e), 0, original.len()))?;
        Ok(AdmittedPhoneticIpaTranscription { checked })
    }
}
pub struct AdmittedPhoneticIpaTranscription {
    checked: SpeechPhoneticIpaTranscription,
}
impl AdmittedPhoneticIpaTranscription {
    pub fn transcription(&self) -> &SpeechPhoneticIpaTranscription {
        &self.checked
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpaSpanRefusal {
    MissingOccurrence,
    InvalidExtent,
    SpellingMismatch,
}
/// Exact scalar positions derive from retained UTF-8, never segment ordinals.
/// This is location inspection, not a second notation-admission entrance.
pub fn phonetic_unit_source_span(
    value: &SpeechPhoneticIpaTranscription,
    index: usize,
) -> Result<IpaSourceSpan, IpaSpanRefusal> {
    let unit = value
        .units()
        .as_slice()
        .get(index)
        .ok_or(IpaSpanRefusal::MissingOccurrence)?;
    let start = usize::try_from(*unit.start()).map_err(|_| IpaSpanRefusal::InvalidExtent)?;
    let end = usize::try_from(*unit.end()).map_err(|_| IpaSpanRefusal::InvalidExtent)?;
    let original = value.original();
    let spelling = original
        .get(start..end)
        .ok_or(IpaSpanRefusal::InvalidExtent)?;
    if spelling != unit.source_spelling().get() {
        return Err(IpaSpanRefusal::SpellingMismatch);
    }
    Ok(IpaSourceSpan::new(original, start, end))
}
