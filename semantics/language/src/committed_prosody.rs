//! Rich prosody prepared from an original committed dependency's discourse fact.
//! The dependency is committed; this grants no prosody commitment or playback.
use crate::{
    committed_discourse::PreparedCommittedVocativeFact,
    lexical::PreparedLexicalTape,
    prosody::{prepare_rich_prosody, PreparedRichProsody, ProsodyRefusal},
    LanguageProsodyProfile,
};
#[derive(Debug)]
pub enum CommittedVocativeProsodyRefusal {
    LexicalBasis,
    Prosody(ProsodyRefusal),
}
pub struct PreparedRichProsodyFromCommittedVocative<'a, 'commit> {
    discourse: &'a PreparedCommittedVocativeFact<'commit>,
    prosody: PreparedRichProsody,
}
impl<'a, 'commit> PreparedRichProsodyFromCommittedVocative<'a, 'commit> {
    pub fn discourse(&self) -> &'a PreparedCommittedVocativeFact<'commit> {
        self.discourse
    }
    pub fn prepared(&self) -> &PreparedRichProsody {
        &self.prosody
    }
}
pub fn prepare_rich_prosody_from_committed_vocative<'a, 'commit>(
    lexical: &PreparedLexicalTape,
    ordinal: usize,
    discourse: &'a PreparedCommittedVocativeFact<'commit>,
    profile: &LanguageProsodyProfile,
) -> Result<PreparedRichProsodyFromCommittedVocative<'a, 'commit>, CommittedVocativeProsodyRefusal>
{
    if lexical.tape()
        != discourse
            .committed()
            .admission()
            .fact()
            .query()
            .beam()
            .lexical()
            .tape()
    {
        return Err(CommittedVocativeProsodyRefusal::LexicalBasis);
    }
    let prosody = prepare_rich_prosody(lexical, ordinal, discourse.fact().fact(), profile)
        .map_err(CommittedVocativeProsodyRefusal::Prosody)?;
    Ok(PreparedRichProsodyFromCommittedVocative { discourse, prosody })
}
