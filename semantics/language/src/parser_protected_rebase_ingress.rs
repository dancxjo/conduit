//! Unwired guarded ingress component for a concrete protected rebase stage.
//! This is not the aggregate fact/commit/snapshot transaction owner.
use crate::parser_protected_custody::{CustodyRefusal, ProtectedRebaseStage};
use crate::parser_session_runtime::{
    ParserSourceExecutor, ParserSourceFlowRefusal, PreparedParserSourceFlow,
};
use crate::{
    LanguageParserProtectedRebaseReceipt, LanguageParserProtectedSetProposal,
    LanguageParserProtectedSetRebaseContext,
};

type RebaseFlow<E> = PreparedParserSourceFlow<
    LanguageParserProtectedSetRebaseContext,
    LanguageParserProtectedSetProposal,
    E,
>;

pub(crate) enum GuardedRebaseRefusal<E> {
    Custody(CustodyRefusal),
    Ingress(ParserSourceFlowRefusal<E>),
}
/// Exclusive Flow access prevents callers bypassing pre-consumption admission.
/// An abandoned consumed stage closes ingress automatically, without replay.
pub(crate) struct GuardedProtectedRebase<'a, E: ParserSourceExecutor> {
    stage: Option<ProtectedRebaseStage<'a>>,
    flow: &'a mut RebaseFlow<E>,
    consumed: bool,
    published: bool,
}
impl<'a, E: ParserSourceExecutor> GuardedProtectedRebase<'a, E> {
    pub(crate) fn new(stage: ProtectedRebaseStage<'a>, flow: &'a mut RebaseFlow<E>) -> Self {
        Self {
            stage: Some(stage),
            flow,
            consumed: false,
            published: false,
        }
    }
    pub(crate) fn execute(
        mut self,
        context: LanguageParserProtectedSetRebaseContext,
    ) -> Result<(), GuardedRebaseRefusal<E::Error>> {
        self.stage
            .as_ref()
            .expect("owned unpublished stage")
            .check_before_consumption()
            .map_err(GuardedRebaseRefusal::Custody)?;
        self.stage
            .as_ref()
            .expect("owned unpublished stage")
            .check_previous(context.previous())
            .map_err(GuardedRebaseRefusal::Custody)?;
        let output = match self.flow.transact(context.clone()) {
            Ok(output) => {
                self.consumed = true;
                output
            }
            Err(error) => {
                // Poison means the target may have consumed even on an error.
                self.consumed |= self.flow.is_poisoned();
                return Err(GuardedRebaseRefusal::Ingress(error));
            }
        };
        let receipt = LanguageParserProtectedRebaseReceipt::new(context, output)
            .map_err(|error| GuardedRebaseRefusal::Custody(CustodyRefusal::Native(error)))?;
        self.stage
            .take()
            .expect("owned unpublished stage")
            .finish(receipt)
            .map_err(GuardedRebaseRefusal::Custody)?;
        self.published = true;
        Ok(())
    }
}
impl<E: ParserSourceExecutor> Drop for GuardedProtectedRebase<'_, E> {
    fn drop(&mut self) {
        if self.consumed && !self.published {
            self.flow.cancel();
        }
    }
}
