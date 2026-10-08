//! Complete mixed-call frame reservation before the first backing allocation.
//! Frames move into retained original Source/model histories after execution.
use crate::{
    parser_session_canonical_ingress::{
        ParserCanonicalSourceExecutor, PreparedParserExecutionFrames,
    },
    parser_session_numeric_custody::{ParserNumericExecutor, ParserNumericFrames},
    parser_session_window8_stage::Window8RevisionStage,
};
use alloc::vec::Vec;
#[derive(Debug)]
pub(crate) struct ModelFrameRefusal;
pub(crate) struct Window8ModelFrameLimits {
    pub(crate) maximum_frame_bytes: usize,
    pub(crate) maximum_preparation_requested_bytes: usize,
    pub(crate) maximum_retained_bytes: usize,
}
pub(crate) fn prepare<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    limits: &Window8ModelFrameLimits,
) -> Result<(PreparedParserExecutionFrames, ParserNumericFrames), ModelFrameRefusal> {
    let result = (|| {
        let frame = limits.maximum_frame_bytes;
        if frame == 0 || frame > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return Err(ModelFrameRefusal);
        }
        let requested = frame.checked_mul(6).ok_or(ModelFrameRefusal)?;
        if requested > limits.maximum_preparation_requested_bytes
            || requested > limits.maximum_retained_bytes
        {
            return Err(ModelFrameRefusal);
        }
        stage
            .book()
            .can_model(requested)
            .map_err(|_| ModelFrameRefusal)?;
        let source =
            PreparedParserExecutionFrames::prepare(frame, frame).map_err(|_| ModelFrameRefusal)?;
        let buffer = || {
            let mut v = Vec::new();
            v.try_reserve_exact(frame).map_err(|_| ModelFrameRefusal)?;
            if v.capacity() > frame {
                return Err(ModelFrameRefusal);
            }
            v.resize(frame, 0);
            Ok(v)
        };
        let numeric = ParserNumericFrames {
            indices: buffer()?,
            scores: buffer()?,
            output: buffer()?,
            feature_guard: buffer()?,
        };
        let retained = numeric
            .retained_capacity_bytes()
            .and_then(|n| n.checked_add(source.retained_capacity_bytes()))
            .ok_or(ModelFrameRefusal)?;
        if retained > requested {
            return Err(ModelFrameRefusal);
        }
        Ok((source, numeric))
    })();
    if result.is_err() {
        stage.abort();
    }
    result
}
