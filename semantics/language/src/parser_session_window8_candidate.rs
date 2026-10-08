//! Exact provisional qualified lexical admission gate from the retained Source parser.
//! Only an invariant refusal at this gate is a semantic candidate rejection.
use crate::{
    LanguageParserWindow8QualifiedLexicalAnalysis, LanguageParserWindow8QualifiedLexicalProposal,
    parser_canonical_refinement::PreparedParserCanonicalRefinement,
    parser_session_pure_source::PureSourceHistory, parser_session_window8_ports::PORTS,
};
use alloc::vec::Vec;
use conduit_core::validate_canonical_structured_value;
use conduit_plot::rust_binding::{
    NativeBindingRefusal, PreparedNativeFamily, PreparedNativeRustBinding,
};

pub(crate) struct Window8QualifiedLexicalAdmission {
    /// Locates a complete retained qualified lexical proposal execution in the same revision book.
    pub(crate) lexical_proposal_execution: usize,
    /// Full refined candidate including original query, context and proposal.
    pub(crate) original_candidate: Vec<u8>,
    pub(crate) outcome: QualifiedLexicalOutcome,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QualifiedLexicalOutcome {
    Accepted,
    RejectedInvariant { index: usize },
}
#[derive(Debug)]
pub(crate) enum QualifiedLexicalRefusal {
    Origin,
    Pressure,
    Refinement,
    Native(NativeBindingRefusal),
}
impl Window8QualifiedLexicalAdmission {
    pub(crate) fn into_book(self) -> crate::parser_session_window8_book::BookAdmission {
        use crate::parser_session_window8_book::{BookAdmission, BookAdmissionOutcome};
        BookAdmission {
            gate: crate::parser_session_window8_book::BookAdmissionGate::QualifiedLexical,
            canonical: self.original_candidate,
            descriptor: LanguageParserWindow8QualifiedLexicalAnalysis::PREPARED_DESCRIPTOR,
            source_execution: self.lexical_proposal_execution,
            outcome: match self.outcome {
                QualifiedLexicalOutcome::Accepted => BookAdmissionOutcome::Accepted,
                QualifiedLexicalOutcome::RejectedInvariant { index } => {
                    BookAdmissionOutcome::ProvisionalInvariantRefusal {
                        original_law_index: index,
                    }
                }
            },
        }
    }
    /// The supplied buffer is reserved by the revision before any consumption.
    /// This mechanically preserves all fields from the exact qualified lexical proposal output;
    /// neither a caller-selected snapshot nor an independently valid fact enters.
    pub(crate) fn admit(
        lexical_proposal_execution: usize,
        origin: &PureSourceHistory,
        refinement: &mut PreparedParserCanonicalRefinement<
            LanguageParserWindow8QualifiedLexicalProposal,
            LanguageParserWindow8QualifiedLexicalAnalysis,
        >,
        family: &mut PreparedNativeFamily,
        mut buffer: Vec<u8>,
    ) -> Result<Self, QualifiedLexicalRefusal> {
        use QualifiedLexicalRefusal as R;
        if !original_qualified_port(origin)
            || !family.contains_descriptor(
                LanguageParserWindow8QualifiedLexicalAnalysis::PREPARED_DESCRIPTOR,
            )
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
        let outcome = match family.decode::<LanguageParserWindow8QualifiedLexicalAnalysis>(&buffer)
        {
            Ok(value) => {
                drop(value);
                QualifiedLexicalOutcome::Accepted
            }
            Err(NativeBindingRefusal::ViolatedInvariant { index }) => {
                QualifiedLexicalOutcome::RejectedInvariant { index }
            }
            Err(error) => return Err(R::Native(error)),
        };
        Ok(Self {
            lexical_proposal_execution,
            original_candidate: buffer,
            outcome,
        })
    }
    /// Historical admission is separately reserved by the Session; the family
    /// retains the exact original Type and ordered law programs for this fact.
    pub(crate) fn readmit(
        &self,
        origin: &PureSourceHistory,
        refinement: &mut PreparedParserCanonicalRefinement<
            LanguageParserWindow8QualifiedLexicalProposal,
            LanguageParserWindow8QualifiedLexicalAnalysis,
        >,
        family: &mut PreparedNativeFamily,
    ) -> Result<(), QualifiedLexicalRefusal> {
        use QualifiedLexicalRefusal as R;
        if !family
            .contains_descriptor(LanguageParserWindow8QualifiedLexicalAnalysis::PREPARED_DESCRIPTOR)
        {
            return Err(R::Origin);
        }
        if !original_qualified_port(origin) {
            return Err(R::Origin);
        }
        let source = validate_canonical_structured_value(&origin.output).map_err(|_| R::Origin)?;
        if refinement.compose(source).map_err(|_| R::Refinement)?
            != self.original_candidate.as_slice()
        {
            return Err(R::Origin);
        }
        let observed = match family
            .decode::<LanguageParserWindow8QualifiedLexicalAnalysis>(&self.original_candidate)
        {
            Ok(value) => {
                drop(value);
                QualifiedLexicalOutcome::Accepted
            }
            Err(NativeBindingRefusal::ViolatedInvariant { index }) => {
                QualifiedLexicalOutcome::RejectedInvariant { index }
            }
            Err(error) => return Err(R::Native(error)),
        };
        if observed != self.outcome {
            return Err(R::Origin);
        }
        Ok(())
    }
}

fn original_qualified_port(origin: &PureSourceHistory) -> bool {
    PORTS
        .iter()
        .find(|port| port.name == "language-window8-qualified-lexical-proposal")
        .is_some_and(|port| {
            origin.matches_fixed(
                port.original_programs,
                port.original_custody,
                port.input,
                port.output,
            )
        })
}
