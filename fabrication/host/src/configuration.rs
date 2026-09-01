use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    validate_profile, BaseSelection, DriverSelection, FabricationCatalog, FabricationPackageSet,
    HostBounds, HostPolicy, HostProfile, PresenterSelection, ResourceBudget, TargetSelection,
    HOST_PROFILE_SCHEMA,
};

pub const HOST_CONFIGURATION_SCHEMA: u32 = 1;

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
    pub fabrication_descriptor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigurationBase {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementations: Vec<String>,
}

impl ConfigurationBase {
    fn preferences(&self) -> Vec<&str> {
        self.implementation
            .iter()
            .map(String::as_str)
            .chain(self.implementations.iter().map(String::as_str))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigurationDiagnostic {
    Decode {
        detail: String,
    },
    UnsupportedSchema {
        found: u32,
    },
    InvalidName,
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

#[derive(Debug, Clone)]
pub struct CheckedHostConfiguration {
    configuration: HostConfiguration,
    profile: HostProfile,
    configuration_id: String,
    resolved_bases: Vec<(String, String)>,
}

impl CheckedHostConfiguration {
    pub fn configuration(&self) -> &HostConfiguration {
        &self.configuration
    }
    pub fn profile(&self) -> &HostProfile {
        &self.profile
    }
    pub fn into_profile(self) -> HostProfile {
        self.profile
    }
    pub fn configuration_id(&self) -> &str {
        &self.configuration_id
    }
    pub fn resolved_bases(&self) -> &[(String, String)] {
        &self.resolved_bases
    }
}

// Configuration identity predates the canonical Conduit construction source.
// Keep this encoding private so accepted identities remain stable without
// retaining TOML as a parseable or authorable construction format.
fn stable_host_configuration_identity_source(
    configuration: &HostConfiguration,
) -> Result<String, ConfigurationDiagnostic> {
    let mut canonical = configuration.clone();
    canonical
        .bases
        .sort_by(|left, right| left.kind.cmp(&right.kind));
    canonical
        .resources
        .sort_by(|left, right| left.id.cmp(&right.id));
    toml::to_string_pretty(&canonical).map_err(|error| ConfigurationDiagnostic::Encode {
        detail: error.to_string(),
    })
}

pub fn check_host_configuration(
    configuration: HostConfiguration,
    catalog: &FabricationCatalog,
    packages: &FabricationPackageSet,
) -> Result<CheckedHostConfiguration, Vec<ConfigurationDiagnostic>> {
    let mut configuration = configuration;
    // Schema-1 descriptions may retain the retired deployment-role machine
    // labels. Migrate them at the checked-description boundary; the canonical
    // catalog and emitted identity contain only the hosted computer target.
    if configuration.target.architecture == "x86_64"
        && configuration.target.os.as_deref() == Some("linux")
        && configuration.target.board.is_none()
        && matches!(
            configuration.target.machine.as_str(),
            "workstation" | "server"
        )
    {
        configuration.target.machine = "computer".into();
    }
    let mut diagnostics = Vec::new();
    if configuration.schema != HOST_CONFIGURATION_SCHEMA {
        diagnostics.push(ConfigurationDiagnostic::UnsupportedSchema {
            found: configuration.schema,
        });
    }
    if configuration.name.trim().is_empty() {
        diagnostics.push(ConfigurationDiagnostic::InvalidName);
    }
    let descriptor = packages.target_descriptors().into_iter().find(|item| {
        item.architecture == configuration.target.architecture
            && item.machine == configuration.target.machine
            && item.board == configuration.target.board
            && item.os == configuration.target.os
    });
    let Some(descriptor) = descriptor else {
        diagnostics.push(ConfigurationDiagnostic::UnknownTarget {
            target: format!(
                "{}/{} board={:?} os={:?}",
                configuration.target.architecture,
                configuration.target.machine,
                configuration.target.board,
                configuration.target.os
            ),
        });
        return Err(diagnostics);
    };
    validate_limits(&configuration.limits, &descriptor.maxima, &mut diagnostics);
    let target_key = format!(
        "{}/{}/{}",
        descriptor.family, descriptor.architecture, descriptor.machine
    );
    let mut selected = BTreeMap::<String, String>::new();
    let mut declarations = BTreeMap::<String, Vec<String>>::new();
    for base in &configuration.bases {
        if !catalog.base_kinds.contains(&base.kind) {
            diagnostics.push(ConfigurationDiagnostic::UnknownBase {
                kind: base.kind.clone(),
            });
            continue;
        }
        if !catalog
            .base_targets
            .get(&base.kind)
            .is_some_and(|targets| target_matches(targets, &target_key))
        {
            diagnostics.push(ConfigurationDiagnostic::UnsupportedBase {
                kind: base.kind.clone(),
                target: target_key.clone(),
            });
        }
        let preferences = base.preferences();
        if preferences.is_empty() {
            diagnostics.push(ConfigurationDiagnostic::MissingImplementation {
                kind: base.kind.clone(),
            });
            continue;
        }
        if base.implementation.is_some() && !base.implementations.is_empty() {
            diagnostics.push(ConfigurationDiagnostic::ContradictoryImplementations {
                kind: base.kind.clone(),
            });
        }
        let preference_values = preferences
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>();
        if declarations
            .insert(base.kind.clone(), preference_values.clone())
            .is_some_and(|old| old != preference_values)
        {
            diagnostics.push(ConfigurationDiagnostic::DuplicateContradictoryBase {
                kind: base.kind.clone(),
            });
            diagnostics.push(ConfigurationDiagnostic::ConflictingResourceAssignment {
                resource: format!("Base controller {}", base.kind),
            });
        }
        let mut resolved = None;
        let allowed = packages
            .offers_for_target(&target_key)
            .into_iter()
            .filter(|offer| offer.offer.base_kind == base.kind)
            .map(|offer| offer.offer.implementation_id)
            .collect::<Vec<_>>();
        for implementation in preferences {
            if !catalog
                .driver_kinds
                .iter()
                .any(|item| item == implementation)
            {
                diagnostics.push(ConfigurationDiagnostic::UnknownImplementation {
                    implementation: implementation.into(),
                });
                continue;
            }
            if allowed.iter().any(|allowed| allowed == implementation)
                && catalog
                    .driver_targets
                    .get(implementation)
                    .is_some_and(|targets| target_matches(targets, &target_key))
            {
                if resolved.is_none() {
                    resolved = Some(implementation.to_owned());
                }
            } else {
                diagnostics.push(ConfigurationDiagnostic::IncompatibleImplementation {
                    implementation: implementation.into(),
                    target: target_key.clone(),
                });
            }
        }
        let Some(implementation) = resolved else {
            continue;
        };
        if selected
            .insert(base.kind.clone(), implementation.clone())
            .is_some_and(|old| old != implementation)
        {
            diagnostics.push(ConfigurationDiagnostic::DuplicateContradictoryBase {
                kind: base.kind.clone(),
            });
        }
    }
    let mut resource_ids = BTreeSet::new();
    for resource in &configuration.resources {
        if !resource_ids.insert(&resource.id) {
            diagnostics.push(ConfigurationDiagnostic::DuplicateResource {
                id: resource.id.clone(),
            });
        }
        if resource.slots == 0 || resource.bytes == 0 {
            diagnostics.push(ConfigurationDiagnostic::UnboundedCapacity { field: "resource" });
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let canonical =
        stable_host_configuration_identity_source(&configuration).map_err(|item| vec![item])?;
    let configuration_id = format!("sha256:{:x}", Sha256::digest(canonical.as_bytes()));
    let mut resolved_bases = selected.into_iter().collect::<Vec<_>>();
    resolved_bases.sort();
    let profile = HostProfile {
        schema: HOST_PROFILE_SCHEMA.into(),
        name: configuration.name.clone(),
        source_configuration_id: Some(configuration_id.clone()),
        target: TargetSelection {
            family: descriptor.family.clone(),
            architecture: descriptor.architecture.clone(),
            machine: descriptor.machine.clone(),
            build_profile: "release".into(),
            fabrication_descriptor: configuration.target.fabrication_descriptor.clone(),
        },
        host_core: descriptor.host_core.clone(),
        fragments: Vec::new(),
        capabilities: Vec::new(),
        host_operations: descriptor.host_operations.clone(),
        resources: configuration.resources.clone(),
        bases: resolved_bases
            .iter()
            .enumerate()
            .map(|(index, (kind, implementation))| BaseSelection {
                id: format!("base/{index}"),
                kind: kind.clone(),
                driver: implementation.clone(),
            })
            .collect(),
        drivers: resolved_bases
            .iter()
            .enumerate()
            .map(|(index, (_, implementation))| DriverSelection {
                id: format!("driver/{index}"),
                kind: implementation.clone(),
            })
            .collect(),
        lines: Vec::new(),
        presenters: descriptor
            .presenter
            .as_ref()
            .map(|presenter| PresenterSelection {
                id: presenter.id.clone(),
                implementation: presenter.implementation_id.clone(),
                interactive: presenter.interactive,
            })
            .into_iter()
            .collect(),
        facilities: Vec::new(),
        exclusions: Vec::new(),
        policy: HostPolicy {
            authority_profile: "authority/explicit@1".into(),
            trust_profile: "trust/local-explicit@1".into(),
            update_profile: "update/rebuild@1".into(),
            ambient_defaults: false,
        },
        bounds: configuration.limits.clone(),
    };
    validate_profile(profile.clone(), catalog).map_err(|items| {
        items
            .into_iter()
            .map(|item| ConfigurationDiagnostic::Profile {
                detail: format!("{item:?}"),
            })
            .collect::<Vec<_>>()
    })?;
    Ok(CheckedHostConfiguration {
        configuration,
        profile,
        configuration_id,
        resolved_bases,
    })
}

fn target_matches(patterns: &[String], target: &str) -> bool {
    let actual = target.split('/').collect::<Vec<_>>();
    patterns.iter().any(|pattern| {
        pattern
            .split('/')
            .zip(&actual)
            .all(|(expected, found)| expected == "*" || expected == *found)
    })
}

fn validate_limits(
    limits: &HostBounds,
    maxima: &HostBounds,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) {
    macro_rules! limit {
        ($field:ident) => {{
            let requested = u64::from(limits.$field);
            let maximum = u64::from(maxima.$field);
            if requested == 0 {
                diagnostics.push(ConfigurationDiagnostic::UnboundedCapacity {
                    field: stringify!($field),
                });
            } else if requested > maximum {
                diagnostics.push(ConfigurationDiagnostic::LimitExceeded {
                    field: stringify!($field),
                    requested,
                    maximum,
                });
            }
        }};
    }
    for (field, requested, maximum) in [
        (
            "static_memory_bytes",
            limits.static_memory_bytes,
            maxima.static_memory_bytes,
        ),
        (
            "heap_arena_bytes",
            limits.heap_arena_bytes,
            maxima.heap_arena_bytes,
        ),
        (
            "buffered_bytes",
            limits.buffered_bytes,
            maxima.buffered_bytes,
        ),
    ] {
        if requested == 0 {
            diagnostics.push(ConfigurationDiagnostic::UnboundedCapacity { field });
        } else if requested > maximum {
            diagnostics.push(ConfigurationDiagnostic::LimitExceeded {
                field,
                requested,
                maximum,
            });
        }
    }
    limit!(queue_items);
    limit!(active_instances);
    limit!(operation_slots);
    limit!(timer_slots);
    limit!(line_sessions);
    limit!(evidence_items);
}
