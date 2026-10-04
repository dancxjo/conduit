//! Resolve exact references within immutable caller-supplied material snapshots.
//! Conduit laws own basis and ordinal checks. Traversal/indexing is mechanical;
//! no phonetic specification, observation, source or commitment is inferred.
use crate::semantic::*;
use conduit_plot::rust_binding::NativeBindingRefusal;

#[derive(Debug)]
pub enum ReferenceRefusal {
    ReferenceKind,
    Native(NativeBindingRefusal),
    Basis(NativeBindingRefusal),
    Ordinal(NativeBindingRefusal),
    Representation,
}

fn check(
    basis: &SpeechTokenSequenceBasis,
    reference: LanguageSpeechTokenRef,
    count: usize,
) -> Result<SpeechSequenceReferenceMatch, ReferenceRefusal> {
    let count = u32::try_from(count).map_err(|_| ReferenceRefusal::Representation)?;
    let basis_match =
        SpeechSequenceBasisMatch::new(basis.clone(), reference).map_err(ReferenceRefusal::Basis)?;
    SpeechSequenceReferenceMatch::new(basis_match, count).map_err(ReferenceRefusal::Ordinal)
}

/// This binds the reference to this exact borrowed snapshot, not a global store.
pub struct ResolvedPhone<'a> {
    reference: &'a LanguageSegmentRef,
    snapshot: &'a SpeechPhoneSequence,
    token: &'a SpeechPhoneToken,
    checked: SpeechSequenceReferenceMatch,
}
impl<'a> ResolvedPhone<'a> {
    pub fn reference(&self) -> &'a LanguageSegmentRef {
        self.reference
    }
    pub fn snapshot(&self) -> &'a SpeechPhoneSequence {
        self.snapshot
    }
    pub fn token(&self) -> &'a SpeechPhoneToken {
        self.token
    }
    pub fn checked(&self) -> &SpeechSequenceReferenceMatch {
        &self.checked
    }
}
pub fn resolve_phone<'a>(
    reference: &'a LanguageSegmentRef,
    snapshot: &'a SpeechPhoneSequence,
) -> Result<ResolvedPhone<'a>, ReferenceRefusal> {
    let LanguageSegmentRef::Phone(value) = reference else {
        return Err(ReferenceRefusal::ReferenceKind);
    };
    let projected = LanguageSpeechTokenRef::new(
        value.inventory_id().clone(),
        value.language().clone(),
        *value.ordinal(),
        value.revision_id().clone(),
        value.sequence_id().clone(),
        value.utterance_id().clone(),
    )
    .map_err(ReferenceRefusal::Native)?;
    let checked = check(
        snapshot.basis(),
        projected,
        snapshot.tokens().as_slice().len(),
    )?;
    let ordinal = usize::try_from(*checked.basis_match().reference().ordinal())
        .map_err(|_| ReferenceRefusal::Representation)?;
    let token = snapshot
        .tokens()
        .as_slice()
        .get(ordinal)
        .ok_or(ReferenceRefusal::Representation)?;
    Ok(ResolvedPhone {
        reference,
        snapshot,
        token,
        checked,
    })
}

pub struct ResolvedPhoneme<'a> {
    reference: &'a LanguageSegmentRef,
    snapshot: &'a SpeechPhonemeSequence,
    token: &'a SpeechPhonemeToken,
    checked: SpeechSequenceReferenceMatch,
}
impl<'a> ResolvedPhoneme<'a> {
    pub fn reference(&self) -> &'a LanguageSegmentRef {
        self.reference
    }
    pub fn snapshot(&self) -> &'a SpeechPhonemeSequence {
        self.snapshot
    }
    pub fn token(&self) -> &'a SpeechPhonemeToken {
        self.token
    }
    pub fn checked(&self) -> &SpeechSequenceReferenceMatch {
        &self.checked
    }
}
pub fn resolve_phoneme<'a>(
    reference: &'a LanguageSegmentRef,
    snapshot: &'a SpeechPhonemeSequence,
) -> Result<ResolvedPhoneme<'a>, ReferenceRefusal> {
    let LanguageSegmentRef::Phoneme(value) = reference else {
        return Err(ReferenceRefusal::ReferenceKind);
    };
    let projected = LanguageSpeechTokenRef::new(
        value.inventory_id().clone(),
        value.language().clone(),
        *value.ordinal(),
        value.revision_id().clone(),
        value.sequence_id().clone(),
        value.utterance_id().clone(),
    )
    .map_err(ReferenceRefusal::Native)?;
    let checked = check(
        snapshot.basis(),
        projected,
        snapshot.tokens().as_slice().len(),
    )?;
    let ordinal = usize::try_from(*checked.basis_match().reference().ordinal())
        .map_err(|_| ReferenceRefusal::Representation)?;
    let token = snapshot
        .tokens()
        .as_slice()
        .get(ordinal)
        .ok_or(ReferenceRefusal::Representation)?;
    Ok(ResolvedPhoneme {
        reference,
        snapshot,
        token,
        checked,
    })
}
