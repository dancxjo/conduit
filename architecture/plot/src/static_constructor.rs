//! Compile-time preparation through an explicitly supplied semantic owner.
use crate::prelude::*;
use crate::{ProfileCatalog, StartupCatalog};
use conduit_core::{
    ConfigurationEntry, ConfigurationValue, Kind, KindId, KindIdentity, PortTemporal,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
};

const MAXIMUM_CONFIGURATION_BYTES: usize = 1024 * 1024;

/// A compiled owner of ordinary immutable construction. A zero-input Kind alone
/// never grants this entrance. Hosts, runtime offers and Source metadata cannot
/// supply an implementation; domain owners implement it using their existing
/// portable preparation, with the same contract and finite result Type.
pub trait StaticValueConstructor {
    type Refusal;
    fn contract(&self) -> Kind;
    fn result_type(&self) -> StructuredInfoType;
    fn prepare_configuration(
        &self,
        configuration: &[ConfigurationEntry],
    ) -> Result<StructuredInfoValue, Self::Refusal>;
}

#[derive(Debug)]
pub enum StaticConstructorRefusal<E> {
    Contract,
    Configuration,
    ConfigurationLimit,
    OutputType,
    Structured(StructuredInfoRefusal),
    Owner(E),
}

/// Executed constructor admission, retaining its ordinary immutable arguments.
/// This receipt has no authored Source claim; lexical lowering retains those
/// identities separately when it supplies the same checked arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedStaticValue {
    constructor_kind: KindId,
    constructor_revision: KindIdentity,
    configuration: Vec<ConfigurationEntry>,
    value: StructuredInfoValue,
}

impl PreparedStaticValue {
    pub fn constructor_kind(&self) -> &KindId {
        &self.constructor_kind
    }
    pub fn constructor_revision(&self) -> &KindIdentity {
        &self.constructor_revision
    }
    pub fn configuration(&self) -> &[ConfigurationEntry] {
        &self.configuration
    }
    pub fn value(&self) -> &StructuredInfoValue {
        &self.value
    }
}

/// Admit the ordinary explicit constructor before any glyph elaboration can
/// claim equivalent Info. Exact installed Startup and canonical Kind truth,
/// finite configuration rules and exact output Type all precede the receipt.
pub fn prepare_static_constructor<C: StaticValueConstructor>(
    constructor: &C,
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
    configuration: &[ConfigurationEntry],
) -> Result<PreparedStaticValue, StaticConstructorRefusal<C::Refusal>> {
    use StaticConstructorRefusal as R;
    let kind = constructor.contract();
    let result = constructor.result_type();
    let expected_kind = match result.shape() {
        StructuredInfoTypeShape::Leaf(kind) => kind.clone(),
        _ => result
            .profile()
            .map_err(R::Structured)?
            .value_kind()
            .clone(),
    };
    let signature = startup
        .signature(kind.kind_id.as_str())
        .ok_or(R::Contract)?;
    if profile.canonical_kind(&kind.kind_id) != Some(&kind)
        || startup
            .canonical_startup_parameters(signature)
            .map_err(|_| R::Contract)?
            != kind.startup_parameters
        || !kind.inputs.is_empty()
        || kind.outputs.len() != 1
        || kind.outputs[0].temporal != PortTemporal::Value
        || kind.outputs[0].value_kind != expected_kind
        || kind.configuration.len() != kind.startup_parameters.len()
    {
        return Err(R::Contract);
    }
    if configuration.len() != kind.configuration.len() {
        return Err(R::Configuration);
    }
    let mut bytes = 0usize;
    for field in &kind.configuration {
        let mut entries = configuration.iter().filter(|entry| entry.key == field.key);
        let entry = entries.next().ok_or(R::Configuration)?;
        if entries.next().is_some() {
            return Err(R::Configuration);
        }
        let parameter = kind
            .startup_parameters
            .iter()
            .find(|parameter| parameter.name == field.key)
            .ok_or(R::Contract)?;
        if entry.value.semantic_kind() != parameter.value_type {
            return Err(R::Configuration);
        }
        crate::validate_configuration_value(field, &entry.value).map_err(|_| R::Configuration)?;
        let value_bytes = match &entry.value {
            ConfigurationValue::Structured(value) => value.canonical_value().len(),
            ConfigurationValue::Text(value) => value.len(),
            _ => core::mem::size_of::<ConfigurationValue>(),
        };
        bytes = bytes
            .saturating_add(entry.key.len())
            .saturating_add(value_bytes);
        if bytes > MAXIMUM_CONFIGURATION_BYTES {
            return Err(R::ConfigurationLimit);
        }
    }
    let value = constructor
        .prepare_configuration(configuration)
        .map_err(R::Owner)?;
    if value.value_type() != &result {
        return Err(R::OutputType);
    }
    // Revalidate the exact structural value and its finite codec bounds before
    // keeping it. Domain and Native predicate admission remain with the owner.
    let value =
        StructuredInfoValue::from_canonical_bytes(&value.canonical_bytes().map_err(R::Structured)?)
            .map_err(R::Structured)?;
    Ok(PreparedStaticValue {
        constructor_kind: kind.kind_id,
        constructor_revision: kind.kind_contract_revision,
        configuration: configuration.to_vec(),
        value,
    })
}

#[cfg(test)]
mod tests;
