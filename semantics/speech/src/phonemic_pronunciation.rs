//! Exact original lexical selection through finite supplied phonemic data.
//! Known phones are not reinterpreted; contextual realization remains separate.
use crate::{
    common_acoustic_quantities::{
        execute, SpeechCommonAcousticExecution, SpeechCommonAcousticRefusal,
    },
    semantic::*,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_language::pronunciation_selection::PreparedPronunciationSelection;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
const PROGRAM: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/phonemic_pronunciation_program.hex"
));
#[derive(Debug)]
pub enum PhonemicPronunciationRefusal {
    Native(NativeBindingRefusal),
    Source(SpeechCommonAcousticRefusal),
    Unresolved(Box<RejectedPhonemicPronunciation>),
}
pub struct RejectedPhonemicPronunciation {
    request: SpeechPhonemicPronunciationRequest,
    query: SpeechPhonemicPronunciationQuery,
    executions: Vec<SpeechCommonAcousticExecution>,
    index: SpeechPhonemicPronunciationRowIndex,
}
impl core::fmt::Debug for RejectedPhonemicPronunciation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RejectedPhonemicPronunciation")
            .field("request", &self.request)
            .field("index", &self.index)
            .finish()
    }
}
impl RejectedPhonemicPronunciation {
    pub fn request(&self) -> &SpeechPhonemicPronunciationRequest {
        &self.request
    }
    pub fn query(&self) -> &SpeechPhonemicPronunciationQuery {
        &self.query
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn index(&self) -> &SpeechPhonemicPronunciationRowIndex {
        &self.index
    }
}
pub struct PreparedPhonemicPronunciation<'a> {
    selection: &'a PreparedPronunciationSelection,
    profile: &'a SpeechPhonemicPronunciationProfile,
    request: SpeechPhonemicPronunciationRequest,
    request_frame: Vec<u8>,
    query: SpeechPhonemicPronunciationQuery,
    executions: Vec<SpeechCommonAcousticExecution>,
    index: SpeechPhonemicPronunciationRowIndex,
    row: &'a SpeechPhonemicPronunciationRow,
    result: SpeechPhonemicPronunciationResult,
    admitted: Vec<u8>,
}
impl<'a> PreparedPhonemicPronunciation<'a> {
    pub fn selection(&self) -> &'a PreparedPronunciationSelection {
        self.selection
    }
    pub fn profile(&self) -> &'a SpeechPhonemicPronunciationProfile {
        self.profile
    }
    pub fn request(&self) -> &SpeechPhonemicPronunciationRequest {
        &self.request
    }
    pub fn request_canonical(&self) -> &[u8] {
        &self.request_frame
    }
    pub fn query(&self) -> &SpeechPhonemicPronunciationQuery {
        &self.query
    }
    pub fn executions(&self) -> &[SpeechCommonAcousticExecution] {
        &self.executions
    }
    pub fn index(&self) -> &SpeechPhonemicPronunciationRowIndex {
        &self.index
    }
    pub fn row(&self) -> &'a SpeechPhonemicPronunciationRow {
        self.row
    }
    pub fn result(&self) -> &SpeechPhonemicPronunciationResult {
        &self.result
    }
    pub fn admitted_canonical(&self) -> &[u8] {
        &self.admitted
    }
}
pub fn prepare_phonemic_pronunciation<'a>(
    selection: &'a PreparedPronunciationSelection,
    profile: &'a SpeechPhonemicPronunciationProfile,
) -> Result<PreparedPhonemicPronunciation<'a>, PhonemicPronunciationRefusal> {
    use PhonemicPronunciationRefusal::*;
    let request = SpeechPhonemicPronunciationRequest::new(
        selection.candidate().clone(),
        selection.request().source().material().language().clone(),
        profile.clone(),
    )
    .map_err(Native)?;
    let request_frame = request.clone().encode().map_err(Native)?;
    let query = SpeechPhonemicPronunciationQuery::new(
        request.candidate().clone(),
        request.profile().rows().clone(),
    )
    .map_err(Native)?;
    let mut executions = Vec::new();
    let output = execute(PROGRAM, query.clone(), &mut executions).map_err(Source)?;
    let index = SpeechPhonemicPronunciationRowIndex::decode(&output).map_err(Native)?;
    let row_index = usize::try_from(*index.index()).ok();
    let Some(row) = row_index.and_then(|i| profile.rows().as_slice().get(i)) else {
        return Err(Unresolved(Box::new(RejectedPhonemicPronunciation {
            request,
            query,
            executions,
            index,
        })));
    };
    let result =
        SpeechPhonemicPronunciationResult::new(true, row.phonemes().clone()).map_err(Native)?;
    let admitted = result.clone().encode().map_err(Native)?;
    Ok(PreparedPhonemicPronunciation {
        selection,
        profile,
        request,
        request_frame,
        query,
        executions,
        index,
        row,
        result,
        admitted,
    })
}
