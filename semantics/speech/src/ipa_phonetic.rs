//! Universal phonetic transcription preparation: no inventory or variety basis.
use crate::{
    ipa_diagnostic::{IpaNotationDiagnostic, IpaSourceSpan},
    ipa_notation::IpaNotationRefusal,
    ipa_order::NotationOrder,
    ipa_partition::{partition_located, PartitionRefusal, UnitSpelling},
    ipa_phone::phone_from_ipa,
    ipa_unicode::{UnitKind, SUPPORTED_SEGMENTS},
    semantic::*,
};
use alloc::{string::String, vec::Vec};
use conduit_plot::rust_binding::BoundedSequence;

pub const PHONETIC_GRAMMAR_REVISION: &str = "speech/ipa/version1";

/// Executed syntax admission, distinct from an authored candidate record.
/// Phones are universal; no phoneme, language membership or realization is inferred.
pub struct AdmittedPhoneticTranscription {
    transcription: SpeechPhoneticTranscription,
}
impl AdmittedPhoneticTranscription {
    pub fn transcription(&self) -> &SpeechPhoneticTranscription {
        &self.transcription
    }
}

/// Qualified quoted-IPA entrance. `original` is the body, without display
/// brackets. Source spelling is retained exactly; both nasal spellings remain
/// distinct and no inventory or normalization equivalence is synthesized.
pub fn phonetic_from_ipa(
    original: String,
    provenance: SpeechEvidenceProvenance,
) -> Result<AdmittedPhoneticTranscription, IpaNotationDiagnostic> {
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
    let units: Vec<_> = SUPPORTED_SEGMENTS
        .iter()
        .map(|&spelling| (spelling, UnitKind::Segment))
        .chain([
            ("ˈ", UnitKind::PrimaryStress),
            ("ˌ", UnitKind::SecondaryStress),
            ("ː", UnitKind::Length),
            (".", UnitKind::SyllableBoundary),
        ])
        .collect();
    let spellings: Vec<_> = units
        .iter()
        .enumerate()
        .map(|(unit, (spelling, _))| UnitSpelling { spelling, unit })
        .collect();
    let spans = partition_located(&original, &spellings).map_err(|error| {
        let refusal = match error.refusal {
            PartitionRefusal::Ambiguous => AmbiguousTranscription,
            PartitionRefusal::Bound => Capacity,
            PartitionRefusal::InvalidProfile => InvalidProfile,
            _ => UnsupportedTranscription,
        };
        at(error.start, error.end, refusal)
    })?;
    let mut order = NotationOrder::default();
    let mut occurrences = Vec::new();
    let mut scalar_start = 0_u64;
    let mut last_span = (0, 0);
    for span in spans {
        last_span = (span.start, span.end);
        let (spelling, kind) = units[span.unit];
        order
            .admit(kind)
            .map_err(|()| at(span.start, span.end, SuprasegmentalOrder))?;
        let event = match kind {
            UnitKind::Segment => {
                let phone = phone_from_ipa(spelling.into(), provenance.clone())
                    .map_err(|refusal| at(span.start, span.end, refusal))?;
                SpeechPhoneticEvent::phone(phone.notation().clone()).map_err(native)?
            }
            UnitKind::PrimaryStress => SpeechPhoneticEvent::PrimaryStress,
            UnitKind::SecondaryStress => SpeechPhoneticEvent::SecondaryStress,
            UnitKind::Length => SpeechPhoneticEvent::Length,
            UnitKind::SyllableBoundary => SpeechPhoneticEvent::SyllableBoundary,
        };
        let scalar_end = scalar_start + spelling.chars().count() as u64;
        occurrences.push(
            SpeechPhoneticOccurrence::new(
                span.end as u64,
                scalar_end,
                scalar_start,
                SpeechIpaSpelling::new(spelling.into()).map_err(native)?,
                span.start as u64,
                event,
            )
            .map_err(native)?,
        );
        scalar_start = scalar_end;
    }
    if !order.is_complete() {
        return Err(at(last_span.0, last_span.1, SuprasegmentalOrder));
    }
    let transcription = SpeechPhoneticTranscription::new(
        PHONETIC_GRAMMAR_REVISION.into(),
        original,
        provenance,
        BoundedSequence::try_from_iter(occurrences).map_err(|_| IpaNotationDiagnostic {
            refusal: Capacity,
            span: whole,
        })?,
    )
    .map_err(native)?;
    Ok(AdmittedPhoneticTranscription { transcription })
}
