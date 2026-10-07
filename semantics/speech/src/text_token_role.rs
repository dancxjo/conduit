//! Source-owned spoken/nonspoken participation; complete material is retained.
use crate::semantic::*;
use conduit_language::{lexical::PreparedLexicalTape, *};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
#[derive(Debug)]
pub enum TextTokenRoleRefusal {
    Token,
    Arc,
    Native(NativeBindingRefusal),
    Program,
    Inconsistent(alloc::boxed::Box<RejectedTextTokenRole>),
}
#[derive(Debug)]
pub struct RejectedTextTokenRole {
    request: SpeechTextTokenRoleRequest,
    result: SpeechTextTokenRoleResult,
}
impl RejectedTextTokenRole {
    pub fn request(&self) -> &SpeechTextTokenRoleRequest {
        &self.request
    }
    pub fn result(&self) -> &SpeechTextTokenRoleResult {
        &self.result
    }
}
pub struct PreparedTextTokenRole {
    request: SpeechTextTokenRoleRequest,
    result: SpeechTextTokenRoleResult,
}
impl PreparedTextTokenRole {
    pub fn request(&self) -> &SpeechTextTokenRoleRequest {
        &self.request
    }
    pub fn result(&self) -> &SpeechTextTokenRoleResult {
        &self.result
    }
}
pub fn prepare_text_token_role(
    lexical: &PreparedLexicalTape,
    ordinal: usize,
    analysis: &LanguageAnalysisRevisionId,
    arc: &LanguageDependencyArc,
    choice: u64,
) -> Result<PreparedTextTokenRole, TextTokenRoleRefusal> {
    use TextTokenRoleRefusal::*;
    let tape = lexical.tape();
    let token = tape.tokens().iter().nth(ordinal).ok_or(Token)?;
    if let LanguageDependencyHead::Token(head) = arc.governor() {
        if head.revision() != analysis
            || !tape.tokens().iter().any(|t| t.identity() == head.token())
        {
            return Err(Arc);
        }
    }
    let request = SpeechTextTokenRoleRequest::new(
        analysis.clone(),
        arc.clone(),
        choice,
        tape.source().clone(),
        token.clone(),
    )
    .map_err(Native)?;
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
        concat!(env!("OUT_DIR"), "/text_token_role_program.hex")
    ))
    .map_err(|_| Program)?;
    let bytes = program
        .evaluate(&request.clone().encode().map_err(Native)?)
        .map_err(|_| Program)?;
    let result = SpeechTextTokenRoleResult::decode(&bytes).map_err(Native)?;
    if matches!(result.role(), SpeechTextTokenRole::Refused) {
        return Err(Inconsistent(alloc::boxed::Box::new(
            RejectedTextTokenRole { request, result },
        )));
    }
    Ok(PreparedTextTokenRole { request, result })
}

/// Explicit schema family for callers that install this bounded projection.
pub fn text_token_role_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)>
{
    alloc::vec![
        (
            "SpeechTextTokenRole",
            SpeechTextTokenRole::semantic_type().expect("token role")
        ),
        (
            "SpeechTextTokenRoleRequest",
            SpeechTextTokenRoleRequest::semantic_type().expect("token role request")
        ),
        (
            "SpeechTextTokenRoleResult",
            SpeechTextTokenRoleResult::semantic_type().expect("token role result")
        ),
    ]
}
