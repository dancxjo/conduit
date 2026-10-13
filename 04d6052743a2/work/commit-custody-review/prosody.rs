//! Checked source profiles own symbolic prosody, independently of acoustic rates.
use crate::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum ProsodyRefusal {
    Native(NativeBindingRefusal),
    Program,
    Token,
    Source,
}
pub struct PreparedRichProsody {
    accepted: LanguageRichProsodyAccepted,
}
impl PreparedRichProsody {
    pub fn requested(&self) -> &LanguageRichProsodyRequest {
        self.accepted.request()
    }
    pub fn accepted(&self) -> &LanguageRichProsodyAccepted {
        &self.accepted
    }
}
pub struct PreparedFallbackProsody {
    accepted: LanguageFallbackProsodyAccepted,
}
impl PreparedFallbackProsody {
    pub fn requested(&self) -> &LanguageFallbackProsodyRequest {
        self.accepted.request()
    }
    pub fn accepted(&self) -> &LanguageFallbackProsodyAccepted {
        &self.accepted
    }
}
fn choice<T: NativeRustBinding>(
    input: T,
    program: &str,
) -> Result<LanguageProsodyChoice, ProsodyRefusal> {
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program)
        .map_err(|_| ProsodyRefusal::Program)?;
    let bytes = program
        .evaluate(&input.encode().map_err(ProsodyRefusal::Native)?)
        .map_err(|_| ProsodyRefusal::Program)?;
    LanguageProsodyChoice::decode(&bytes).map_err(ProsodyRefusal::Native)
}
pub fn prepare_rich_prosody(
    lexical: &crate::lexical::PreparedLexicalTape,
    ordinal: usize,
    discourse: &LanguageVocativeDiscourseFact,
    profile: &LanguageProsodyProfile,
) -> Result<PreparedRichProsody, ProsodyRefusal> {
    let source = lexical.tape().source();
    let token = lexical
        .tape()
        .tokens()
        .iter()
        .nth(ordinal)
        .ok_or(ProsodyRefusal::Token)?;
    if discourse.source() != source {
        return Err(ProsodyRefusal::Source);
    }
    let requested = LanguageRichProsodyRequest::new(
        discourse.clone(),
        lexical.tape().profile().clone(),
        profile.clone(),
        source.clone(),
        token.clone(),
    )
    .map_err(ProsodyRefusal::Native)?;
    let selected = choice(
        requested.clone(),
        include_str!(concat!(env!("OUT_DIR"), "/rich_prosody_program.hex")),
    )?;
    let accepted =
        LanguageRichProsodyAccepted::new(selected, requested).map_err(ProsodyRefusal::Native)?;
    Ok(PreparedRichProsody { accepted })
}
pub fn prepare_fallback_prosody(
    lexical: &crate::lexical::PreparedLexicalTape,
    ordinal: usize,
    profile: &LanguageProsodyProfile,
) -> Result<PreparedFallbackProsody, ProsodyRefusal> {
    let source = lexical.tape().source();
    let token = lexical
        .tape()
        .tokens()
        .iter()
        .nth(ordinal)
        .ok_or(ProsodyRefusal::Token)?;
    let requested = LanguageFallbackProsodyRequest::new(
        lexical.tape().profile().clone(),
        profile.clone(),
        source.clone(),
        token.clone(),
    )
    .map_err(ProsodyRefusal::Native)?;
    let selected = choice(
        requested.clone(),
        include_str!(concat!(env!("OUT_DIR"), "/fallback_prosody_program.hex")),
    )?;
    let accepted = LanguageFallbackProsodyAccepted::new(selected, requested)
        .map_err(ProsodyRefusal::Native)?;
    Ok(PreparedFallbackProsody { accepted })
}
pub fn prosody_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageProsodyBoundary",
            LanguageProsodyBoundary::semantic_type().unwrap()
        ),
        (
            "LanguageProsodyProminence",
            LanguageProsodyProminence::semantic_type().unwrap()
        ),
        (
            "LanguageProsodyPitch",
            LanguageProsodyPitch::semantic_type().unwrap()
        ),
        (
            "LanguageProsodyChoice",
            LanguageProsodyChoice::semantic_type().unwrap()
        ),
        (
            "LanguageProsodyProfile",
            LanguageProsodyProfile::semantic_type().unwrap()
        ),
        (
            "LanguageRichProsodyRequest",
            LanguageRichProsodyRequest::semantic_type().unwrap()
        ),
        (
            "LanguageFallbackProsodyRequest",
            LanguageFallbackProsodyRequest::semantic_type().unwrap()
        ),
        (
            "LanguageRichProsodyAccepted",
            LanguageRichProsodyAccepted::semantic_type().unwrap()
        ),
        (
            "LanguageFallbackProsodyAccepted",
            LanguageFallbackProsodyAccepted::semantic_type().unwrap()
        ),
    ]
}
