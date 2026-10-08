//! One cancellation domain for the complete fixed Window8 Source selection and
//! the original Source/Kernel model boundary. No optional or foreign ports.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_mixed_custody::PreparedParserMixedCustody,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_pure_source::{PreparedParserPureSource, PureSourceRefusal},
    parser_session_stage::ParserSessionTargets,
    parser_session_window8_numeric_profile::ProposalWindow8V2NumericProfile,
    parser_session_window8_ports::PORTS,
};
use alloc::vec::Vec;
pub(crate) struct Window8Registry<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor> {
    source: Vec<PreparedParserPureSource>,
    mixed: PreparedParserMixedCustody<S, N, ProposalWindow8V2NumericProfile>,
    cancelled: bool,
}
impl<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor> Window8Registry<S, N> {
    /// All complete owner and preparation reservations precede this move-only
    /// assembly. The original ordered selection is checked without allocation.
    pub(crate) fn from_prepared(
        mut source: Vec<PreparedParserPureSource>,
        mut mixed: PreparedParserMixedCustody<S, N, ProposalWindow8V2NumericProfile>,
    ) -> Result<Self, PureSourceRefusal> {
        if source.len() != PORTS.len()
            || !source.iter().zip(PORTS).all(|(owner, fixed)| {
                owner.matches_fixed(
                    fixed.original_programs,
                    fixed.original_custody,
                    fixed.input,
                    fixed.output,
                )
            })
        {
            for owner in &mut source {
                owner.cancel();
            }
            mixed.cancel();
            return Err(PureSourceRefusal::Descriptor);
        }
        Ok(Self {
            source,
            mixed,
            cancelled: false,
        })
    }
    pub(crate) fn source(
        &mut self,
        index: usize,
    ) -> Result<&mut PreparedParserPureSource, PureSourceRefusal> {
        if self.cancelled {
            return Err(PureSourceRefusal::Closed);
        }
        self.source
            .get_mut(index)
            .ok_or(PureSourceRefusal::Descriptor)
    }
    pub(crate) fn mixed(
        &mut self,
    ) -> Result<
        &mut PreparedParserMixedCustody<S, N, ProposalWindow8V2NumericProfile>,
        PureSourceRefusal,
    > {
        if self.cancelled {
            return Err(PureSourceRefusal::Closed);
        }
        Ok(&mut self.mixed)
    }
}
impl<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor> ParserSessionTargets
    for Window8Registry<S, N>
{
    fn cancel_all(&mut self) {
        self.cancelled = true;
        for owner in &mut self.source {
            owner.cancel();
        }
        self.mixed.cancel();
    }
}
