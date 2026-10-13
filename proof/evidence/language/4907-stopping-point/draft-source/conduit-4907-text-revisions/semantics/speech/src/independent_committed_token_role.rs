//! Unwired speech participation from an explicit independent Source commit.
//! Protection, stable facts, and coverage alone cannot satisfy this API.
use crate::text_token_role::{
    prepare_text_token_role, PreparedTextTokenRole, TextTokenRoleRefusal,
};
use conduit_language::{
    lexical::PreparedLexicalTape, parser_independent_commit::PreparedIndependentCommittedDependency,
};

#[derive(Debug)]
pub enum IndependentCommittedTokenRoleRefusal {
    LexicalTape,
    Dependent,
    Role(TextTokenRoleRefusal),
}
pub struct PreparedIndependentCommittedTokenRole<'a> {
    committed: &'a PreparedIndependentCommittedDependency,
    role: PreparedTextTokenRole,
}
impl<'a> PreparedIndependentCommittedTokenRole<'a> {
    pub fn committed(&self) -> &'a PreparedIndependentCommittedDependency {
        self.committed
    }
    pub fn role(&self) -> &PreparedTextTokenRole {
        &self.role
    }
}
pub fn prepare_independent_committed_token_role<'a>(
    lexical: &PreparedLexicalTape,
    committed: &'a PreparedIndependentCommittedDependency,
) -> Result<PreparedIndependentCommittedTokenRole<'a>, IndependentCommittedTokenRoleRefusal> {
    let admission = committed.request().admission();
    let query = admission.fact().query();
    if lexical.tape() != query.beam().lexical().tape() {
        return Err(IndependentCommittedTokenRoleRefusal::LexicalTape);
    }
    let dependent = usize::try_from(*query.dependent())
        .map_err(|_| IndependentCommittedTokenRoleRefusal::Dependent)?;
    let choice = query
        .beam()
        .candidate0()
        .choices()
        .get(dependent)
        .ok_or(IndependentCommittedTokenRoleRefusal::Dependent)?;
    let role = prepare_text_token_role(
        lexical,
        dependent,
        query.beam().basis().analysis_revision(),
        admission.arc(),
        *choice,
    )
    .map_err(IndependentCommittedTokenRoleRefusal::Role)?;
    Ok(PreparedIndependentCommittedTokenRole { committed, role })
}
