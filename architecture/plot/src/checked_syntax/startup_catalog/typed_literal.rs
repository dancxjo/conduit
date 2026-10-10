//! Finite imported notation metadata. No parser callbacks or runtime offers.
use super::*;
use conduit_core::{KindId, KindIdentity, PortTemporal, SourceDocumentId, StructuredInfoType};

mod scan;
pub use scan::{ScannedTypedLiteral, TypedLiteralScanRefusal};

#[cfg(test)]
mod tests;

pub const MAXIMUM_TYPED_LITERAL_FAMILIES: usize = 64;
pub const MAXIMUM_TYPED_LITERAL_BRANCHES: usize = 4;
pub const MAXIMUM_TYPED_LITERAL_PAYLOAD_BYTES: usize = 4096;
const MAXIMUM_IDENTITY_BYTES: usize = 256;
const MAXIMUM_REGISTRY_BYTES: usize = 1024 * 1024;

/// Fixed language-reviewed pairs; metadata cannot install arbitrary punctuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypedLiteralDelimiter {
    Square,
    Slash,
    Angle,
    DoubleSquare,
}
impl TypedLiteralDelimiter {
    pub fn pair(self) -> (char, char) {
        match self {
            Self::Square => ('[', ']'),
            Self::Slash => ('/', '/'),
            Self::Angle => ('⟨', '⟩'),
            Self::DoubleSquare => ('⟦', '⟧'),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedLiteralLexicalPolicy {
    /// Backslash parity quotes closers; raw bytes go to the domain constructor.
    RawUnicode,
    /// Existing portable pattern class, escape and empty/i flag policy.
    PortablePattern,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedLiteralFamilyOrigin {
    pub package_content_digest: [u8; 32],
    pub module_path: String,
    pub source_document_id: SourceDocumentId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedLiteralBranch {
    pub delimiter: TypedLiteralDelimiter,
    pub lexical_policy: TypedLiteralLexicalPolicy,
    pub parser_contract: String,
    pub constructor_kind: KindId,
    pub constructor_revision: KindIdentity,
    pub result_type: StructuredInfoType,
    pub maximum_payload_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedLiteralFamily {
    pub revision: String,
    pub origin: TypedLiteralFamilyOrigin,
    pub branches: Vec<TypedLiteralBranch>,
}

impl TypedLiteralFamily {
    pub fn branch(&self, delimiter: TypedLiteralDelimiter) -> Option<&TypedLiteralBranch> {
        self.branches
            .iter()
            .find(|branch| branch.delimiter == delimiter)
    }

    /// Versioned checked metadata, independent of local alias and host pointers.
    pub fn identity_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_metadata()?;
        let mut bytes = b"conduitese/typed-literal-family@1\0".to_vec();
        field(&mut bytes, self.revision.as_bytes());
        bytes.extend_from_slice(&self.origin.package_content_digest);
        field(&mut bytes, self.origin.module_path.as_bytes());
        field(
            &mut bytes,
            self.origin.source_document_id.as_str().as_bytes(),
        );
        let mut branches: Vec<_> = self.branches.iter().collect();
        branches.sort_by_key(|branch| branch.delimiter.pair());
        for branch in branches {
            let (open, close) = branch.delimiter.pair();
            bytes.extend_from_slice(&(open as u32).to_le_bytes());
            bytes.extend_from_slice(&(close as u32).to_le_bytes());
            bytes.push(match branch.lexical_policy {
                TypedLiteralLexicalPolicy::RawUnicode => 0,
                TypedLiteralLexicalPolicy::PortablePattern => 1,
            });
            field(&mut bytes, branch.parser_contract.as_bytes());
            field(&mut bytes, branch.constructor_kind.as_str().as_bytes());
            field(&mut bytes, branch.constructor_revision.as_str().as_bytes());
            field(
                &mut bytes,
                &branch
                    .result_type
                    .canonical_bytes()
                    .map_err(|e| format!("{e:?}"))?,
            );
            bytes.extend_from_slice(&(branch.maximum_payload_bytes as u32).to_le_bytes());
        }
        Ok(bytes)
    }

    fn validate_metadata(&self) -> Result<(), String> {
        if !identity(&self.revision)
            || !identity(&self.origin.module_path)
            || self.origin.source_document_id.as_str().len() != 64
            || !self
                .origin
                .source_document_id
                .as_str()
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.branches.is_empty()
            || self.branches.len() > MAXIMUM_TYPED_LITERAL_BRANCHES
        {
            return Err("invalid or unbounded typed literal family identity".into());
        }
        let mut keys = alloc::collections::BTreeSet::new();
        for branch in &self.branches {
            if !keys.insert(branch.delimiter)
                || !identity(&branch.parser_contract)
                || !identity(branch.constructor_kind.as_str())
                || !identity(branch.constructor_revision.as_str())
                || branch.maximum_payload_bytes == 0
                || branch.maximum_payload_bytes > MAXIMUM_TYPED_LITERAL_PAYLOAD_BYTES
            {
                return Err("duplicate or unbounded typed literal branch".into());
            }
            if branch.lexical_policy == TypedLiteralLexicalPolicy::PortablePattern
                && !matches!(
                    branch.delimiter,
                    TypedLiteralDelimiter::Slash | TypedLiteralDelimiter::DoubleSquare
                )
            {
                return Err(
                    "portable pattern policy requires a reviewed slash or double-square branch"
                        .into(),
                );
            }
            branch
                .result_type
                .profile()
                .map_err(|e| format!("invalid literal result Type: {e:?}"))?;
        }
        Ok(())
    }
}

impl StartupCatalog {
    pub(crate) fn installed_literal_families(
        &self,
    ) -> impl Iterator<Item = (&str, &TypedLiteralFamily)> {
        self.typed_literal_families
            .iter()
            .map(|(path, family)| (path.as_str(), family))
    }

    pub fn typed_literal_family(&self, path: &str) -> Option<&TypedLiteralFamily> {
        self.typed_literal_families.get(path)
    }

    /// Register one exact family against installed ordinary constructor truth.
    /// Admission is transactional; grammar recognition and owner parsing are
    /// separate preparation steps and never selected by expected result Type.
    pub fn insert_typed_literal_family(
        &mut self,
        path: impl Into<String>,
        family: TypedLiteralFamily,
        profile: &crate::ProfileCatalog,
    ) -> Result<(), String> {
        let path = path.into();
        if path.len() > MAXIMUM_IDENTITY_BYTES
            || !crate::surface_lex::is_source_import_path(&path)
            || self.typed_literal_families.contains_key(&path)
            || self.kinds.contains_key(&path)
            || self.structured_types.contains_key(&path)
            || self.value_kind_aliases.contains_key(&path)
            || self.native_families.contains_key(&path)
        {
            return Err("duplicate, ambiguous or invalid typed literal family path".into());
        }
        family.validate_metadata()?;
        for branch in &family.branches {
            let signature = self
                .signature(branch.constructor_kind.as_str())
                .ok_or("typed literal constructor startup contract is not installed")?;
            let kind = profile
                .canonical_kind(&branch.constructor_kind)
                .ok_or("typed literal constructor semantic contract is not installed")?;
            let parameters = self
                .canonical_startup_parameters(signature)
                .map_err(|e| format!("literal constructor parameters: {e:?}"))?;
            let result_kind = match branch.result_type.shape() {
                conduit_core::StructuredInfoTypeShape::Leaf(kind) => kind.clone(),
                _ => branch
                    .result_type
                    .profile()
                    .map_err(|e| format!("{e:?}"))?
                    .value_kind()
                    .clone(),
            };
            if kind.kind_contract_revision != branch.constructor_revision
                || kind.startup_parameters != parameters
                || !kind.inputs.is_empty()
                || kind.outputs.len() != 1
                || kind.outputs[0].temporal != PortTemporal::Value
                || kind.outputs[0].value_kind != result_kind
            {
                return Err(
                    "typed literal branch differs from its exact ordinary constructor".into(),
                );
            }
        }
        let added = path.len().saturating_add(family.identity_bytes()?.len());
        let mut total = added;
        for (name, existing) in &self.typed_literal_families {
            total = total
                .saturating_add(name.len())
                .saturating_add(existing.identity_bytes()?.len());
        }
        if self.typed_literal_families.len() >= MAXIMUM_TYPED_LITERAL_FAMILIES
            || total > MAXIMUM_REGISTRY_BYTES
        {
            return Err("typed literal family registry exceeds its finite profile".into());
        }
        self.typed_literal_families.insert(path, family);
        Ok(())
    }
}

fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_IDENTITY_BYTES
        && value.bytes().all(|byte| byte.is_ascii_graphic())
}
fn field(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
    bytes.extend_from_slice(value);
}
