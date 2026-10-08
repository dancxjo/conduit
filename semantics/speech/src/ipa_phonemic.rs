//! Phonemic transcription admission against one complete explicit inventory.
use crate::{
    ipa_diagnostic::IpaSourceSpan,
    ipa_inventory::{IpaInventoryRefusal, PreparedIpaInventory},
    ipa_notation::IpaNotationRefusal,
    ipa_phoneme_partition::{partition, Part},
    semantic::*,
};
use alloc::{format, string::String, vec::Vec};
use conduit_plot::rust_binding::BoundedSequence;

/// Locations refer to the quoted body, excluding generated display slashes.
#[derive(Debug)]
pub struct IpaPhonemicDiagnostic {
    pub refusal: IpaInventoryRefusal,
    pub span: IpaSourceSpan,
}

/// Executed membership receipt. Serializing the candidate transcription alone
/// does not prove resolution; this receipt retains the actual whole inventory.
pub struct AdmittedPhonemicTranscription<'a> {
    inventory: &'a SpeechInventory,
    transcription: SpeechPhonemicTranscription,
}
impl AdmittedPhonemicTranscription<'_> {
    pub fn inventory(&self) -> &SpeechInventory {
        self.inventory
    }
    pub fn transcription(&self) -> &SpeechPhonemicTranscription {
        &self.transcription
    }
    pub fn require_inventory(
        &self,
        inventory: &SpeechInventory,
    ) -> Result<(), IpaInventoryRefusal> {
        if self.inventory == inventory {
            Ok(())
        } else {
            Err(IpaInventoryRefusal::Basis)
        }
    }
}

impl<'a> PreparedIpaInventory<'a> {
    /// Every segment resolves to a declared phoneme. Length must be part of an
    /// explicit binding; it cannot manufacture an inventory contrast. Complete
    /// partitioning refuses competing definitions or segment groupings.
    pub fn phonemic_from_ipa(
        &self,
        original: String,
        provenance: SpeechEvidenceProvenance,
    ) -> Result<AdmittedPhonemicTranscription<'a>, IpaPhonemicDiagnostic> {
        use IpaInventoryRefusal::Notation;
        // The profile entrance retains display slashes within its 4096-byte
        // limit. Check before copying; over-budget input is never scanned whole.
        if original.len() > 4094 {
            let mut start = 4094;
            while !original.is_char_boundary(start) {
                start -= 1;
            }
            let end = start
                + original[start..]
                    .chars()
                    .next()
                    .expect("excess scalar")
                    .len_utf8();
            return Err(IpaPhonemicDiagnostic {
                refusal: Notation(IpaNotationRefusal::Capacity),
                span: IpaSourceSpan::from_bytes(&original, start, end).expect("UTF-8 excess"),
            });
        }
        let whole = IpaSourceSpan::from_bytes(&original, 0, original.len()).expect("whole UTF-8");
        let at = |start, end, refusal| IpaPhonemicDiagnostic {
            refusal,
            span: IpaSourceSpan::from_bytes(&original, start, end).expect("parser source boundary"),
        };
        let native = |error| IpaPhonemicDiagnostic {
            refusal: Notation(IpaNotationRefusal::Native(error)),
            span: whole,
        };
        let parsed = self
            .notation()
            .parse_located(
                format!("/{original}/"),
                SpeechIpaDisplayKind::Phonemic,
                provenance,
            )
            .map_err(|error| {
                at(
                    error.span.byte_start.saturating_sub(1).min(original.len()),
                    error.span.byte_end.saturating_sub(1).min(original.len()),
                    Notation(error.refusal),
                )
            })?;
        let units = parsed.transcription().units().as_slice();
        let bindings: Vec<_> = self.phonemes().iter().map(|(binding, _)| binding).collect();
        let parts = partition(units, self.notation().profile(), &bindings).map_err(|error| {
            at(
                *units[error.start].start() as usize - 1,
                *units[error.end - 1].end() as usize - 1,
                error.refusal,
            )
        })?;
        let mut occurrences = Vec::new();
        for (start, part) in parts {
            let (length, event) = match part {
                Part::Phoneme { binding, length } => (
                    length,
                    SpeechPhonemicEvent::phoneme(bindings[binding].phoneme().clone())
                        .map_err(native)?,
                ),
                Part::Mark => {
                    let kind = self
                        .notation()
                        .profile()
                        .units()
                        .as_slice()
                        .iter()
                        .find(|unit| unit.identity() == units[start].unit())
                        .expect("executed unit")
                        .kind();
                    (
                        1,
                        match kind {
                            SpeechIpaUnitKind::PrimaryStress => SpeechPhonemicEvent::PrimaryStress,
                            SpeechIpaUnitKind::SecondaryStress => {
                                SpeechPhonemicEvent::SecondaryStress
                            }
                            SpeechIpaUnitKind::SyllableBoundary => {
                                SpeechPhonemicEvent::SyllableBoundary
                            }
                            _ => unreachable!("partition admits only transcription marks"),
                        },
                    )
                }
            };
            let end = start + length;
            let first = &units[start];
            let last = &units[end - 1];
            let byte_start = *first.start() as usize - 1;
            let byte_end = *last.end() as usize - 1;
            occurrences.push(
                SpeechPhonemicOccurrence::new(
                    byte_end as u64,
                    last.scalar_end() - 1,
                    first.scalar_start() - 1,
                    SpeechIpaSpelling::new(original[byte_start..byte_end].into())
                        .map_err(native)?,
                    byte_start as u64,
                    end as u64,
                    start as u64,
                    event,
                )
                .map_err(native)?,
            );
        }
        let transcription = SpeechPhonemicTranscription::new(
            self.basis().clone(),
            BoundedSequence::try_from_iter(bindings.into_iter().cloned()).map_err(|_| {
                IpaPhonemicDiagnostic {
                    refusal: IpaInventoryRefusal::BindingCoverage,
                    span: whole,
                }
            })?,
            parsed.transcription().clone(),
            BoundedSequence::try_from_iter(occurrences).map_err(|_| IpaPhonemicDiagnostic {
                refusal: Notation(IpaNotationRefusal::Capacity),
                span: whole,
            })?,
            original,
        )
        .map_err(native)?;
        Ok(AdmittedPhonemicTranscription {
            inventory: self.inventory(),
            transcription,
        })
    }
}
