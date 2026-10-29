//! Checked provider initialization intent. This module performs no discovery,
//! initialization, capture, authority grant, or runtime offer publication.
use crate::{ConfigurationBase, MakeCatalog};
use conduit_core::{ConfigurationValue, KindConfigurationField, KindConfigurationRule};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAXIMUM_FIELDS: usize = 16;
const MAXIMUM_SOURCE_BYTES: usize = 16 * 1024;

/// Source values are untrusted until checked against the selected package's
/// finite fields. Objects, arrays, null and floating-point values are refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BaseInitialization {
    pub values: BTreeMap<String, serde_json::Value>,
}

/// Package-owned initialization contract. Every field is explicitly required;
/// startup defaults are not authority to select equipment implicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseInitializationContract {
    pub fields: Vec<KindConfigurationField>,
}

impl BaseInitialization {
    pub(crate) fn validate_source(&self) -> Result<(), String> {
        if self.values.is_empty() || self.values.len() > MAXIMUM_FIELDS {
            return Err("invalid initialization field count".into());
        }
        for (key, value) in &self.values {
            if !valid_key(key)
                || !(value.is_string() || value.is_boolean() || value.as_u64().is_some())
            {
                return Err("invalid initialization field name or primitive value".into());
            }
        }
        if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > MAXIMUM_SOURCE_BYTES {
            return Err("initialization exceeds its finite byte bound".into());
        }
        Ok(())
    }
    pub fn checked_values(
        &self,
        contract: &BaseInitializationContract,
    ) -> Result<BTreeMap<String, ConfigurationValue>, String> {
        if self.values.is_empty()
            || self.values.len() > MAXIMUM_FIELDS
            || contract.fields.len() > MAXIMUM_FIELDS
        {
            return Err("initialization field count exceeds its finite contract".into());
        }
        contract.validate()?;
        self.validate_source()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAXIMUM_SOURCE_BYTES {
            return Err("initialization exceeds its finite byte bound".into());
        }
        let mut result = BTreeMap::new();
        for field in &contract.fields {
            let value = self
                .values
                .get(&field.key)
                .ok_or_else(|| format!("missing initialization field '{}'", field.key))?;
            let value = match value {
                serde_json::Value::String(text) => ConfigurationValue::Text(text.clone()),
                serde_json::Value::Bool(value) => ConfigurationValue::Bool(*value),
                serde_json::Value::Number(value) => ConfigurationValue::U64(
                    value
                        .as_u64()
                        .ok_or("initialization requires unsigned integers")?,
                ),
                _ => return Err("initialization requires primitive typed values".into()),
            };
            conduit_plot::validate_configuration_value(field, &value).map_err(|e| e.message)?;
            if result.insert(field.key.clone(), value).is_some() {
                return Err("duplicate initialization contract field".into());
            }
        }
        if result.len() != self.values.len() {
            return Err("unknown initialization field".into());
        }
        Ok(result)
    }
}

pub(crate) fn check(
    base: &ConfigurationBase,
    implementation: &str,
    catalog: &MakeCatalog,
) -> Result<(), String> {
    let Some(initialization) = &base.initialization else {
        return Ok(());
    };
    let contract = catalog
        .initialization_contracts
        .get(implementation)
        .ok_or("selected implementation has no initialization contract")?;
    initialization.checked_values(contract).map(|_| ())
}

#[cfg(test)]
mod tests;

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
impl BaseInitializationContract {
    pub fn validate(&self) -> Result<(), String> {
        let mut keys = std::collections::BTreeSet::new();
        if self.fields.is_empty() || self.fields.len() > MAXIMUM_FIELDS {
            return Err("invalid initialization contract field count".into());
        }
        for field in &self.fields {
            if !valid_key(&field.key) || !keys.insert(&field.key) {
                return Err("invalid or duplicate initialization contract field".into());
            }
            let bounded = match &field.rule {
                KindConfigurationRule::U64Range { minimum, maximum }
                | KindConfigurationRule::DurationMillis { minimum, maximum } => minimum <= maximum,
                KindConfigurationRule::TextBytes { maximum }
                | KindConfigurationRule::DataLabel { maximum } => {
                    *maximum > 0 && *maximum as usize <= MAXIMUM_SOURCE_BYTES
                }
                KindConfigurationRule::TextOneOf { values } => {
                    !values.is_empty()
                        && values.len() <= MAXIMUM_FIELDS
                        && values
                            .iter()
                            .all(|v| !v.is_empty() && v.len() <= MAXIMUM_SOURCE_BYTES)
                }
                _ => false,
            };
            if !bounded {
                return Err(
                    "initialization contract requires a supported finite typed rule".into(),
                );
            }
        }
        Ok(())
    }
}
pub(crate) fn check_unique(
    bases: &[ConfigurationBase],
) -> Result<(), crate::ConfigurationDiagnostic> {
    let mut selections = BTreeMap::new();
    for base in bases {
        if let Some(prior) = selections.insert(&base.kind, &base.initialization) {
            if prior != &base.initialization {
                return Err(crate::ConfigurationDiagnostic::Initialization {
                    kind: base.kind.clone(),
                    detail: "contradictory initialization for one Base".into(),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_contribution(
    contribution: &crate::MakeContribution,
) -> Vec<crate::PackageCompositionDiagnostic> {
    let mut diagnostics = Vec::new();
    for (implementation, contract) in &contribution.catalog().initialization_contracts {
        let result = if contribution
            .offers()
            .iter()
            .any(|offer| &offer.implementation_id == implementation)
        {
            contract.validate()
        } else {
            Err("initialization contract must be owned by its implementation package".into())
        };
        if let Err(detail) = result {
            diagnostics.push(
                crate::PackageCompositionDiagnostic::InitializationContract {
                    package_id: contribution.package_id().into(),
                    implementation_id: implementation.clone(),
                    detail,
                },
            );
        }
    }
    diagnostics
}
