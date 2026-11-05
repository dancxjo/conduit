//! Presenter selection from selected Base/resource roots, with a target default.
use super::{target_matches, ConfigurationDiagnostic};
use crate::{MakeCatalog, PresenterSelection, ResourceBudget, TargetPresenter};
use std::collections::BTreeSet;

pub(super) struct ResolvedPresentation {
    pub presenter: Option<PresenterSelection>,
    pub supplemental: Vec<PresenterSelection>,
    pub host_calls: Vec<String>,
    pub facilities: Vec<String>,
    pub capabilities: Vec<crate::CapabilitySelection>,
}

pub(super) fn resolve(
    default: Option<&TargetPresenter>,
    existing_host_calls: &[String],
    target: &str,
    catalog: &MakeCatalog,
    bases: &[(String, String)],
    resources: &[ResourceBudget],
) -> Result<ResolvedPresentation, Vec<ConfigurationDiagnostic>> {
    let anchor_selected = default.is_some_and(|value| {
        catalog
            .presenters
            .get(&value.implementation_id)
            .is_none_or(|metadata| {
                target_matches(&metadata.targets, target)
                    && presenter_roots_are_selected(
                        &metadata.prerequisites,
                        catalog,
                        bases,
                        resources,
                        &mut BTreeSet::new(),
                    )
            })
    });
    let mut candidates = catalog
        .presenters
        .iter()
        .filter(|(id, _)| {
            !anchor_selected || default.is_none_or(|value| value.implementation_id != **id)
        })
        .filter(|(_, metadata)| target_matches(&metadata.targets, target))
        .filter(|(_, metadata)| {
            presenter_roots_are_selected(
                &metadata.prerequisites,
                catalog,
                bases,
                resources,
                &mut BTreeSet::new(),
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(right.0));
    if candidates.len() > 1 {
        return Err(vec![ConfigurationDiagnostic::AmbiguousFaceRealization {
            implementations: candidates.into_iter().map(|(id, _)| id.clone()).collect(),
        }]);
    }
    let selected = candidates.pop();
    let mut host_calls = existing_host_calls.to_vec();
    let mut facilities = Vec::new();
    let mut capabilities = Vec::new();
    if anchor_selected {
        if let Some(metadata) =
            default.and_then(|anchor| catalog.presenters.get(&anchor.implementation_id))
        {
            collect_derived_face_requirements(
                &metadata.prerequisites,
                catalog,
                &mut host_calls,
                &mut facilities,
                &mut BTreeSet::new(),
            );
        }
    }
    if let Some((_, metadata)) = selected {
        collect_derived_face_requirements(
            &metadata.prerequisites,
            catalog,
            &mut host_calls,
            &mut facilities,
            &mut BTreeSet::new(),
        );
    }
    for presenter in default
        .filter(|_| anchor_selected)
        .map(|value| value.implementation_id.as_str())
        .into_iter()
        .chain(selected.map(|(id, _)| id.as_str()))
    {
        if let Some(metadata) = catalog.presenters.get(presenter) {
            collect_implementations(
                &metadata.prerequisites,
                catalog,
                &mut capabilities,
                &mut BTreeSet::new(),
            );
        }
    }
    capabilities.sort_by(|a, b| a.implementation.cmp(&b.implementation));
    capabilities.dedup_by(|a, b| a.implementation == b.implementation);
    host_calls.sort();
    host_calls.dedup();
    facilities.sort();
    facilities.dedup();
    let default_selection = || {
        default.map(|value| PresenterSelection {
            id: value.id.clone(),
            implementation: value.implementation_id.clone(),
            interactive: value.interactive,
        })
    };
    let presenter = selected
        .map(|(id, _)| {
            if default.is_some_and(|value| value.implementation_id == *id) {
                default_selection().expect("matching default exists")
            } else {
                PresenterSelection {
                    id: default.filter(|_| anchor_selected).map_or_else(
                        || "presenter/main".into(),
                        |anchor| format!("{}/derived", anchor.id),
                    ),
                    implementation: id.clone(),
                    interactive: true,
                }
            }
        })
        .or_else(default_selection);
    // A selected fixed package presenter is an independent product anchor.
    // Selected resource-backed presentation supplements that existing entrance.
    let mut supplemental = Vec::new();
    if let (Some(anchor), Some(selected)) = (default, presenter.as_ref()) {
        if anchor_selected && selected.implementation != anchor.implementation_id {
            supplemental.push(default_selection().expect("anchor exists"));
        }
    }
    Ok(ResolvedPresentation {
        supplemental,
        presenter,
        host_calls,
        facilities,
        capabilities,
    })
}

fn presenter_roots_are_selected(
    roots: &[crate::PrerequisiteNode],
    catalog: &MakeCatalog,
    bases: &[(String, String)],
    resources: &[ResourceBudget],
    visited: &mut BTreeSet<crate::PrerequisiteNode>,
) -> bool {
    roots.iter().all(|node| {
        if !visited.insert(node.clone()) {
            return false;
        }
        let selected = match node {
            crate::PrerequisiteNode::Base(value) => bases.iter().any(|(kind, _)| kind == value),
            crate::PrerequisiteNode::Driver(value) => bases
                .iter()
                .any(|(_, implementation)| implementation == value),
            crate::PrerequisiteNode::Resource(value) => {
                resources.iter().any(|resource| resource.class == *value)
            }
            crate::PrerequisiteNode::HostCall(_) | crate::PrerequisiteNode::Facility(_) => true,
            crate::PrerequisiteNode::Implementation(value) => {
                catalog.implementations.get(value).is_some_and(|metadata| {
                    presenter_roots_are_selected(
                        &metadata.prerequisites,
                        catalog,
                        bases,
                        resources,
                        visited,
                    )
                })
            }
        };
        let satisfied = selected
            && catalog.dependencies.get(node).is_none_or(|dependencies| {
                presenter_roots_are_selected(dependencies, catalog, bases, resources, visited)
            });
        visited.remove(node);
        satisfied
    })
}

fn collect_derived_face_requirements(
    roots: &[crate::PrerequisiteNode],
    catalog: &MakeCatalog,
    host_calls: &mut Vec<String>,
    facilities: &mut Vec<String>,
    visited: &mut BTreeSet<crate::PrerequisiteNode>,
) {
    for node in roots {
        if !visited.insert(node.clone()) {
            continue;
        }
        match node {
            crate::PrerequisiteNode::HostCall(value) => host_calls.push(value.clone()),
            crate::PrerequisiteNode::Facility(value) => facilities.push(value.clone()),
            _ => {}
        }
        if let crate::PrerequisiteNode::Implementation(value) = node {
            if let Some(metadata) = catalog.implementations.get(value) {
                collect_derived_face_requirements(
                    &metadata.prerequisites,
                    catalog,
                    host_calls,
                    facilities,
                    visited,
                );
            }
        }
        if let Some(dependencies) = catalog.dependencies.get(node) {
            collect_derived_face_requirements(
                dependencies,
                catalog,
                host_calls,
                facilities,
                visited,
            );
        }
    }
}

fn collect_implementations(
    roots: &[crate::PrerequisiteNode],
    catalog: &MakeCatalog,
    output: &mut Vec<crate::CapabilitySelection>,
    visited: &mut BTreeSet<crate::PrerequisiteNode>,
) {
    for node in roots {
        if !visited.insert(node.clone()) {
            continue;
        }
        if let crate::PrerequisiteNode::Implementation(value) = node {
            if let Some(metadata) = catalog.implementations.get(value) {
                output.push(crate::CapabilitySelection {
                    kind: metadata.kind.clone(),
                    contract_revision: metadata.contract_revision.clone(),
                    implementation: value.clone(),
                });
                collect_implementations(&metadata.prerequisites, catalog, output, visited);
            }
        }
        if let Some(dependencies) = catalog.dependencies.get(node) {
            collect_implementations(dependencies, catalog, output, visited);
        }
    }
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
