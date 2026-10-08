//! Fixed profile mixed Source/model custody, preceding Session publication.
//! The Source projectors and original adopted model are retained independently;
//! a target response must equal their complete canonical composition.
use crate::{
    parser_canonical_history::ParserCanonicalHistory,
    parser_session_execution::{verification::PreparedSourceVerification, ParserSessionExecution},
    parser_session_numeric_profile::{FixedParserNumericProfile, PinnedFourSlotNumericProfile},
};
use alloc::{rc::Rc, sync::Arc, vec::Vec};
use conduit_ai::integer_categorical_step::{
    PreparedCategoricalCanonicalAdmission, PreparedCategoricalStep,
};
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};
use core::cell::RefCell;

/// Target metadata is checked by the fixed mixed Plan preparation boundary.
/// Its declared metadata does not itself prove ordinary target execution.
pub(crate) trait ParserNumericExecutor {
    type Error;
    fn plan(&self) -> &conduit_core::Plan;
    fn transact(
        &mut self,
        ordinal: u64,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error>;
    fn cancel(&mut self);
}

/// Every buffer is reserved by the owning finite Session before ingress. No
/// runtime replenishment is possible; a receipt permanently takes these owners.
pub(crate) struct ParserNumericFrames {
    pub indices: Vec<u8>,
    pub scores: Vec<u8>,
    pub output: Vec<u8>,
}
impl ParserNumericFrames {
    pub(crate) fn retained_capacity_bytes(&self) -> Option<usize> {
        self.indices
            .capacity()
            .checked_add(self.scores.capacity())?
            .checked_add(self.output.capacity())
    }
}

pub(crate) struct ParserNumericHistory<P: FixedParserNumericProfile = PinnedFourSlotNumericProfile>
{
    pub(crate) features: ParserCanonicalHistory<P::Query, P::Features>,
    pub(crate) indices: Vec<u8>,
    pub(crate) scores: Vec<u8>,
    pub(crate) output: Vec<u8>,
    pub(crate) original_model: Arc<PreparedCategoricalStep>,
    // Complete original selected Plan, shared immutably and charged once
    // by its preparation owner. This is not an authorization digest.
    pub(crate) original_plan: Rc<conduit_core::Plan>,
    pub(crate) ordinal: u64,
}

#[derive(Debug)]
pub(crate) enum ParserNumericRefusal<E> {
    Cancelled,
    Parent,
    Pressure,
    Native,
    Source,
    Model,
    DifferentOutput,
    Execution(E),
}

pub(crate) struct PreparedParserNumericCustody<
    E: ParserNumericExecutor,
    P: FixedParserNumericProfile = PinnedFourSlotNumericProfile,
> {
    profile: core::marker::PhantomData<fn() -> P>,
    executor: E,
    family: Rc<RefCell<PreparedNativeFamily>>,
    projector: PreparedSourceVerification,
    wrapper: PreparedSourceVerification,
    numerical: PreparedCategoricalCanonicalAdmission,
    original_plan: Rc<conduit_core::Plan>,
    maximum_invocations: u32,
    next_ordinal: u64,
    cancelled: bool,
}
impl<E: ParserNumericExecutor, P: FixedParserNumericProfile> PreparedParserNumericCustody<E, P> {
    /// Only the fixed mixed Plan owner may assemble this after complete Source,
    /// resource, ordered-cord, endpoint and selected-placement admission.
    pub(crate) fn from_prepared(
        executor: E,
        family: Rc<RefCell<PreparedNativeFamily>>,
        projector: PreparedSourceVerification,
        wrapper: PreparedSourceVerification,
        numerical: PreparedCategoricalCanonicalAdmission,
        original_plan: Rc<conduit_core::Plan>,
        maximum_invocations: u32,
        selection: &P::Selection,
    ) -> Result<Self, ParserNumericRefusal<E::Error>> {
        use ParserNumericRefusal as R;
        if !P::admits_selection(selection)
            || !core::ptr::eq(P::selected_model(selection), numerical.profile().as_ref())
            || maximum_invocations == 0
            || original_plan.fragments.is_empty()
            || projector.entry() != P::INDICES
            || wrapper.entry() != P::SCORES
            || numerical.profile().dimensions().1 != P::SCORE_CLASSES
            || numerical.profile().dimensions().2 != P::LOOKUPS
            || !family
                .borrow()
                .contains_descriptor(P::Features::PREPARED_DESCRIPTOR)
            || !family
                .borrow()
                .contains_descriptor(P::Scores::PREPARED_DESCRIPTOR)
        {
            return Err(R::Parent);
        }
        Ok(Self {
            profile: core::marker::PhantomData,
            executor,
            family,
            projector,
            wrapper,
            numerical,
            original_plan,
            maximum_invocations,
            next_ordinal: 0,
            cancelled: false,
        })
    }

    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
        self.executor.cancel();
    }
    /// Consumption is limited to an opaque complete Source feature execution.
    /// Arbitrary individually valid Native feature snapshots cannot enter here.
    pub(crate) fn execute(
        &mut self,
        features: ParserSessionExecution<P::Query, P::Features>,
        mut frames: ParserNumericFrames,
    ) -> Result<ParserNumericHistory<P>, ParserNumericRefusal<E::Error>> {
        use ParserNumericRefusal as R;
        if self.cancelled {
            return Err(R::Cancelled);
        }
        if features.entry() != P::FEATURES {
            return Err(R::Parent);
        }
        if self.next_ordinal >= u64::from(self.maximum_invocations) {
            return Err(R::Pressure);
        }
        if self.executor.plan() != self.original_plan.as_ref() {
            return Err(R::Parent);
        }
        let (_, _, history) = ParserCanonicalHistory::from_execution(features);
        // Full parent bytes survive the drop of its two provisional Native values.
        // Readmission here is independently covered by the active decode quota.
        drop(
            self.family
                .borrow_mut()
                .decode::<P::Features>(history.output_bytes())
                .map_err(|_| R::Native)?,
        );
        let indices = self
            .projector
            .evaluate(history.output_bytes())
            .map_err(|_| R::Source)?;
        if indices.len() > frames.indices.capacity() {
            return Err(R::Pressure);
        }
        frames.indices.clear();
        frames.indices.extend_from_slice(indices);
        let scores = self
            .numerical
            .evaluate(&frames.indices)
            .map_err(|_| R::Model)?;
        if scores.len() > frames.scores.capacity() {
            return Err(R::Pressure);
        }
        frames.scores.clear();
        frames.scores.extend_from_slice(scores);
        let expected = self
            .wrapper
            .evaluate(&frames.scores)
            .map_err(|_| R::Source)?;
        if expected.len() > frames.output.len() {
            return Err(R::Pressure);
        }
        drop(
            self.family
                .borrow_mut()
                .decode::<P::Scores>(expected)
                .map_err(|_| R::Native)?,
        );
        // Poison before the first target consumption. Late refusal and unwinding
        // both leave this owner permanently unavailable, without a partial receipt.
        self.cancelled = true;
        struct ConsumedTarget<'a, E: ParserNumericExecutor> {
            target: &'a mut E,
            published: bool,
        }
        impl<E: ParserNumericExecutor> Drop for ConsumedTarget<'_, E> {
            fn drop(&mut self) {
                if !self.published {
                    self.target.cancel();
                }
            }
        }
        let mut consumed = ConsumedTarget {
            target: &mut self.executor,
            published: false,
        };
        let length = consumed
            .target
            .transact(
                self.next_ordinal,
                history.output_bytes(),
                &mut frames.output,
            )
            .map_err(R::Execution)?;
        if length > frames.output.len() || expected != &frames.output[..length] {
            return Err(R::DifferentOutput);
        }
        frames.output.truncate(length);
        let receipt = ParserNumericHistory {
            features: history,
            indices: frames.indices,
            scores: frames.scores,
            output: frames.output,
            original_model: self.numerical.profile().clone(),
            original_plan: self.original_plan.clone(),
            ordinal: self.next_ordinal,
        };
        consumed.published = true;
        drop(consumed);
        self.next_ordinal += 1;
        self.cancelled = false;
        Ok(receipt)
    }
}
impl<E: ParserNumericExecutor, P: FixedParserNumericProfile> Drop
    for PreparedParserNumericCustody<E, P>
{
    fn drop(&mut self) {
        self.executor.cancel();
    }
}

/// Exclusive historical admission; prepared Source evaluators, the numerical
/// codec, original model, original Plan and stored frames are separate owners.
pub(crate) struct ParserNumericReadmissionBudget {
    pub(crate) maximum_live_native_bytes: usize,
}
pub(crate) struct ParserNumericReadmission<
    'a,
    P: FixedParserNumericProfile = PinnedFourSlotNumericProfile,
> {
    pub(crate) output: P::Scores,
    _budget: &'a mut ParserNumericReadmissionBudget,
}
impl<P: FixedParserNumericProfile> ParserNumericHistory<P> {
    pub(crate) fn replay_and_readmit<'a>(
        &self,
        feature_verifier: &mut PreparedSourceVerification,
        projector: &mut PreparedSourceVerification,
        wrapper: &mut PreparedSourceVerification,
        numerical: &mut PreparedCategoricalCanonicalAdmission,
        family: &mut PreparedNativeFamily,
        expected_original_plan: &conduit_core::Plan,
        budget: &'a mut ParserNumericReadmissionBudget,
    ) -> Result<ParserNumericReadmission<'a, P>, ParserNumericRefusal<core::convert::Infallible>>
    {
        use ParserNumericRefusal as R;
        // Reserve the entire historical peak before any decode or replay.
        let peak = family
            .storage_receipt()
            .conversion_requested_bytes_bound
            .checked_mul(2)
            .ok_or(R::Pressure)?;
        if peak > budget.maximum_live_native_bytes {
            return Err(R::Pressure);
        }
        if self.features.entry() != P::FEATURES
            || projector.entry() != P::INDICES
            || wrapper.entry() != P::SCORES
            || !Arc::ptr_eq(numerical.profile(), &self.original_model)
            || self.original_plan.as_ref() != expected_original_plan
        {
            return Err(R::Parent);
        }
        let mut parent_budget =
            crate::parser_canonical_history::ParserHistoricalReadmissionBudget::new(peak)
                .ok_or(R::Pressure)?;
        drop(
            self.features
                .replay_and_readmit(feature_verifier, family, &mut parent_budget)
                .map_err(|_| R::Source)?,
        );
        if projector
            .evaluate(self.features.output_bytes())
            .map_err(|_| R::Source)?
            != self.indices
        {
            return Err(R::DifferentOutput);
        }
        if numerical.evaluate(&self.indices).map_err(|_| R::Model)? != self.scores {
            return Err(R::DifferentOutput);
        }
        if wrapper.evaluate(&self.scores).map_err(|_| R::Source)? != self.output {
            return Err(R::DifferentOutput);
        }
        if !family.contains_descriptor(P::Scores::PREPARED_DESCRIPTOR) {
            return Err(R::Native);
        }
        let output = family
            .decode::<P::Scores>(&self.output)
            .map_err(|_| R::Native)?;
        Ok(ParserNumericReadmission {
            output,
            _budget: budget,
        })
    }
}
