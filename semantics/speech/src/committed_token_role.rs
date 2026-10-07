//! A speech token role retaining the exact Source dependency commit custody.
use crate::text_token_role::{
    prepare_text_token_role, PreparedTextTokenRole, TextTokenRoleRefusal,
};
use conduit_language::{lexical::PreparedLexicalTape, LanguageParserCommittedDependencyAdmission};

#[derive(Debug)]
pub enum CommittedTokenRoleRefusal {
    LexicalTape,
    Dependent,
    Role(TextTokenRoleRefusal),
}

pub struct PreparedCommittedTokenRole<'a> {
    committed: &'a LanguageParserCommittedDependencyAdmission,
    role: PreparedTextTokenRole,
}

impl<'a> PreparedCommittedTokenRole<'a> {
    pub fn committed(&self) -> &'a LanguageParserCommittedDependencyAdmission {
        self.committed
    }

    pub fn role(&self) -> &PreparedTextTokenRole {
        &self.role
    }
}

pub fn prepare_committed_token_role<'a>(
    lexical: &PreparedLexicalTape,
    committed: &'a LanguageParserCommittedDependencyAdmission,
) -> Result<PreparedCommittedTokenRole<'a>, CommittedTokenRoleRefusal> {
    let admission = committed.admission();
    let query = admission.fact().query();
    if lexical.tape() != query.beam().lexical().tape() {
        return Err(CommittedTokenRoleRefusal::LexicalTape);
    }
    let dependent =
        usize::try_from(*query.dependent()).map_err(|_| CommittedTokenRoleRefusal::Dependent)?;
    let choice = query
        .beam()
        .candidate0()
        .choices()
        .get(dependent)
        .ok_or(CommittedTokenRoleRefusal::Dependent)?;
    let role = prepare_text_token_role(
        lexical,
        dependent,
        query.beam().basis().analysis_revision(),
        admission.arc(),
        *choice,
    )
    .map_err(CommittedTokenRoleRefusal::Role)?;
    Ok(PreparedCommittedTokenRole { committed, role })
}
