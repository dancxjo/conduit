//! Complete finite authored IPA admission, independent of terminal rendering.
use crate::{
    ipa_inventory::PreparedIpaInventory,
    ipa_notation::IpaNotationRefusal,
    ipa_phonetic::{IpaSourceSpan, LocatedIpaRefusal, PreparedPhoneticIpaProfile},
    semantic::*,
};
use alloc::{vec, vec::Vec};
use conduit_plot::rust_binding::BoundedSequence;

pub fn located_refusal(
    reason: SpeechIpaRefusalReason,
    span: IpaSourceSpan,
) -> SpeechIpaLocatedRefusal {
    SpeechIpaLocatedRefusal::new(
        span.byte_end as u64,
        span.byte_start as u64,
        reason,
        span.scalar_end as u64,
        span.scalar_start as u64,
    )
    .expect("ordered exact source extent")
}
fn notation_reason(reason: IpaNotationRefusal) -> SpeechIpaRefusalReason {
    match reason {
        IpaNotationRefusal::AmbiguousTranscription => SpeechIpaRefusalReason::Ambiguous,
        IpaNotationRefusal::Capacity => SpeechIpaRefusalReason::Capacity,
        IpaNotationRefusal::SuprasegmentalOrder => SpeechIpaRefusalReason::Boundary,
        IpaNotationRefusal::InvalidProfile => SpeechIpaRefusalReason::Profile,
        _ => SpeechIpaRefusalReason::Unsupported,
    }
}
fn located(error: LocatedIpaRefusal) -> SpeechIpaLocatedRefusal {
    located_refusal(notation_reason(error.reason), error.span)
}
fn whole(text: &str, reason: SpeechIpaRefusalReason) -> SpeechIpaLocatedRefusal {
    located_refusal(reason, IpaSourceSpan::new(text, 0, text.len()))
}

pub fn phonetic_from_ipa(request: &SpeechPhoneticIpaRequest) -> SpeechPhoneticIpaOutcome {
    let profile = match PreparedPhoneticIpaProfile::prepare(request.profile()) {
        Ok(profile) => profile,
        Err(error) => return phonetic_refusal(whole(request.original(), notation_reason(error))),
    };
    match profile.phonetic_from_ipa(request.original().clone(), request.provenance().clone()) {
        Ok(admitted) => {
            let value = admitted.transcription();
            SpeechPhoneticIpaOutcome::admitted(
                value.original().clone(),
                value.profile().clone(),
                value.provenance().clone(),
                value.units().clone(),
            )
            .expect("already admitted Native transcription")
        }
        Err(error) => phonetic_refusal(located(error)),
    }
}

/// A single phonetic phone retains unknown identity; spelling does not mint one.
pub fn phone_from_ipa(
    request: &SpeechPhoneticIpaRequest,
) -> Result<SpeechPhoneticIpaPhone, SpeechIpaLocatedRefusal> {
    let transcription = match phonetic_from_ipa(request) {
        SpeechPhoneticIpaOutcome::Admitted(value) => SpeechPhoneticIpaTranscription::new(
            value.original().clone(),
            value.profile().clone(),
            value.provenance().clone(),
            value.units().clone(),
        )
        .expect("admitted transcription fields"),
        SpeechPhoneticIpaOutcome::Refused(reason) => {
            return Err(SpeechIpaLocatedRefusal::new(
                *reason.byte_end(),
                *reason.byte_start(),
                *reason.reason(),
                *reason.scalar_end(),
                *reason.scalar_start(),
            )
            .expect("checked refusal extent"))
        }
    };
    if transcription.units().len() != 1
        || !transcription.profile().units().iter().any(|unit| {
            unit.identity() == transcription.units()[0].unit()
                && *unit.kind() == SpeechIpaUnitKind::Segment
        })
    {
        return Err(whole(
            request.original(),
            SpeechIpaRefusalReason::Membership,
        ));
    }
    SpeechPhoneticIpaPhone::new(PhoneSpecification::unknown(), transcription)
        .map_err(|_| whole(request.original(), SpeechIpaRefusalReason::Malformed))
}

pub fn phonemic_from_ipa(
    request: &SpeechPhonemicIpaRequest,
    inventory: &SpeechInventory,
) -> SpeechPhonemicIpaOutcome {
    match admit_phonemic(request, inventory) {
        Ok(value) => SpeechPhonemicIpaOutcome::admitted(
            value.bindings().clone(),
            value.inventory_id().clone(),
            value.notation().clone(),
            value.phonemes().clone(),
        )
        .expect("already checked phonemic transcription"),
        Err(reason) => SpeechPhonemicIpaOutcome::refused(
            *reason.byte_end(),
            *reason.byte_start(),
            *reason.reason(),
            *reason.scalar_end(),
            *reason.scalar_start(),
        )
        .expect("checked refusal extent"),
    }
}
fn admit_phonemic(
    request: &SpeechPhonemicIpaRequest,
    inventory: &SpeechInventory,
) -> Result<SpeechPhonemicIpaTranscription, SpeechIpaLocatedRefusal> {
    let text = request.original();
    let reject = |reason| whole(text, reason);
    let profile = request.profile();
    if inventory.identity() != profile.inventory_id()
        || inventory.identity() != request.inventory_id()
    {
        return Err(reject(SpeechIpaRefusalReason::Inventory));
    }
    if request.variety() != profile.variety()
        || inventory.language() != request.variety().language()
    {
        return Err(reject(SpeechIpaRefusalReason::Variety));
    }
    if request.revision() != profile.revision() {
        return Err(reject(SpeechIpaRefusalReason::Revision));
    }
    let _owner = PreparedIpaInventory::prepare(
        inventory,
        profile,
        request.variety(),
        request.revision(),
        request.phone_bindings().as_slice(),
        request.phoneme_bindings().as_slice(),
    )
    .map_err(|_| reject(SpeechIpaRefusalReason::Membership))?;
    // Removing domain facts creates an inventory-independent notation basis;
    // no placeholder language/inventory is supplied to phonetic preparation.
    let universal = SpeechPhoneticIpaProfile::new(
        profile.aliases().clone(),
        profile.identity().clone(),
        profile.provenance().clone(),
        profile.revision().clone(),
        profile.units().clone(),
    )
    .map_err(|_| reject(SpeechIpaRefusalReason::Profile))?;
    let parser = PreparedPhoneticIpaProfile::prepare(&universal)
        .map_err(|error| reject(notation_reason(error)))?;
    let parsed = parser
        .phonetic_from_ipa(text.clone(), request.provenance().clone())
        .map_err(located)?;
    let occurrences = parsed.transcription().units();
    let units: Vec<_> = occurrences
        .iter()
        .enumerate()
        .filter(|(_, occurrence)| {
            profile.units().iter().any(|unit| {
                unit.identity() == occurrence.unit()
                    && matches!(
                        unit.kind(),
                        SpeechIpaUnitKind::Segment | SpeechIpaUnitKind::Length
                    )
            })
        })
        .collect();
    let mut counts = vec![0u8; units.len() + 1];
    let mut next = vec![None; units.len() + 1];
    let mut blocked_boundary = None;
    counts[units.len()] = 1;
    for start in (0..units.len()).rev() {
        for (index, binding) in request.phoneme_bindings().iter().enumerate() {
            let n = binding.units().len();
            if start + n > units.len()
                || !binding
                    .units()
                    .iter()
                    .zip(&units[start..start + n])
                    .all(|(id, (_, occurrence))| id == occurrence.unit())
            {
                continue;
            }
            // Stress and explicit syllable boundaries remain barriers. A
            // multi-unit phoneme cannot consume units across an omitted event.
            if let Some(pair) = units[start..start + n]
                .windows(2)
                .find(|pair| pair[1].0 != pair[0].0 + 1)
            {
                let barrier = &occurrences[pair[0].0 + 1];
                blocked_boundary.get_or_insert(IpaSourceSpan::new(
                    text,
                    *barrier.start() as usize,
                    *barrier.end() as usize,
                ));
                continue;
            }
            let suffix = counts[start + n];
            if suffix > 0 {
                next[start] = Some((index, start + n));
                counts[start] = counts[start].saturating_add(suffix).min(2);
            }
        }
    }
    if counts[0] != 1 {
        if counts[0] == 0 {
            if let Some(span) = blocked_boundary {
                return Err(located_refusal(SpeechIpaRefusalReason::Membership, span));
            }
        }
        return Err(reject(if counts[0] == 0 {
            SpeechIpaRefusalReason::Membership
        } else {
            SpeechIpaRefusalReason::Ambiguous
        }));
    }
    let mut phonemes = Vec::new();
    let mut at = 0;
    while at < units.len() {
        let (index, end) = next[at].expect("unique checked partition");
        phonemes.push(request.phoneme_bindings()[index].phoneme().clone());
        at = end;
    }
    let transcription = SpeechIpaTranscription::new(
        SpeechIpaDisplayKind::Phonemic,
        profile.inventory_id().clone(),
        text.clone(),
        profile.identity().clone(),
        profile.revision().clone(),
        request.provenance().clone(),
        occurrences.clone(),
        request.variety().clone(),
    )
    .map_err(|_| reject(SpeechIpaRefusalReason::Malformed))?;
    let notation = SpeechIpaProfileMatch::new(profile.clone(), transcription)
        .map_err(|_| reject(SpeechIpaRefusalReason::Profile))?;
    SpeechPhonemicIpaTranscription::new(
        request.phoneme_bindings().clone(),
        inventory.identity().clone(),
        notation,
        BoundedSequence::try_from_iter(phonemes)
            .map_err(|_| reject(SpeechIpaRefusalReason::Capacity))?,
    )
    .map_err(|_| reject(SpeechIpaRefusalReason::Malformed))
}

fn phonetic_refusal(reason: SpeechIpaLocatedRefusal) -> SpeechPhoneticIpaOutcome {
    SpeechPhoneticIpaOutcome::refused(
        *reason.byte_end(),
        *reason.byte_start(),
        *reason.reason(),
        *reason.scalar_end(),
        *reason.scalar_start(),
    )
    .expect("checked refusal extent")
}
