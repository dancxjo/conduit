//! Exact selected lexical meaning maps through checked supplied pronunciation data.
//! Unknown and ambiguous rows refuse; no word, POS or language heuristic executes.
use crate::semantic::*;
use conduit_language::pronunciation_selection::PreparedPronunciationSelection;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum PronunciationRefusal {
    Native(NativeBindingRefusal),
    Program,
    Unresolved(alloc::boxed::Box<RejectedPronunciation>),
}
#[derive(Debug)]
pub struct RejectedPronunciation {
    request: SpeechPronunciationRequest,
    selection: SpeechPronunciationRowIndex,
}
impl RejectedPronunciation {
    pub fn request(&self) -> &SpeechPronunciationRequest {
        &self.request
    }
    pub fn selection(&self) -> &SpeechPronunciationRowIndex {
        &self.selection
    }
}
pub struct PreparedPronunciation<'a> {
    selection: &'a PreparedPronunciationSelection,
    request: SpeechPronunciationRequest,
    result: SpeechPronunciationResult,
    row_selection: SpeechPronunciationRowIndex,
}
impl<'a> PreparedPronunciation<'a> {
    pub fn selection(&self) -> &'a PreparedPronunciationSelection {
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
pub fn prepare_pronunciation<'a>(
    selection: &'a PreparedPronunciationSelection,
    profile: &SpeechPronunciationProfile,
) -> Result<PreparedPronunciation<'a>, PronunciationRefusal> {
    use PronunciationRefusal::*;
    let request = SpeechPronunciationRequest::new(
        selection.candidate().clone(),
        selection.request().source().material().language().clone(),
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
    let bytes = program
        .evaluate(&query.encode().map_err(Native)?)
        .map_err(|_| Program)?;
    let index = SpeechPronunciationRowIndex::decode(&bytes).map_err(Native)?;
    let Some(row) = request.profile().rows().iter().nth(*index.index() as usize) else {
        return Err(Unresolved(alloc::boxed::Box::new(RejectedPronunciation {
            request,
            selection: index,
        })));
    };
    let result = SpeechPronunciationResult::new(true, row.phones().clone()).map_err(Native)?;
    Ok(PreparedPronunciation {
        selection,
        request,
        result,
        row_selection: index,
    })
}
