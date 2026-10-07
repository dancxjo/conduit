//! A speech token role retaining the exact independent Source protection receipt.
//! This preparation grants no playback or effect-owner acknowledgment authority.
use crate::text_token_role::{
    prepare_text_token_role, PreparedTextTokenRole, TextTokenRoleRefusal,
};
use conduit_language::{lexical::PreparedLexicalTape, LanguageParserIndependentProtectedAdmission};

#[derive(Debug)]
pub enum IndependentTokenRoleRefusal {
    LexicalTape,
    Dependent,
    Role(TextTokenRoleRefusal),
}

pub struct PreparedIndependentTokenRole<'a> {
    protected: &'a LanguageParserIndependentProtectedAdmission,
    role: PreparedTextTokenRole,
}

impl<'a> PreparedIndependentTokenRole<'a> {
    pub fn protected(&self) -> &'a LanguageParserIndependentProtectedAdmission {
        self.protected
    }

    pub fn role(&self) -> &PreparedTextTokenRole {
        &self.role
    }
}

pub fn prepare_independent_token_role<'a>(
    lexical: &PreparedLexicalTape,
    protected: &'a LanguageParserIndependentProtectedAdmission,
) -> Result<PreparedIndependentTokenRole<'a>, IndependentTokenRoleRefusal> {
    let admission = protected.admission();
    let query = admission.fact().query();
    if lexical.tape() != query.beam().lexical().tape() {
        return Err(IndependentTokenRoleRefusal::LexicalTape);
    }
    let dependent =
        usize::try_from(*query.dependent()).map_err(|_| IndependentTokenRoleRefusal::Dependent)?;
    let choice = query
        .beam()
        .candidate0()
        .choices()
        .get(dependent)
        .ok_or(IndependentTokenRoleRefusal::Dependent)?;
    let role = prepare_text_token_role(
        lexical,
        dependent,
        query.beam().basis().analysis_revision(),
        admission.arc(),
        *choice,
    )
    .map_err(IndependentTokenRoleRefusal::Role)?;
    Ok(PreparedIndependentTokenRole { protected, role })
}
