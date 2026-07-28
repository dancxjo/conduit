//! Opaque references to domain-owned type contracts.
//!
//! The portable core retains only exact identity. Hosted registries discover
//! descriptors and ask domain providers directional compatibility questions.

use core::fmt;

use crate::{Id, SemanticHash};

/// An exact reference to a domain-owned type contract.
///
/// The contract identifier is namespaced, but its namespace and meaning remain
/// opaque to `conduit-core`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeContractRef<'a> {
    /// Stable namespaced contract identifier.
    pub contract_id: Id<'a>,
    /// Exact domain-owned contract schema revision.
    pub schema_version: u32,
    /// Exact canonical semantic identity of the contract descriptor.
    pub semantic_hash: SemanticHash,
}

impl<'a> TypeContractRef<'a> {
    /// Validates the portable reference shape.
    pub fn validate(self) -> Result<(), TypeContractRefError> {
        Id::new(self.contract_id.as_str())
            .map_err(|_| TypeContractRefError::InvalidContractIdentifier)?;
        if self.contract_id.as_str().split_once('/').is_none() {
            return Err(TypeContractRefError::MissingNamespace);
        }
        Ok(())
    }

    /// Returns the namespace that selects the hosted domain provider.
    pub fn namespace(self) -> Result<Id<'a>, TypeContractRefError> {
        self.validate()?;
        let (namespace, _) = self
            .contract_id
            .as_str()
            .split_once('/')
            .ok_or(TypeContractRefError::MissingNamespace)?;
        Id::new(namespace).map_err(|_| TypeContractRefError::InvalidContractIdentifier)
    }
}

/// Invalid opaque type-contract reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeContractRefError {
    /// The identifier does not use the portable identifier grammar.
    InvalidContractIdentifier,
    /// The contract identifier has no provider-selecting namespace.
    MissingNamespace,
}

impl fmt::Display for TypeContractRefError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContractIdentifier => {
                formatter.write_str("type contract identifier is invalid")
            }
            Self::MissingNamespace => {
                formatter.write_str("type contract identifier is not namespaced")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: SemanticHash = SemanticHash::from_bytes([0x11; 32]);

    #[test]
    fn accepts_an_opaque_namespaced_reference() {
        let reference = TypeContractRef {
            contract_id: Id("example/record"),
            schema_version: 3,
            semantic_hash: HASH,
        };

        assert_eq!(reference.validate(), Ok(()));
        assert_eq!(reference.namespace(), Ok(Id("example")));
    }

    #[test]
    fn rejects_missing_or_malformed_namespaces() {
        let local = TypeContractRef {
            contract_id: Id("record"),
            schema_version: 1,
            semantic_hash: HASH,
        };
        assert_eq!(
            local.validate(),
            Err(TypeContractRefError::MissingNamespace)
        );

        let malformed = TypeContractRef {
            contract_id: Id("Example/record"),
            ..local
        };
        assert_eq!(
            malformed.validate(),
            Err(TypeContractRefError::InvalidContractIdentifier)
        );
    }
}
