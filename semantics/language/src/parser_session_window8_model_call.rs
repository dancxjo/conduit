//! Actual mixed boundary from the original retained feature-context output.
//! All source/model frames remain owned by the unpublished revision book.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_model_frames::{self as model_frames, Window8ModelFrameLimits},
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_values::view,
};
#[derive(Debug)]
pub(crate) struct ModelCallRefusal;
pub(crate) fn execute<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    frames: &Window8ModelFrameLimits,
    context: usize,
    epoch: u64,
    model_calls: &mut u64,
) -> Result<usize, ModelCallRefusal> {
    let result = (|| {
        let next = model_calls.checked_add(1).ok_or(ModelCallRefusal)?;
        let parent = stage.book().source.get(context).ok_or(ModelCallRefusal)?;
        let query = queries
            .copy_record(
                "language-proposal-window8-v2-feature-values",
                view(parent.output_bytes()).map_err(|_| ModelCallRefusal)?,
            )
            .map_err(|_| ModelCallRefusal)?;
        let (source_frames, numeric_frames) =
            model_frames::prepare(stage, frames).map_err(|_| ModelCallRefusal)?;
        let history = stage
            .model(
                context,
                query,
                source_frames,
                numeric_frames,
                epoch,
                *model_calls,
            )
            .map_err(|_| ModelCallRefusal)?;
        *model_calls = next;
        Ok(history)
    })();
    if result.is_err() {
        stage.abort();
    }
    result
}
