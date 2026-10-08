//! Exact provisional stable-fact admission gate from the retained Source parser.
//! Only an invariant refusal at this gate is a semantic candidate rejection.
use crate::{
    parser_canonical_refinement::PreparedParserCanonicalRefinement,
    parser_session_execution::ParserSessionEntry, parser_session_fixed_ingress::ParserFixedHistory,
    LanguageParserJointStableFact, LanguageParserJointStableFactProposal,
};
use alloc::vec::Vec;
use conduit_core::validate_canonical_structured_value;
use conduit_plot::rust_binding::{
    NativeBindingRefusal, PreparedNativeFamily, PreparedNativeRustBinding,
};

pub(crate) struct ParserStableCandidateAdmission {
    /// Locates a complete retained stable-proposal execution in the same revision book.
    pub(crate) stable_proposal_execution: usize,
    /// Full refined candidate including original query, context and proposal.
    pub(crate) original_candidate: Vec<u8>,
    pub(crate) outcome: StableCandidateOutcome,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StableCandidateOutcome {
    Accepted,
    RejectedInvariant { index: usize },
}
#[derive(Debug)]
pub(crate) enum StableCandidateRefusal {
    Origin,
    Pressure,
    Refinement,
    Native(NativeBindingRefusal),
}
impl ParserStableCandidateAdmission {
    /// The supplied buffer is reserved by the revision before any consumption.
    /// This mechanically preserves all fields from the exact stable-proposal output;
    /// neither a caller-selected snapshot nor an independently valid fact enters.
    pub(crate) fn admit(
        stable_proposal_execution: usize,
        origin: &ParserFixedHistory,
        refinement: &mut PreparedParserCanonicalRefinement<
            LanguageParserJointStableFactProposal,
            LanguageParserJointStableFact,
        >,
        family: &mut PreparedNativeFamily,
        mut buffer: Vec<u8>,
    ) -> Result<Self, StableCandidateRefusal> {
        use StableCandidateRefusal as R;
        if origin.entry != ParserSessionEntry::StableFact
            || !family.contains_descriptor(LanguageParserJointStableFact::PREPARED_DESCRIPTOR)
        {
            return Err(R::Origin);
        }
        let source = validate_canonical_structured_value(&origin.output).map_err(|_| R::Origin)?;
        let candidate = refinement.compose(source).map_err(|_| R::Refinement)?;
        if candidate.len() > buffer.capacity() {
            return Err(R::Pressure);
        }
        buffer.clear();
        buffer.extend_from_slice(candidate);
        let outcome = match family.decode::<LanguageParserJointStableFact>(&buffer) {
            Ok(value) => {
                drop(value);
                StableCandidateOutcome::Accepted
            }
            Err(NativeBindingRefusal::ViolatedInvariant { index }) => {
                StableCandidateOutcome::RejectedInvariant { index }
            }
            Err(error) => return Err(R::Native(error)),
        };
        Ok(Self {
            stable_proposal_execution,
            original_candidate: buffer,
            outcome,
        })
    }
    /// Historical admission is separately reserved by the Session; the family
    /// retains the exact original Type and ordered law programs for this fact.
    pub(crate) fn readmit(
        &self,
        origin: &ParserFixedHistory,
        refinement: &mut PreparedParserCanonicalRefinement<
            LanguageParserJointStableFactProposal,
            LanguageParserJointStableFact,
        >,
        family: &mut PreparedNativeFamily,
    ) -> Result<(), StableCandidateRefusal> {
        use StableCandidateRefusal as R;
        if !family.contains_descriptor(LanguageParserJointStableFact::PREPARED_DESCRIPTOR) {
            return Err(R::Origin);
        }
        if origin.entry != ParserSessionEntry::StableFact {
            return Err(R::Origin);
        }
        let source = validate_canonical_structured_value(&origin.output).map_err(|_| R::Origin)?;
        if refinement.compose(source).map_err(|_| R::Refinement)?
            != self.original_candidate.as_slice()
        {
            return Err(R::Origin);
        }
        let observed =
            match family.decode::<LanguageParserJointStableFact>(&self.original_candidate) {
                Ok(value) => {
                    drop(value);
                    StableCandidateOutcome::Accepted
                }
                Err(NativeBindingRefusal::ViolatedInvariant { index }) => {
                    StableCandidateOutcome::RejectedInvariant { index }
                }
                Err(error) => return Err(R::Native(error)),
            };
        if observed != self.outcome {
            return Err(R::Origin);
        }
        Ok(())
    }
}
