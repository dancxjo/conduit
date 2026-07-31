//! Hosted typed configuration resolution and secret-safe diagnostics.

use std::convert::Infallible;
use std::fmt;

use conduit_core::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, CompatibilityOutcome, ConfigContract,
    ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement, FieldDisposition, Id,
    MapField, SemanticHash, Sensitivity, TypeContractRef,
};

use crate::TypeRegistry;

/// Sensitive configuration bytes whose ordinary formatting is always redacted.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SecretValue<'a>(&'a str);

impl<'a> SecretValue<'a> {
    /// Wraps a value that must not appear in diagnostics or debug output.
    #[must_use]
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }

    /// Explicitly exposes the value to an authorized implementation boundary.
    #[must_use]
    pub const fn expose_secret(self) -> &'a str {
        self.0
    }
}

impl fmt::Debug for SecretValue<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretValue<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

/// One typed source or host-provided configuration value.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum ConfigValue<'a> {
    /// Canonical public data.
    Public(CanonicalValue<'a>),
    /// Restricted data, redacted by ordinary formatting.
    Restricted(SecretValue<'a>),
    /// Secret data, redacted by ordinary formatting.
    Secret(SecretValue<'a>),
}

impl<'a> ConfigValue<'a> {
    const fn sensitivity(self) -> Sensitivity {
        match self {
            Self::Public(_) => Sensitivity::Public,
            Self::Restricted(_) => Sensitivity::Restricted,
            Self::Secret(_) => Sensitivity::Secret,
        }
    }

    const fn public_value(self) -> Option<CanonicalValue<'a>> {
        match self {
            Self::Public(value) => Some(value),
            Self::Restricted(_) | Self::Secret(_) => None,
        }
    }
}

impl fmt::Debug for ConfigValue<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public(value) => formatter.debug_tuple("Public").field(value).finish(),
            Self::Restricted(_) => formatter.write_str("Restricted([REDACTED])"),
            Self::Secret(_) => formatter.write_str("Secret([REDACTED])"),
        }
    }
}

/// One key, exact type, and value supplied for configuration resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigAssignment<'a> {
    /// Stable field key.
    pub key: Id<'a>,
    /// Exact type of the supplied value.
    pub value_type: TypeContractRef<'a>,
    /// Public or protected value.
    pub value: ConfigValue<'a>,
}

/// One resolved field, including whether absence supplied the canonical default.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedConfigEntry<'a> {
    /// Exact field contract.
    pub field: &'a ConfigFieldContract<'a>,
    /// Resolved explicit or default value.
    pub value: ConfigValue<'a>,
    /// True when the value came from missing-value defaulting.
    pub defaulted: bool,
}

/// Fully validated pre-start configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConfig<'a> {
    entries: Vec<ResolvedConfigEntry<'a>>,
}

impl<'a> ResolvedConfig<'a> {
    /// Returns fields in stable contract order.
    #[must_use]
    pub fn entries(&self) -> &[ResolvedConfigEntry<'a>] {
        &self.entries
    }

    /// Looks up one resolved field.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&ResolvedConfigEntry<'a>> {
        self.entries
            .iter()
            .find(|entry| entry.field.key.as_str() == key)
    }

    /// Hashes only public semantic-identity values with canonical default omission.
    ///
    /// Plan-identity fields and protected values are deliberately absent. A
    /// later exact execution plan pins those values or secret bindings.
    pub fn semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let fields = self
            .entries
            .iter()
            .filter(|entry| entry.field.identity == ConfigIdentity::Semantic)
            .map(|entry| {
                let value = entry
                    .value
                    .public_value()
                    .expect("validated semantic configuration is public");
                let disposition = match &entry.field.requirement {
                    ConfigRequirement::Defaulted(default) => FieldDisposition::Defaulted(default),
                    ConfigRequirement::Required | ConfigRequirement::Optional => {
                        FieldDisposition::Semantic
                    }
                };
                MapField {
                    name: entry.field.key,
                    value,
                    disposition,
                }
            })
            .collect::<Vec<_>>();
        CanonicalDescriptor {
            kind: Id("conduit/config-values"),
            schema_version: 0,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
    }
}

/// Typed configuration resolution failure without carried value material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigResolutionError<'a> {
    /// The field schema itself is invalid.
    InvalidContract(Id<'a>),
    /// One key was supplied more than once.
    DuplicateAssignment(Id<'a>),
    /// No field exists for the supplied key.
    UnknownField(Id<'a>),
    /// A required field has no value.
    MissingRequired(Id<'a>),
    /// The supplied type is directionally incompatible.
    TypeMismatch(Id<'a>),
    /// A provider is required to determine the supplied type.
    TypeIndeterminate(Id<'a>),
    /// Supplied sensitivity exceeds the field boundary.
    SensitivityViolation(Id<'a>),
    /// A pre-start field was targeted by a runtime update.
    PreStartMutation(Id<'a>),
}

impl<'a> ConfigResolutionError<'a> {
    /// Stable machine-readable diagnostic code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidContract(_) => "CND-CFG-001",
            Self::DuplicateAssignment(_) => "CND-CFG-006",
            Self::UnknownField(_) => "CND-CFG-007",
            Self::MissingRequired(_) => "CND-CFG-008",
            Self::TypeMismatch(_) => "CND-CFG-009",
            Self::TypeIndeterminate(_) => "CND-CFG-010",
            Self::SensitivityViolation(_) => "CND-CFG-011",
            Self::PreStartMutation(_) => "CND-CFG-012",
        }
    }

    const fn key(self) -> Id<'a> {
        match self {
            Self::InvalidContract(key)
            | Self::DuplicateAssignment(key)
            | Self::UnknownField(key)
            | Self::MissingRequired(key)
            | Self::TypeMismatch(key)
            | Self::TypeIndeterminate(key)
            | Self::SensitivityViolation(key)
            | Self::PreStartMutation(key) => key,
        }
    }
}

impl fmt::Display for ConfigResolutionError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: configuration field `{}` is invalid",
            self.code(),
            self.key()
        )
    }
}

impl std::error::Error for ConfigResolutionError<'_> {}

/// Resolves a complete pre-start configuration in contract order.
pub fn resolve_config<'a>(
    registry: &TypeRegistry,
    contract: ConfigContract<'a>,
    assignments: &'a [ConfigAssignment<'a>],
) -> Result<ResolvedConfig<'a>, ConfigResolutionError<'a>> {
    if let Err(error) = contract.validate() {
        let key = match error {
            conduit_core::ConfigContractError::InvalidKey(key)
            | conduit_core::ConfigContractError::DuplicateKey(key)
            | conduit_core::ConfigContractError::InvalidTypeReference(key)
            | conduit_core::ConfigContractError::SecretDefault(key)
            | conduit_core::ConfigContractError::SecretSemanticIdentity(key) => key,
        };
        return Err(ConfigResolutionError::InvalidContract(key));
    }

    for (index, assignment) in assignments.iter().enumerate() {
        if assignments[..index]
            .iter()
            .any(|prior| prior.key == assignment.key)
        {
            return Err(ConfigResolutionError::DuplicateAssignment(assignment.key));
        }
        if !contract
            .fields
            .iter()
            .any(|field| field.key == assignment.key)
        {
            return Err(ConfigResolutionError::UnknownField(assignment.key));
        }
    }

    let mut entries = Vec::with_capacity(contract.fields.len());
    for field in contract.fields {
        let assignment = assignments
            .iter()
            .find(|assignment| assignment.key == field.key);
        let (value, defaulted) = match (assignment, field.requirement) {
            (Some(assignment), _) => {
                validate_assignment(registry, field, assignment)?;
                (assignment.value, false)
            }
            (None, ConfigRequirement::Required) => {
                return Err(ConfigResolutionError::MissingRequired(field.key));
            }
            (None, ConfigRequirement::Optional) => continue,
            (None, ConfigRequirement::Defaulted(default)) => (ConfigValue::Public(default), true),
        };
        entries.push(ResolvedConfigEntry {
            field,
            value,
            defaulted,
        });
    }

    Ok(ResolvedConfig { entries })
}

/// Validates one evidenced runtime update without applying it.
pub fn validate_config_update<'a>(
    registry: &TypeRegistry,
    contract: ConfigContract<'a>,
    assignment: &'a ConfigAssignment<'a>,
) -> Result<(), ConfigResolutionError<'a>> {
    let Some(field) = contract
        .fields
        .iter()
        .find(|field| field.key == assignment.key)
    else {
        return Err(ConfigResolutionError::UnknownField(assignment.key));
    };
    if field.mutability == ConfigMutability::PreStart {
        return Err(ConfigResolutionError::PreStartMutation(field.key));
    }
    validate_assignment(registry, field, assignment)
}

fn validate_assignment<'a>(
    registry: &TypeRegistry,
    field: &'a ConfigFieldContract<'a>,
    assignment: &'a ConfigAssignment<'a>,
) -> Result<(), ConfigResolutionError<'a>> {
    let decision = registry.consumer_accepts_producer(field.value_type, assignment.value_type);
    match decision.outcome {
        CompatibilityOutcome::Compatible => {}
        CompatibilityOutcome::Incompatible => {
            return Err(ConfigResolutionError::TypeMismatch(field.key));
        }
        CompatibilityOutcome::Indeterminate => {
            return Err(ConfigResolutionError::TypeIndeterminate(field.key));
        }
    }
    if assignment.value.sensitivity() > field.sensitivity {
        return Err(ConfigResolutionError::SensitivityViolation(field.key));
    }
    if field.identity == ConfigIdentity::Semantic && assignment.value.public_value().is_none() {
        return Err(ConfigResolutionError::SensitivityViolation(field.key));
    }
    Ok(())
}
