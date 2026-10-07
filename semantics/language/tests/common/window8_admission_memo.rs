//! Unselected prototype: finite immutable custody, no graph-policy implementation.
use conduit_language::{LanguageParserWindow8RawState, LanguageParserWindow8StateProof};
use conduit_plot::rust_binding::NativeRustBinding;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    programs: Vec<Vec<u8>>,
    dependencies: Vec<Vec<u8>>,
    profile: Vec<u8>,
}
impl Scope {
    pub fn new(
        programs: Vec<Vec<u8>>,
        dependencies: Vec<Vec<u8>>,
        profile: Vec<u8>,
    ) -> Result<Self, String> {
        if programs.is_empty() || dependencies.is_empty() || profile.is_empty() {
            return Err("exact scope required".into());
        }
        Ok(Self {
            programs,
            dependencies,
            profile,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    state: Vec<u8>,
    carrier: Vec<u8>,
}
struct Entry {
    key: Key,
    proof: LanguageParserWindow8StateProof,
}
pub struct Memo<F> {
    scope: Scope,
    admit: F,
    maximum: usize,
    entries: Vec<Entry>,
}
impl<F> Memo<F>
where
    F: FnMut(&LanguageParserWindow8RawState) -> Result<LanguageParserWindow8StateProof, String>,
{
    pub fn new(scope: Scope, maximum: usize, admit: F) -> Result<Self, String> {
        if !(1..=1024).contains(&maximum) {
            return Err("finite memo profile is 1..=1024".into());
        }
        Ok(Self {
            scope,
            admit,
            maximum,
            entries: Vec::new(),
        })
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn admit(
        &mut self,
        state: &LanguageParserWindow8RawState,
        carrier: Vec<u8>,
    ) -> Result<LanguageParserWindow8StateProof, String> {
        let key = Key {
            state: state.clone().encode().map_err(|e| format!("{e:?}"))?,
            carrier,
        };
        if let Some(entry) = self.entries.iter().find(|entry| entry.key == key) {
            return Ok(entry.proof.clone());
        }
        // The immutable owned Source engine performs every walk and native law.
        let proof = (self.admit)(state)?;
        if proof.state() != state {
            return Err("Source receipt does not match whole state".into());
        }
        // Constructors have already checked native laws; re-admit exact bytes
        // before custody to refuse a producer returning mismatched framing.
        let proof = LanguageParserWindow8StateProof::decode(
            &proof.clone().encode().map_err(|e| format!("{e:?}"))?,
        )
        .map_err(|e| format!("{e:?}"))?;
        if self.entries.len() == self.maximum {
            self.entries.remove(0);
        }
        self.entries.push(Entry {
            key,
            proof: proof.clone(),
        });
        Ok(proof)
    }
}
