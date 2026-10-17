//! Admission and generic portable evaluation of the checked source-owned rule.
//! Supplied UD edges are evidence inputs, not evidence that a parser succeeded.
use crate::*;
use conduit_core::revision::*;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

pub const VOCATIVE_DISCOURSE_SOURCE: &str = include_str!("../discourse.conduit");
#[derive(Debug)]
pub enum DiscourseRefusal {
    TokenBound,
    Native(NativeBindingRefusal),
    Program,
}
pub struct PreparedVocativeFact {
    fact: LanguageVocativeDiscourseFact,
}
impl PreparedVocativeFact {
    pub fn fact(&self) -> &LanguageVocativeDiscourseFact {
        &self.fact
    }
}
/// The native admission enforces exact text/analysis basis and UD relation.
/// Only the checked Plot determines the discourse role. No text inspection runs.
pub fn prepare_vocative_fact(
    identity: alloc::string::String,
    source: &LanguageTextRevision,
    analysis: &LanguageAnalysisRevisionId,
    basis: &LanguageDependencyArc,
    provenance: LinguisticDerivationProvenance,
    tokens: u64,
) -> Result<PreparedVocativeFact, DiscourseRefusal> {
    if !valid_tokens(basis, tokens) {
        return Err(DiscourseRefusal::TokenBound);
    }
    let input = LanguageVocativeDiscourseAdmission::new(
        analysis.clone(),
        basis.clone(),
        identity,
        provenance,
        source.clone(),
    )
    .map_err(DiscourseRefusal::Native)?;
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
        concat!(env!("OUT_DIR"), "/discourse_program.hex")
    ))
    .map_err(|_| DiscourseRefusal::Program)?;
    let output = program
        .evaluate(&input.clone().encode().map_err(DiscourseRefusal::Native)?)
        .map_err(|_| DiscourseRefusal::Program)?;
    let role = LanguageDiscourseRole::decode(&output).map_err(DiscourseRefusal::Native)?;
    let fact = LanguageVocativeDiscourseFact::new(
        input.analysis().clone(),
        input.basis().clone(),
        input.identity().clone(),
        input.provenance().clone(),
        role,
        input.source().clone(),
    )
    .map_err(DiscourseRefusal::Native)?;
    Ok(PreparedVocativeFact { fact })
}
fn valid_tokens(arc: &LanguageDependencyArc, tokens: u64) -> bool {
    tokens > 0
        && tokens <= 128
        && *arc.dependent().token().ordinal() < tokens
        && match arc.governor() {
            LanguageDependencyHead::Root => false,
            LanguageDependencyHead::Token(head) => *head.token().ordinal() < tokens,
        }
}
pub fn discourse_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageDiscourseRole",
            LanguageDiscourseRole::semantic_type().expect("checked discourse role")
        ),
        (
            "LanguageVocativeDiscourseAdmission",
            LanguageVocativeDiscourseAdmission::semantic_type()
                .expect("checked discourse admission")
        ),
        (
            "LanguageVocativeDiscourseFact",
            LanguageVocativeDiscourseFact::semantic_type().expect("checked discourse fact")
        ),
    ]
}

pub enum DiscourseDelta<'a> {
    Assert(&'a LanguageVocativeDiscourseFact),
    Withdraw(&'a LinguisticTokenIdentity),
}
/// One exact addressed occurrence consumes the existing bounded revision ledger;
/// withdrawal cannot name a different token. Stabilization,
/// commitment and withdrawal remain ledger events rather than inferred flags.
pub struct DiscourseRevisions<'a> {
    pub source: &'a LanguageTextRevision,
    pub subject: &'a LinguisticTokenIdentity,
    pub tokens: u64,
}
impl<'a> RevisionDomain for DiscourseRevisions<'a> {
    type Delta = DiscourseDelta<'a>;
    type Cursor = crate::revision::TokenFrontier;
    fn contract(&self) -> RevisionText<'_> {
        RevisionText::new("language/vocative-discourse-revision@1").expect("constant contract")
    }
    fn validate_delta(&self, role: RevisionDeltaRole, delta: &Self::Delta) -> bool {
        let token = match delta {
            DiscourseDelta::Assert(fact) if role != RevisionDeltaRole::Withdrawal => {
                if fact.source() != self.source || !valid_tokens(fact.basis(), self.tokens) {
                    return false;
                }
                fact.basis().dependent().token()
            }
            DiscourseDelta::Withdraw(token) if role == RevisionDeltaRole::Withdrawal => token,
            _ => return false,
        };
        self.tokens > 0
            && self.tokens <= 128
            && token == self.subject
            && *token.ordinal() < self.tokens
            && token.text_identity() == self.source.material().identity()
            && token.text_revision() == self.source.material().revision()
    }
    fn validate_cursor(&self, cursor: Self::Cursor) -> bool {
        self.tokens > 0 && self.tokens <= 128 && cursor.0 <= self.tokens
    }
    fn region(&self, delta: &Self::Delta) -> (Self::Cursor, Self::Cursor) {
        let token = match delta {
            DiscourseDelta::Assert(fact) => fact.basis().dependent().token(),
            DiscourseDelta::Withdraw(token) => token,
        };
        (
            crate::revision::TokenFrontier(*token.ordinal()),
            crate::revision::TokenFrontier(*token.ordinal() + 1),
        )
    }
    fn distance(&self, start: Self::Cursor, end: Self::Cursor) -> Option<u64> {
        end.0.checked_sub(start.0)
    }
}
