//! Atomic ownership of the fixed Source feature ingress and original model Plan.
//! The public Session additionally owns revision, frontier and commitment policy.
use crate::{
    parser_session_canonical_ingress::{
        ParserCanonicalIngressRefusal, ParserCanonicalSourceExecutor,
        PreparedCanonicalParserSessionPort, PreparedParserExecutionFrames,
    },
    parser_session_numeric_custody::{
        ParserNumericExecutor, ParserNumericFrames, ParserNumericHistory, ParserNumericRefusal,
        PreparedParserNumericCustody,
    },
    LanguageParserV2ChoiceQuery, LanguageParserV2ModelFeatures,
};
use alloc::rc::Rc;

pub(crate) struct PreparedParserMixedCustody<
    S: ParserCanonicalSourceExecutor,
    N: ParserNumericExecutor,
> {
    source: PreparedCanonicalParserSessionPort<
        LanguageParserV2ChoiceQuery,
        LanguageParserV2ModelFeatures,
        S,
    >,
    numeric: PreparedParserNumericCustody<N>,
    original_source_plan: Rc<conduit_core::Plan>,
    cancelled: bool,
}
pub(crate) struct ParserMixedHistory {
    pub(crate) numeric: ParserNumericHistory,
    pub(crate) original_source_plan: Rc<conduit_core::Plan>,
}
#[derive(Debug)]
pub(crate) enum ParserMixedRefusal<S, N> {
    Cancelled,
    Source(ParserCanonicalIngressRefusal<S>),
    Numeric(ParserNumericRefusal<N>),
}
impl<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor> PreparedParserMixedCustody<S, N> {
    /// Only the fixed Session preparation owner assembles this after validating
    /// both complete original Plans and reserving every component plus frames.
    /// Both admitted execution owners exist before the first ingress can run.
    pub(crate) fn from_prepared(
        source: PreparedCanonicalParserSessionPort<
            LanguageParserV2ChoiceQuery,
            LanguageParserV2ModelFeatures,
            S,
        >,
        numeric: PreparedParserNumericCustody<N>,
        original_source_plan: Rc<conduit_core::Plan>,
    ) -> Self {
        Self {
            source,
            numeric,
            original_source_plan,
            cancelled: false,
        }
    }
    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
        self.source.cancel();
        self.numeric.cancel();
    }
    pub(crate) fn execute(
        &mut self,
        original_query: &[u8],
        source_frames: PreparedParserExecutionFrames,
        numeric_frames: ParserNumericFrames,
    ) -> Result<ParserMixedHistory, ParserMixedRefusal<S::Error, N::Error>> {
        if self.cancelled {
            return Err(ParserMixedRefusal::Cancelled);
        }
        // Arm before any ingress, including unwinding from either target. A
        // successful feature result remains provisional until the model result.
        struct Stage<'a, S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor> {
            owner: &'a mut PreparedParserMixedCustody<S, N>,
            published: bool,
        }
        impl<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor> Drop for Stage<'_, S, N> {
            fn drop(&mut self) {
                if !self.published {
                    self.owner.cancel();
                }
            }
        }
        let mut stage = Stage {
            owner: self,
            published: false,
        };
        let features = stage
            .owner
            .source
            .execute(original_query, source_frames)
            .map_err(ParserMixedRefusal::Source)?;
        let numeric = stage
            .owner
            .numeric
            .execute(features, numeric_frames)
            .map_err(ParserMixedRefusal::Numeric)?;
        let history = ParserMixedHistory {
            numeric,
            original_source_plan: stage.owner.original_source_plan.clone(),
        };
        stage.published = true;
        Ok(history)
    }
}
impl<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor> Drop
    for PreparedParserMixedCustody<S, N>
{
    fn drop(&mut self) {
        self.cancel();
    }
}

impl ParserMixedHistory {
    /// Plans are the original immutable preparation owners, retained in full.
    /// This comparison allocates nothing; separately reserved Source/model frame
    /// replay and full generated Native admission follow before a view is returned.
    pub(crate) fn replay_and_readmit<'a>(
        &self,
        feature_verifier: &mut crate::parser_session_execution::verification::PreparedSourceVerification,
        projector: &mut crate::parser_session_execution::verification::PreparedSourceVerification,
        wrapper: &mut crate::parser_session_execution::verification::PreparedSourceVerification,
        numerical: &mut conduit_ai::integer_categorical_step::PreparedCategoricalCanonicalAdmission,
        family: &mut conduit_plot::rust_binding::PreparedNativeFamily,
        expected_source_plan: &conduit_core::Plan,
        expected_numeric_plan: &conduit_core::Plan,
        budget: &'a mut crate::parser_session_numeric_custody::ParserNumericReadmissionBudget,
    ) -> Result<
        crate::parser_session_numeric_custody::ParserNumericReadmission<'a>,
        ParserNumericRefusal<core::convert::Infallible>,
    > {
        if self.original_source_plan.as_ref() != expected_source_plan {
            return Err(ParserNumericRefusal::Parent);
        }
        self.numeric.replay_and_readmit(
            feature_verifier,
            projector,
            wrapper,
            numerical,
            family,
            expected_numeric_plan,
            budget,
        )
    }
}
