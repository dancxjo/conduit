use std::collections::{BTreeMap, BTreeSet};

use crate::{
    canonical::profile_id, FabricationCatalog, HostProfile, PrerequisiteNode, ProfileId,
    HOST_PROFILE_SCHEMA, MAX_PROFILE_ID_BYTES, MAX_PROFILE_ITEMS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileDiagnostic {
    UnsupportedSchema { found: String },
    InvalidIdentity { field: &'static str, value: String },
    TooManyItems { field: &'static str, found: usize },
    UnknownReference { field: &'static str, value: String },
    DuplicateIdentity { field: &'static str, value: String },
    Contradiction { value: String },
    TargetIncompatible { item: String, target: String },
    UnboundedResource { resource: String },
    UnsatisfiedPrerequisite { requester: String, missing: String },
    CircularPrerequisite { path: Vec<String> },
    AmbientDefaultsForbidden,
    Encoding { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedHostProfile {
    profile: HostProfile,
    profile_id: ProfileId,
    dependency_paths: BTreeMap<String, Vec<String>>,
}

impl ValidatedHostProfile {
    pub fn profile(&self) -> &HostProfile {
        &self.profile
    }

    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub fn dependency_paths(&self) -> &BTreeMap<String, Vec<String>> {
        &self.dependency_paths
    }
}

pub fn validate_profile(
    profile: HostProfile,
    catalog: &FabricationCatalog,
) -> Result<ValidatedHostProfile, Vec<ProfileDiagnostic>> {
    let mut diagnostics = Vec::new();
    validate_shape(&profile, catalog, &mut diagnostics);
    let dependency_paths = validate_dependencies(&profile, catalog, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let profile_id = profile_id(&profile).map_err(|diagnostic| vec![diagnostic])?;
    Ok(ValidatedHostProfile {
        profile,
        profile_id,
        dependency_paths,
    })
}

fn validate_shape(
    profile: &HostProfile,
    catalog: &FabricationCatalog,
    diagnostics: &mut Vec<ProfileDiagnostic>,
) {
    if profile.schema != HOST_PROFILE_SCHEMA {
        diagnostics.push(ProfileDiagnostic::UnsupportedSchema {
            found: profile.schema.clone(),
        });
    }
    for (field, value) in [("name", &profile.name), ("host_core", &profile.host_core)] {
        if value.is_empty() || value.len() > MAX_PROFILE_ID_BYTES {
            diagnostics.push(ProfileDiagnostic::InvalidIdentity {
                field,
                value: value.clone(),
            });
        }
    }
    let target = profile.target.key();
    known(&catalog.targets, "target", &target, diagnostics);
    known(
        &catalog.host_cores,
        "host_core",
        &profile.host_core,
        diagnostics,
    );
    for fragment in &profile.fragments {
        known(
            &catalog.profile_fragments,
            "fragment",
            fragment,
            diagnostics,
        );
    }
    for (field, count) in [
        ("capabilities", profile.capabilities.len()),
        ("host_operations", profile.host_operations.len()),
        ("resources", profile.resources.len()),
        ("bases", profile.bases.len()),
        ("drivers", profile.drivers.len()),
        ("lines", profile.lines.len()),
        ("presenters", profile.presenters.len()),
        ("facilities", profile.facilities.len()),
    ] {
        if count > MAX_PROFILE_ITEMS {
            diagnostics.push(ProfileDiagnostic::TooManyItems {
                field,
                found: count,
            });
        }
    }
    validate_unique(
        "resource",
        profile.resources.iter().map(|item| item.id.as_str()),
        diagnostics,
    );
    validate_unique(
        "base",
        profile.bases.iter().map(|item| item.id.as_str()),
        diagnostics,
    );
    validate_unique(
        "driver",
        profile.drivers.iter().map(|item| item.id.as_str()),
        diagnostics,
    );
    validate_unique(
        "presenter",
        profile.presenters.iter().map(|item| item.id.as_str()),
        diagnostics,
    );
    for resource in &profile.resources {
        if resource.slots == 0 || resource.bytes == 0 {
            diagnostics.push(ProfileDiagnostic::UnboundedResource {
                resource: resource.id.clone(),
            });
        }
    }
    for base in &profile.bases {
        known(&catalog.base_kinds, "base.kind", &base.kind, diagnostics);
        known(
            &catalog.driver_kinds,
            "base.driver",
            &base.driver,
            diagnostics,
        );
        if let Some(targets) = catalog.base_targets.get(&base.kind) {
            if !target_matches(targets, &target) {
                diagnostics.push(ProfileDiagnostic::TargetIncompatible {
                    item: base.kind.clone(),
                    target: target.clone(),
                });
            }
        }
        if !profile
            .drivers
            .iter()
            .any(|driver| driver.kind == base.driver)
        {
            diagnostics.push(ProfileDiagnostic::UnsatisfiedPrerequisite {
                requester: format!("base:{}", base.id),
                missing: format!("driver:{}", base.driver),
            });
        }
    }
    for driver in &profile.drivers {
        known(
            &catalog.driver_kinds,
            "driver.kind",
            &driver.kind,
            diagnostics,
        );
        if let Some(targets) = catalog.driver_targets.get(&driver.kind) {
            if !target_matches(targets, &target) {
                diagnostics.push(ProfileDiagnostic::TargetIncompatible {
                    item: driver.kind.clone(),
                    target: target.clone(),
                });
            }
        }
    }
    for line in &profile.lines {
        known(&catalog.line_facilities, "line", line, diagnostics);
    }
    for facility in &profile.facilities {
        known(&catalog.facilities, "facility", facility, diagnostics);
    }
    for policy in [
        &profile.policy.authority_profile,
        &profile.policy.trust_profile,
        &profile.policy.update_profile,
    ] {
        known(&catalog.policy_profiles, "policy", policy, diagnostics);
    }
    if profile.policy.ambient_defaults {
        diagnostics.push(ProfileDiagnostic::AmbientDefaultsForbidden);
    }
    let included = profile
        .capabilities
        .iter()
        .map(|capability| capability.implementation.as_str())
        .chain(
            profile
                .presenters
                .iter()
                .map(|item| item.implementation.as_str()),
        )
        .collect::<BTreeSet<_>>();
    for exclusion in &profile.exclusions {
        if !catalog.implementations.contains_key(exclusion)
            && !catalog.presenters.contains_key(exclusion)
            && !catalog.facilities.contains(exclusion)
        {
            diagnostics.push(ProfileDiagnostic::UnknownReference {
                field: "exclusion",
                value: exclusion.clone(),
            });
        }
        if included.contains(exclusion.as_str()) {
            diagnostics.push(ProfileDiagnostic::Contradiction {
                value: exclusion.clone(),
            });
        }
    }
}

fn validate_dependencies(
    profile: &HostProfile,
    catalog: &FabricationCatalog,
    diagnostics: &mut Vec<ProfileDiagnostic>,
) -> BTreeMap<String, Vec<String>> {
    let mut paths = BTreeMap::new();
    let target = profile.target.key();
    for capability in &profile.capabilities {
        let requester = format!(
            "capability:{}@{}",
            capability.kind, capability.contract_revision
        );
        let Some(metadata) = catalog.implementations.get(&capability.implementation) else {
            diagnostics.push(ProfileDiagnostic::UnknownReference {
                field: "capability.implementation",
                value: capability.implementation.clone(),
            });
            continue;
        };
        if metadata.kind != capability.kind
            || metadata.contract_revision != capability.contract_revision
        {
            diagnostics.push(ProfileDiagnostic::UnsatisfiedPrerequisite {
                requester,
                missing: format!(
                    "exact-kind-contract:{}@{}",
                    capability.kind, capability.contract_revision
                ),
            });
            continue;
        }
        if !target_matches(&metadata.targets, &target) {
            diagnostics.push(ProfileDiagnostic::TargetIncompatible {
                item: capability.implementation.clone(),
                target: target.clone(),
            });
        }
        check_nodes(
            profile,
            catalog,
            &requester,
            &metadata.prerequisites,
            diagnostics,
            &mut paths,
        );
    }
    for presenter in &profile.presenters {
        let requester = format!("presenter:{}", presenter.id);
        let Some(metadata) = catalog.presenters.get(&presenter.implementation) else {
            diagnostics.push(ProfileDiagnostic::UnknownReference {
                field: "presenter.implementation",
                value: presenter.implementation.clone(),
            });
            continue;
        };
        if !target_matches(&metadata.targets, &target) {
            diagnostics.push(ProfileDiagnostic::TargetIncompatible {
                item: presenter.implementation.clone(),
                target: target.clone(),
            });
        }
        check_nodes(
            profile,
            catalog,
            &requester,
            &metadata.prerequisites,
            diagnostics,
            &mut paths,
        );
    }
    paths
}

fn check_nodes(
    profile: &HostProfile,
    catalog: &FabricationCatalog,
    requester: &str,
    roots: &[PrerequisiteNode],
    diagnostics: &mut Vec<ProfileDiagnostic>,
    paths: &mut BTreeMap<String, Vec<String>>,
) {
    for root in roots {
        let mut visiting = Vec::new();
        walk_node(
            profile,
            catalog,
            requester,
            root,
            &mut visiting,
            diagnostics,
            paths,
        );
    }
}

fn walk_node(
    profile: &HostProfile,
    catalog: &FabricationCatalog,
    requester: &str,
    node: &PrerequisiteNode,
    visiting: &mut Vec<PrerequisiteNode>,
    diagnostics: &mut Vec<ProfileDiagnostic>,
    paths: &mut BTreeMap<String, Vec<String>>,
) {
    if let Some(position) = visiting.iter().position(|candidate| candidate == node) {
        let mut path = visiting[position..]
            .iter()
            .map(node_label)
            .collect::<Vec<_>>();
        path.push(node_label(node));
        diagnostics.push(ProfileDiagnostic::CircularPrerequisite { path });
        return;
    }
    visiting.push(node.clone());
    paths.insert(
        format!("{requester} -> {}", node_label(node)),
        visiting.iter().map(node_label).collect(),
    );
    if !profile_satisfies(profile, node) {
        diagnostics.push(ProfileDiagnostic::UnsatisfiedPrerequisite {
            requester: requester.to_owned(),
            missing: node_label(node),
        });
    }
    if let Some(dependencies) = catalog.dependencies.get(node) {
        for dependency in dependencies {
            walk_node(
                profile,
                catalog,
                requester,
                dependency,
                visiting,
                diagnostics,
                paths,
            );
        }
    }
    visiting.pop();
}

fn profile_satisfies(profile: &HostProfile, node: &PrerequisiteNode) -> bool {
    match node {
        PrerequisiteNode::Implementation(value) => profile
            .capabilities
            .iter()
            .any(|capability| capability.implementation == *value),
        PrerequisiteNode::HostOperation(value) => profile.host_operations.contains(value),
        PrerequisiteNode::Resource(value) => profile
            .resources
            .iter()
            .any(|resource| resource.class == *value),
        PrerequisiteNode::Base(value) => profile.bases.iter().any(|base| base.kind == *value),
        PrerequisiteNode::Driver(value) => {
            profile.drivers.iter().any(|driver| driver.kind == *value)
        }
        PrerequisiteNode::Facility(value) => profile.facilities.contains(value),
    }
}

fn node_label(node: &PrerequisiteNode) -> String {
    match node {
        PrerequisiteNode::Implementation(value) => format!("implementation:{value}"),
        PrerequisiteNode::HostOperation(value) => format!("host-operation:{value}"),
        PrerequisiteNode::Resource(value) => format!("resource:{value}"),
        PrerequisiteNode::Base(value) => format!("base:{value}"),
        PrerequisiteNode::Driver(value) => format!("driver:{value}"),
        PrerequisiteNode::Facility(value) => format!("facility:{value}"),
    }
}

fn target_matches(patterns: &[String], target: &str) -> bool {
    patterns.iter().any(|pattern| {
        let pattern = pattern.split('/').collect::<Vec<_>>();
        let target = target.split('/').collect::<Vec<_>>();
        pattern.len() == target.len()
            && pattern
                .iter()
                .zip(target)
                .all(|(expected, actual)| *expected == "*" || *expected == actual)
    })
}

fn known(
    values: &[String],
    field: &'static str,
    value: &str,
    diagnostics: &mut Vec<ProfileDiagnostic>,
) {
    if !values.iter().any(|known| known == value) {
        diagnostics.push(ProfileDiagnostic::UnknownReference {
            field,
            value: value.to_owned(),
        });
    }
}

fn validate_unique<'a>(
    field: &'static str,
    values: impl Iterator<Item = &'a str>,
    diagnostics: &mut Vec<ProfileDiagnostic>,
) {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            diagnostics.push(ProfileDiagnostic::DuplicateIdentity {
                field,
                value: value.to_owned(),
            });
        }
    }
}
