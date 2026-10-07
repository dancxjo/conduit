//! Existing Source pronunciation preparation retaining independent protection custody.
use crate::{
    independent_token_role::PreparedIndependentTokenRole,
    lexical_pronunciation::{prepare_pronunciation, PreparedPronunciation, PronunciationRefusal},
    semantic::SpeechPronunciationProfile,
};
use conduit_language::{
    lexical::PreparedLexicalTape,
    pronunciation_selection::{
        prepare_pronunciation_selection, PreparedPronunciationSelection,
        PronunciationSelectionRefusal,
    },
    LanguagePronunciationSelectionProfile,
};

#[derive(Debug)]
pub enum IndependentPronunciationRefusal {
    LexicalTape,
    Dependent,
    Candidate,
    Selection(PronunciationSelectionRefusal),
    Pronunciation(PronunciationRefusal),
}

pub struct PreparedIndependentPronunciationSelection<'a, 'receipt> {
    role: &'a PreparedIndependentTokenRole<'receipt>,
    selection: PreparedPronunciationSelection,
}

impl<'a, 'receipt> PreparedIndependentPronunciationSelection<'a, 'receipt> {
    pub fn role(&self) -> &'a PreparedIndependentTokenRole<'receipt> {
        self.role
    }

    pub fn selection(&self) -> &PreparedPronunciationSelection {
        &self.selection
    }
}

/// The existing Language Plot selects pronunciation policy. Exact equality then
/// correlates its result with the candidate retained by the protected fact.
pub fn prepare_independent_pronunciation_selection<'a, 'receipt>(
    lexical: &PreparedLexicalTape,
    role: &'a PreparedIndependentTokenRole<'receipt>,
    profile: &LanguagePronunciationSelectionProfile,
) -> Result<PreparedIndependentPronunciationSelection<'a, 'receipt>, IndependentPronunciationRefusal>
{
    use IndependentPronunciationRefusal::*;
    let admission = role.protected().admission();
    let query = admission.fact().query();
    if lexical.tape() != query.beam().lexical().tape() {
        return Err(LexicalTape);
    }
    let dependent = usize::try_from(*query.dependent()).map_err(|_| Dependent)?;
    let token = lexical
        .tape()
        .tokens()
        .as_slice()
        .get(dependent)
        .ok_or(Dependent)?;
    let choice = usize::try_from(*role.role().request().choice()).map_err(|_| Candidate)?;
    let candidate = token.candidates().as_slice().get(choice).ok_or(Candidate)?;
    let selection = prepare_pronunciation_selection(
        lexical,
        dependent,
        query.beam().basis().analysis_revision(),
        admission.arc(),
        profile,
    )
    .map_err(Selection)?;
    if selection.candidate() != candidate {
        return Err(Candidate);
    }
    Ok(PreparedIndependentPronunciationSelection { role, selection })
}

pub struct PreparedIndependentPronunciation<'a, 'role, 'receipt> {
    selection: &'a PreparedIndependentPronunciationSelection<'role, 'receipt>,
    pronunciation: PreparedPronunciation<'a>,
}

impl<'a, 'role, 'receipt> PreparedIndependentPronunciation<'a, 'role, 'receipt> {
    pub fn selection(&self) -> &'a PreparedIndependentPronunciationSelection<'role, 'receipt> {
        self.selection
    }

    pub fn pronunciation(&self) -> &PreparedPronunciation<'a> {
        &self.pronunciation
    }
}

/// Retains the whole protection chain while using the existing Speech lookup.
/// Preparation does not grant playback or acknowledgment authority.
pub fn prepare_independent_pronunciation<'a, 'role, 'receipt>(
    selection: &'a PreparedIndependentPronunciationSelection<'role, 'receipt>,
    profile: &SpeechPronunciationProfile,
) -> Result<PreparedIndependentPronunciation<'a, 'role, 'receipt>, IndependentPronunciationRefusal>
{
    let pronunciation = prepare_pronunciation(selection.selection(), profile)
        .map_err(IndependentPronunciationRefusal::Pronunciation)?;
    Ok(PreparedIndependentPronunciation {
        selection,
        pronunciation,
    })
}
