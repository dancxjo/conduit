//! Authored Host construction data; no runtime grants or Ready claims.
use crate::{HostBounds, ResourceBudget};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostConfiguration {
    pub schema: u32,
    pub name: String,
    pub target: ConfigurationTarget,
    #[serde(default)]
    pub bases: Vec<ConfigurationBase>,
    #[serde(default)]
    pub resources: Vec<ResourceBudget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<crate::CapabilityIntent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preparation: Option<conduit_body::HostPreparationDelegation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<conduit_body::HostExecutionDelegation>,
    pub limits: HostBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationTarget {
    pub architecture: String,
    pub machine: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub make_descriptor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationBase {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization: Option<crate::BaseInitialization>,
}

impl ConfigurationBase {
    pub(super) fn preferences(&self) -> Vec<&str> {
        self.implementation
            .iter()
            .map(String::as_str)
            .chain(self.implementations.iter().map(String::as_str))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigurationDiagnostic {
    Capability {
        detail: String,
    },
    Decode {
        detail: String,
    },
    UnsupportedSchema {
        found: u32,
    },
    InvalidName,
    PreparationDelegation {
        detail: String,
    },
    Initialization {
        kind: String,
        detail: String,
    },
    UnknownTarget {
        target: String,
    },
    UnknownBase {
        kind: String,
    },
    MissingImplementation {
        kind: String,
    },
    ContradictoryImplementations {
        kind: String,
    },
    UnknownImplementation {
        implementation: String,
    },
    UnsupportedBase {
        kind: String,
        target: String,
    },
    IncompatibleImplementation {
        implementation: String,
        target: String,
    },
    DuplicateContradictoryBase {
        kind: String,
    },
    ConflictingResourceAssignment {
        resource: String,
    },
    AmbiguousFaceRealization {
        implementations: Vec<String>,
    },
    DuplicateResource {
        id: String,
    },
    UnboundedCapacity {
        field: &'static str,
    },
    LimitExceeded {
        field: &'static str,
        requested: u64,
        maximum: u64,
    },
    Profile {
        detail: String,
    },
    Encode {
        detail: String,
    },
}
