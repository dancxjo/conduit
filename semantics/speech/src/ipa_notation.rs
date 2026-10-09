//! Declared IPA syntax with exact profile custody and original source spelling.
//! Preparation is an ordinary allocating path, not a bounded Flow execution.
use crate::{
    ipa_diagnostic::{IpaNotationDiagnostic, IpaSourceSpan},
    ipa_order::NotationOrder,
    ipa_partition::{partition_located, PartitionRefusal, UnitSpelling},
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
        self.parse_located(original, display, provenance)
            .map_err(|error| error.refusal)
    }

    /// Parse with locations in the exact original UTF-8 source. Byte and scalar
    /// coordinates include the authored display delimiters and never count phones.
    pub fn parse_located(
        &self,
        original: alloc::string::String,
        display: SpeechIpaDisplayKind,
        provenance: SpeechEvidenceProvenance,
    ) -> Result<SpeechIpaProfileMatch, IpaNotationDiagnostic> {
        use IpaNotationRefusal::*;
        if original.len() > 4096 {
            let mut start = 4096;
            while !original.is_char_boundary(start) {
                start -= 1;
            }
            let end = start
                + original[start..]
                    .chars()
                    .next()
                    .expect("excess scalar")
                    .len_utf8();
            return Err(IpaNotationDiagnostic {
                refusal: Capacity,
                span: IpaSourceSpan::from_bytes(&original, start, end)
                    .expect("first excess UTF-8 scalar"),
            });
        }
        let whole = IpaSourceSpan::from_bytes(&original, 0, original.len()).expect("whole UTF-8");
        let at = |start, end, refusal| IpaNotationDiagnostic {
            refusal,
            span: IpaSourceSpan::from_bytes(&original, start, end).expect("parser UTF-8 boundary"),
        };
        let native = |error| IpaNotationDiagnostic {
            refusal: Native(error),
            span: whole,
        };
        let (open, close) = match display {
            SpeechIpaDisplayKind::Phonemic => ('/', '/'),
            SpeechIpaDisplayKind::Phonetic => ('[', ']'),
        };
        if !original.starts_with(open) {
            return Err(at(
                0,
                original.chars().next().map_or(0, char::len_utf8),
                DisplayDelimiters,
            ));
        }
        if original.len() < 2 || !original.ends_with(close) {
            let start = original
                .char_indices()
                .last()
                .map_or(0, |(offset, _)| offset);
            return Err(at(start, original.len(), DisplayDelimiters));
        }
        let content = &original[1..original.len() - 1];
        let spans = partition_located(content, &self.spellings).map_err(|error| {
            let refusal = match error.refusal {
                PartitionRefusal::Ambiguous => AmbiguousTranscription,
                PartitionRefusal::Bound => Capacity,
                PartitionRefusal::InvalidProfile => InvalidProfile,
                _ => UnsupportedTranscription,
            };
            at(error.start + 1, error.end + 1, refusal)
        })?;
        let mut order = NotationOrder::default();
        let mut occurrences = Vec::new();
        let mut scalar_start = 1_u64;
        let mut last_span = (1, 1);
        for span in spans {
            last_span = (span.start + 1, span.end + 1);
            let unit = &self.profile.units()[span.unit];
            order
                .admit(kind(unit.kind()))
                .map_err(|()| at(span.start + 1, span.end + 1, SuprasegmentalOrder))?;
            let scalar_end = scalar_start + content[span.start..span.end].chars().count() as u64;
            occurrences.push(
                SpeechIpaUnitOccurrence::new(
                    (span.end + 1) as u64,
                    scalar_end,
                    scalar_start,
                    SpeechIpaSpelling::new(content[span.start..span.end].into()).map_err(native)?,
                    (span.start + 1) as u64,
                    unit.identity().clone(),
                )
                .map_err(native)?,
            );
            scalar_start = scalar_end;
        }
        if !order.is_complete() {
            return Err(at(last_span.0, last_span.1, SuprasegmentalOrder));
        }
        let transcription = SpeechIpaTranscription::new(
            display,
            self.profile.inventory_id().clone(),
            original,
            self.profile.identity().clone(),
            self.profile.revision().clone(),
            provenance,
            BoundedSequence::try_from_iter(occurrences).map_err(|_| IpaNotationDiagnostic {
                refusal: Capacity,
                span: whole,
            })?,
            self.profile.variety().clone(),
        )
        .map_err(native)?;
        SpeechIpaProfileMatch::new(self.profile.clone(), transcription).map_err(native)
    }
}
