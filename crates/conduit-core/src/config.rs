//! Typed configuration contracts, separate from live ports.

use core::convert::Infallible;
use core::fmt;

use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition, Id, MapField,
    SemanticHash, Sensitivity, TypeContractRef,
};

/// Whether a configuration field must be supplied or has missing-value meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigRequirement<'a> {
    /// The author or resolver must supply a value.
    Required,
    /// The field may be absent and has no value when absent.
    Optional,
    /// Absence resolves to this exact canonical public value.
    Defaulted(CanonicalValue<'a>),
}

/// Point in the lifecycle at which a configuration value may change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigMutability {
    /// Fixed before node start and immutable while running.
    PreStart,
    /// May change through an evidenced runtime configuration operation.
    Runtime,
}

impl ConfigMutability {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreStart => "pre-start",
            Self::Runtime => "runtime",
        }
    }
}

/// Identity layer in which an exact resolved value participates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigIdentity {
    /// A public value participates in semantic node identity.
    Semantic,
    /// The resolved value or secret binding is pinned by the execution plan.
    Plan,
}

impl ConfigIdentity {
    /// Stable descriptor spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::Plan => "plan",
        }
    }
}

/// One typed configuration field in a node contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigFieldContract<'a> {
    /// Stable key within the node contract.
    pub key: Id<'a>,
    /// Exact domain-owned value type.
    pub value_type: TypeContractRef<'a>,
    /// Required, optional, or canonically defaulted.
    pub requirement: ConfigRequirement<'a>,
    /// Public, restricted, or secret handling.
    pub sensitivity: Sensitivity,
    /// Pre-start-only or runtime mutable.
    pub mutability: ConfigMutability,
    /// Semantic descriptor identity or exact execution-plan identity.
    pub identity: ConfigIdentity,
}

impl ConfigFieldContract<'_> {
    /// Computes the exact canonical field-contract identity.
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let type_fields = [
            semantic(
                "contract_id",
                CanonicalValue::Identifier(self.value_type.contract_id),
            ),
            semantic(
                "schema_version",
                CanonicalValue::Integer(i128::from(self.value_type.schema_version)),
            ),
            semantic(
                "semantic_hash",
                CanonicalValue::Bytes(self.value_type.semantic_hash.as_bytes()),
            ),
        ];
        let (requirement, default, default_disposition) = match self.requirement {
            ConfigRequirement::Required => (
                "required",
                CanonicalValue::Null,
                FieldDisposition::Annotation,
            ),
            ConfigRequirement::Optional => (
                "optional",
                CanonicalValue::Null,
                FieldDisposition::Annotation,
            ),
            ConfigRequirement::Defaulted(value) => ("defaulted", value, FieldDisposition::Semantic),
        };
        let fields = [
            semantic("key", CanonicalValue::Identifier(self.key)),
            semantic("value_type", CanonicalValue::Map(&type_fields)),
            semantic("requirement", CanonicalValue::Identifier(Id(requirement))),
            MapField {
                name: Id("default"),
                value: default,
                disposition: default_disposition,
            },
            semantic(
                "sensitivity",
                CanonicalValue::Identifier(Id(self.sensitivity.as_str())),
            ),
            semantic(
                "mutability",
                CanonicalValue::Identifier(Id(self.mutability.as_str())),
            ),
            semantic(
                "identity",
                CanonicalValue::Identifier(Id(self.identity.as_str())),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/config-field-contract"),
            schema_version: 0,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

/// Typed configuration schema, distinct from patchable ports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigContract<'a> {
    /// Fields in stable contract order.
    pub fields: &'a [ConfigFieldContract<'a>],
}

impl<'a> ConfigContract<'a> {
    /// Validates portable field invariants in deterministic order.
    pub fn validate(&self) -> Result<(), ConfigContractError<'a>> {
        for (index, field) in self.fields.iter().enumerate() {
            if Id::new(field.key.as_str()).is_err() {
                return Err(ConfigContractError::InvalidKey(field.key));
            }
            if self.fields[..index]
                .iter()
                .any(|prior| prior.key == field.key)
            {
                return Err(ConfigContractError::DuplicateKey(field.key));
            }
            if field.value_type.validate().is_err() {
                return Err(ConfigContractError::InvalidTypeReference(field.key));
            }
            if field.sensitivity != Sensitivity::Public
                && matches!(field.requirement, ConfigRequirement::Defaulted(_))
            {
                return Err(ConfigContractError::SecretDefault(field.key));
            }
            if field.sensitivity != Sensitivity::Public
                && field.identity == ConfigIdentity::Semantic
            {
                return Err(ConfigContractError::SecretSemanticIdentity(field.key));
            }
        }
        Ok(())
    }

    /// Computes the order-independent canonical configuration-schema identity.
    ///
    /// The descriptor stores a canonical set of exact field-contract hashes.
    /// Reordering source fields therefore cannot change semantic identity.
    pub fn semantic_hash(&self) -> Result<SemanticHash, ConfigContractIdentityError<'a>> {
        use sha2::Digest as _;

        self.validate()
            .map_err(ConfigContractIdentityError::InvalidContract)?;
        let mut digest = sha2::Sha256::new();
        digest.update(crate::SEMANTIC_HASH_DOMAIN);
        digest.update(crate::CANONICAL_MAGIC);
        write_identifier_hash(&mut digest, "conduit/config-contract");
        digest.update(1_u32.to_be_bytes());
        digest.update([0x31]);
        digest.update(1_u64.to_be_bytes());
        write_identifier_hash(&mut digest, "fields");
        digest.update([0x32]);
        digest.update(
            u64::try_from(self.fields.len())
                .map_err(|_| ConfigContractIdentityError::LengthOverflow)?
                .to_be_bytes(),
        );

        for rank in 0..self.fields.len() {
            let mut selected = None;
            for candidate in self.fields {
                let candidate_hash = candidate.semantic_hash().map_err(|_| {
                    ConfigContractIdentityError::InvalidCanonicalField(candidate.key)
                })?;
                let mut preceding = 0;
                for other in self.fields {
                    let other_hash = other.semantic_hash().map_err(|_| {
                        ConfigContractIdentityError::InvalidCanonicalField(other.key)
                    })?;
                    if other_hash.as_bytes() < candidate_hash.as_bytes() {
                        preceding += 1;
                    }
                }
                if preceding == rank {
                    selected = Some(candidate_hash);
                    break;
                }
            }
            let hash = selected.ok_or(ConfigContractIdentityError::DuplicateFieldIdentity)?;
            digest.update([0x20]);
            digest.update(32_u64.to_be_bytes());
            digest.update(hash.as_bytes());
        }

        Ok(SemanticHash::from_bytes(digest.finalize().into()))
    }
}

fn write_identifier_hash(digest: &mut sha2::Sha256, value: &str) {
    use sha2::Digest as _;

    digest.update([0x22]);
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value.as_bytes());
}

/// Invalid typed configuration schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigContractError<'a> {
    /// A field key is outside the portable grammar.
    InvalidKey(Id<'a>),
    /// Two fields use the same stable key.
    DuplicateKey(Id<'a>),
    /// A field carries a malformed type reference.
    InvalidTypeReference(Id<'a>),
    /// Version 1 forbids inline defaults for protected fields.
    SecretDefault(Id<'a>),
    /// Protected material cannot participate directly in semantic hashes.
    SecretSemanticIdentity(Id<'a>),
}

impl fmt::Display for ConfigContractError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (code, key) = match self {
            Self::InvalidKey(key) => ("CND-CFG-001", key),
            Self::DuplicateKey(key) => ("CND-CFG-002", key),
            Self::InvalidTypeReference(key) => ("CND-CFG-003", key),
            Self::SecretDefault(key) => ("CND-CFG-004", key),
            Self::SecretSemanticIdentity(key) => ("CND-CFG-005", key),
        };
        write!(formatter, "{code}: invalid configuration field `{key}`")
    }
}

/// Configuration-schema identity failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigContractIdentityError<'a> {
    /// The configuration contract is structurally invalid.
    InvalidContract(ConfigContractError<'a>),
    /// A field default is not valid canonical form.
    InvalidCanonicalField(Id<'a>),
    /// Two fields unexpectedly produced one exact identity.
    DuplicateFieldIdentity,
    /// The field count cannot fit canonical form version 1.
    LengthOverflow,
}

impl fmt::Display for ConfigContractIdentityError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidContract(error) => error.fmt(formatter),
            Self::InvalidCanonicalField(key) => {
                write!(formatter, "configuration field `{key}` is not canonical")
            }
            Self::DuplicateFieldIdentity => {
                formatter.write_str("configuration fields have duplicate semantic identity")
            }
            Self::LengthOverflow => {
                formatter.write_str("configuration field count exceeds canonical form")
            }
        }
    }
}
