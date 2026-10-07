//! Borrow an admitted independent lexical fact while dependency arcs remain open.
use crate::{
    lexical::PreparedLexicalTape, LanguageLexicalCandidate, LanguageParserWindow8StableLexicalFact,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

#[derive(Debug)]
pub enum StableLexicalSelectionRefusal {
    LexicalTape,
    Program,
    Native(NativeBindingRefusal),
}

pub struct PreparedStableLexicalSelection<'a> {
    fact: &'a LanguageParserWindow8StableLexicalFact,
    lexical: &'a PreparedLexicalTape,
    candidate: LanguageLexicalCandidate,
}
impl<'a> PreparedStableLexicalSelection<'a> {
    pub fn fact(&self) -> &'a LanguageParserWindow8StableLexicalFact {
        self.fact
    }
    pub fn lexical(&self) -> &'a PreparedLexicalTape {
        self.lexical
    }
    pub fn candidate(&self) -> &LanguageLexicalCandidate {
        &self.candidate
    }
}

/// This preparation projects already-admitted lexical agreement. It grants no
/// dependency commitment, prosodic boundary, or playback authority.
pub fn prepare_stable_lexical_selection<'a>(
    lexical: &'a PreparedLexicalTape,
    fact: &'a LanguageParserWindow8StableLexicalFact,
) -> Result<PreparedStableLexicalSelection<'a>, StableLexicalSelectionRefusal> {
    use StableLexicalSelectionRefusal::*;
    if lexical.tape() != fact.query().snapshot().lexical().tape() {
        return Err(LexicalTape);
    }
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
        concat!(env!("OUT_DIR"), "/window8_stable_lexical_candidate.hex")
    ))
    .map_err(|_| Program)?;
    let output = program
        .evaluate(&fact.clone().encode().map_err(Native)?)
        .map_err(|_| Program)?;
    let candidate = LanguageLexicalCandidate::decode(&output).map_err(Native)?;
    Ok(PreparedStableLexicalSelection {
        fact,
        lexical,
        candidate,
    })
}
