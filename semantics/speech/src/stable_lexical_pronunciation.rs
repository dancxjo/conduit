//! Source pronunciation lookup retaining the independent admitted lexical fact.
use crate::semantic::*;
use conduit_language::stable_lexical_selection::PreparedStableLexicalSelection;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum StableLexicalPronunciationRefusal {
    Native(NativeBindingRefusal),
    Program,
    Unresolved,
}

pub struct PreparedStableLexicalPronunciation<'a, 'fact> {
    selection: &'a PreparedStableLexicalSelection<'fact>,
    request: SpeechPronunciationRequest,
    result: SpeechPronunciationResult,
    row_selection: SpeechPronunciationRowIndex,
}
impl<'a, 'fact> PreparedStableLexicalPronunciation<'a, 'fact> {
    pub fn selection(&self) -> &'a PreparedStableLexicalSelection<'fact> {
        self.selection
    }
    pub fn request(&self) -> &SpeechPronunciationRequest {
        &self.request
    }
    pub fn result(&self) -> &SpeechPronunciationResult {
        &self.result
    }
    pub fn row_selection(&self) -> &SpeechPronunciationRowIndex {
        &self.row_selection
    }
}

/// Uses the same authored pronunciation lookup as dependency-based preparation.
/// Retaining lexical truth does not authorize playback or synthesize an arc.
pub fn prepare_stable_lexical_pronunciation<'a, 'fact>(
    selection: &'a PreparedStableLexicalSelection<'fact>,
    profile: &SpeechPronunciationProfile,
) -> Result<PreparedStableLexicalPronunciation<'a, 'fact>, StableLexicalPronunciationRefusal> {
    use StableLexicalPronunciationRefusal::*;
    let request = SpeechPronunciationRequest::new(
        selection.candidate().clone(),
        selection
            .lexical()
            .tape()
            .source()
            .material()
            .language()
            .clone(),
        profile.clone(),
    )
    .map_err(Native)?;
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
        concat!(env!("OUT_DIR"), "/pronunciation_program.hex")
    ))
    .map_err(|_| Program)?;
    let query = SpeechPronunciationQuery::new(
        request.candidate().clone(),
        request.profile().rows().clone(),
    )
    .map_err(Native)?;
    let output = program
        .evaluate(&query.encode().map_err(Native)?)
        .map_err(|_| Program)?;
    let row_selection = SpeechPronunciationRowIndex::decode(&output).map_err(Native)?;
    let row = request
        .profile()
        .rows()
        .iter()
        .nth(*row_selection.index() as usize)
        .ok_or(Unresolved)?;
    let result = SpeechPronunciationResult::new(true, row.phones().clone()).map_err(Native)?;
    Ok(PreparedStableLexicalPronunciation {
        selection,
        request,
        result,
        row_selection,
    })
}
