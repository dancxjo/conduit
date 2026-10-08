//! Discourse derived from, and retaining, an original Native dependency commit.
//! This grants no target-language commitment or device playback authority.
use crate::{
    discourse::{prepare_vocative_fact, DiscourseRefusal, PreparedVocativeFact},
    lexical::PreparedLexicalTape,
    LanguageParserCommittedDependencyAdmission, LinguisticDerivationProvenance,
};

#[derive(Debug)]
pub enum CommittedDiscourseRefusal {
    LexicalBasis,
    Discourse(DiscourseRefusal),
}

pub struct PreparedCommittedVocativeFact<'a> {
    committed: &'a LanguageParserCommittedDependencyAdmission,
    fact: PreparedVocativeFact,
}

impl<'a> PreparedCommittedVocativeFact<'a> {
    pub fn committed(&self) -> &'a LanguageParserCommittedDependencyAdmission {
        self.committed
    }

    pub fn fact(&self) -> &PreparedVocativeFact {
        &self.fact
    }
}

pub fn prepare_committed_vocative_fact<'a>(
    identity: alloc::string::String,
    lexical: &PreparedLexicalTape,
    committed: &'a LanguageParserCommittedDependencyAdmission,
    provenance: LinguisticDerivationProvenance,
) -> Result<PreparedCommittedVocativeFact<'a>, CommittedDiscourseRefusal> {
    let admission = committed.admission();
    let beam = admission.fact().query().beam();
    if lexical.tape() != beam.lexical().tape() {
        return Err(CommittedDiscourseRefusal::LexicalBasis);
    }
    let fact = prepare_vocative_fact(
        identity,
        lexical.tape().source(),
        beam.basis().analysis_revision(),
        admission.arc(),
        provenance,
        lexical.tape().tokens().len() as u64,
    )
    .map_err(CommittedDiscourseRefusal::Discourse)?;
    Ok(PreparedCommittedVocativeFact { committed, fact })
}
