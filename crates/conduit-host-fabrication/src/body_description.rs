use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    architecture_package_for, check_host_configuration, ArchitecturePackageDiagnostic,
    CheckedHostConfiguration, FabricationCatalog, HostConfiguration, SporeOutputKind,
};

pub const BODY_DESCRIPTION_SCHEMA: u32 = 1;
pub const MAXIMUM_BODY_HOSTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyDescription {
    pub schema: u32,
    pub name: String,
    pub body: BodyBindingTarget,
    pub hosts: Vec<BodyHostDescription>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyBindingTarget {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyHostDescription {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    pub configuration: String,
    pub spore: SporeDescription,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<DeploymentDescription>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SporeDescription {
    pub join_mode: SporeJoinMode,
    pub output: SporeOutputKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SporeJoinMode {
    Prejoined,
    SelfJoining,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentDescription {
    pub destination: String,
}

#[derive(Debug, Clone)]
pub struct CheckedBodyHost {
    pub description: BodyHostDescription,
    pub configuration: CheckedHostConfiguration,
}

#[derive(Debug, Clone)]
pub struct CheckedBodyDescription {
    description: BodyDescription,
    description_id: String,
    hosts: Vec<CheckedBodyHost>,
}

impl CheckedBodyDescription {
    pub fn description(&self) -> &BodyDescription {
        &self.description
    }
    pub fn description_id(&self) -> &str {
        &self.description_id
    }
    pub fn hosts(&self) -> &[CheckedBodyHost] {
        &self.hosts
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyDescriptionDiagnostic {
    Decode {
        detail: String,
    },
    UnsupportedSchema {
        found: u32,
    },
    InvalidName,
    MissingBody,
    InvalidBodyId,
    EmptyHosts,
    TooManyHosts {
        actual: usize,
        maximum: usize,
    },
    DuplicateHost {
        name: String,
    },
    DuplicatePart {
        part: String,
    },
    MissingConfiguration {
        path: String,
    },
    InvalidHostConfiguration {
        host: String,
        diagnostics: Vec<String>,
    },
    MissingPrejoinedPart {
        host: String,
    },
    UnexpectedPrejoinedInvitation {
        host: String,
    },
    MissingInvitation {
        host: String,
    },
    UnexpectedSelfJoiningPart {
        host: String,
    },
    IncompatibleOutput {
        host: String,
        diagnostic: String,
    },
    AmbiguousDeployment {
        host: String,
    },
    UnsupportedDeployment {
        host: String,
    },
    Encode {
        detail: String,
    },
}

pub fn parse_body_description(source: &str) -> Result<BodyDescription, BodyDescriptionDiagnostic> {
    toml::from_str(source).map_err(|error| BodyDescriptionDiagnostic::Decode {
        detail: error.to_string(),
    })
}

pub fn check_body_description(
    mut description: BodyDescription,
    configurations: &BTreeMap<String, HostConfiguration>,
    catalog: &FabricationCatalog,
) -> Result<CheckedBodyDescription, Vec<BodyDescriptionDiagnostic>> {
    let mut diagnostics = Vec::new();
    if description.schema != BODY_DESCRIPTION_SCHEMA {
        diagnostics.push(BodyDescriptionDiagnostic::UnsupportedSchema {
            found: description.schema,
        });
    }
    if description.name.trim().is_empty() {
        diagnostics.push(BodyDescriptionDiagnostic::InvalidName);
    }
    if description.body.id.trim().is_empty() {
        diagnostics.push(BodyDescriptionDiagnostic::MissingBody);
    } else if !description.body.id.starts_with("body:") {
        diagnostics.push(BodyDescriptionDiagnostic::InvalidBodyId);
    }
    if description.hosts.is_empty() {
        diagnostics.push(BodyDescriptionDiagnostic::EmptyHosts);
    }
    if description.hosts.len() > MAXIMUM_BODY_HOSTS {
        diagnostics.push(BodyDescriptionDiagnostic::TooManyHosts {
            actual: description.hosts.len(),
            maximum: MAXIMUM_BODY_HOSTS,
        });
    }
    let mut names = BTreeSet::new();
    let mut parts = BTreeSet::new();
    let mut checked_hosts = Vec::new();
    for host in &description.hosts {
        if host.name.trim().is_empty() || !names.insert(host.name.clone()) {
            diagnostics.push(BodyDescriptionDiagnostic::DuplicateHost {
                name: host.name.clone(),
            });
        }
        match host.spore.join_mode {
            SporeJoinMode::Prejoined => {
                match host.part.as_ref().filter(|item| !item.trim().is_empty()) {
                    Some(part) if !parts.insert(part.clone()) => diagnostics
                        .push(BodyDescriptionDiagnostic::DuplicatePart { part: part.clone() }),
                    Some(_) => {}
                    None => diagnostics.push(BodyDescriptionDiagnostic::MissingPrejoinedPart {
                        host: host.name.clone(),
                    }),
                }
                if host.spore.invitation.is_some() {
                    diagnostics.push(BodyDescriptionDiagnostic::UnexpectedPrejoinedInvitation {
                        host: host.name.clone(),
                    });
                }
            }
            SporeJoinMode::SelfJoining => {
                if host
                    .spore
                    .invitation
                    .as_ref()
                    .is_none_or(|item| item.trim().is_empty())
                {
                    diagnostics.push(BodyDescriptionDiagnostic::MissingInvitation {
                        host: host.name.clone(),
                    });
                }
                if host.part.is_some() {
                    diagnostics.push(BodyDescriptionDiagnostic::UnexpectedSelfJoiningPart {
                        host: host.name.clone(),
                    });
                }
            }
        }
        if host
            .deployment
            .as_ref()
            .is_some_and(|item| item.destination.trim().is_empty())
        {
            diagnostics.push(BodyDescriptionDiagnostic::AmbiguousDeployment {
                host: host.name.clone(),
            });
        }
        let Some(configuration) = configurations.get(&host.configuration) else {
            diagnostics.push(BodyDescriptionDiagnostic::MissingConfiguration {
                path: host.configuration.clone(),
            });
            continue;
        };
        match check_host_configuration(configuration.clone(), catalog) {
            Ok(checked) => {
                match architecture_package_for(checked.profile())
                    .and_then(|package| package.derive(checked.profile(), &host.spore.output))
                {
                    Ok(selection) => {
                        if host.deployment.is_some() && selection.deployment_adapter.is_none() {
                            diagnostics.push(BodyDescriptionDiagnostic::UnsupportedDeployment {
                                host: host.name.clone(),
                            });
                        } else {
                            checked_hosts.push(CheckedBodyHost {
                                description: host.clone(),
                                configuration: checked,
                            });
                        }
                    }
                    Err(item) => diagnostics.push(BodyDescriptionDiagnostic::IncompatibleOutput {
                        host: host.name.clone(),
                        diagnostic: architecture_diagnostic(item),
                    }),
                }
            }
            Err(items) => diagnostics.push(BodyDescriptionDiagnostic::InvalidHostConfiguration {
                host: host.name.clone(),
                diagnostics: items.into_iter().map(|item| format!("{item:?}")).collect(),
            }),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    description
        .hosts
        .sort_by(|left, right| left.name.cmp(&right.name));
    checked_hosts.sort_by(|left, right| left.description.name.cmp(&right.description.name));
    let canonical = toml::to_string(&description).map_err(|error| {
        vec![BodyDescriptionDiagnostic::Encode {
            detail: error.to_string(),
        }]
    })?;
    let description_id = format!(
        "body-description:sha256:{:x}",
        Sha256::digest(canonical.as_bytes())
    );
    Ok(CheckedBodyDescription {
        description,
        description_id,
        hosts: checked_hosts,
    })
}

fn architecture_diagnostic(item: ArchitecturePackageDiagnostic) -> String {
    format!("{item:?}")
}
