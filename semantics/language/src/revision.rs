//! Dependency-analysis consumer of the shared revision law. Native arcs and
//! exact source token identities remain language-owned; no parser lives here.
use crate::{LanguageDependencyArc, LanguageDependencyHead, LinguisticTokenIdentity};
use conduit_core::revision::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TokenFrontier(pub u64);

pub enum DependencyDelta {
    Assert(LanguageDependencyArc),
    Withdraw(LinguisticTokenIdentity),
}

/// One finite source-text epoch. Different analysis revisions retain their
/// native identities; token ordinals are never compared across source epochs.
pub struct DependencyRevisions<'a> {
    pub source_text: RevisionText<'a>,
    pub tokens: u64,
}
impl RevisionDomain for DependencyRevisions<'_> {
    type Delta = DependencyDelta;
    type Cursor = TokenFrontier;
    fn contract(&self) -> RevisionText<'_> {
        RevisionText::new("language/dependency-revision@1").expect("constant contract")
    }
    fn validate_delta(&self, role: RevisionDeltaRole, delta: &Self::Delta) -> bool {
        let token = match delta {
            DependencyDelta::Assert(arc) if role != RevisionDeltaRole::Withdrawal => {
                if let LanguageDependencyHead::Token(head) = arc.governor() {
                    if *head.token().ordinal() >= self.tokens {
                        return false;
                    }
                }
                arc.dependent().token()
            }
            DependencyDelta::Withdraw(token) if role == RevisionDeltaRole::Withdrawal => token,
            _ => return false,
        };
        self.tokens > 0
            && self.tokens <= 4096
            && token.text_identity().as_str() == self.source_text.as_str()
            && *token.ordinal() < self.tokens
    }
    fn validate_cursor(&self, cursor: TokenFrontier) -> bool {
        self.tokens > 0 && self.tokens <= 4096 && cursor.0 <= self.tokens
    }
    fn region(&self, delta: &DependencyDelta) -> (TokenFrontier, TokenFrontier) {
        let token = match delta {
            DependencyDelta::Assert(arc) => arc.dependent().token(),
            DependencyDelta::Withdraw(token) => token,
        };
        (
            TokenFrontier(*token.ordinal()),
            TokenFrontier(*token.ordinal() + 1),
        )
    }
    fn distance(&self, start: TokenFrontier, end: TokenFrontier) -> Option<u64> {
        end.0.checked_sub(start.0)
    }
}
