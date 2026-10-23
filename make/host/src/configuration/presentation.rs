//! Presenter selection from selected Base/resource roots, with a target default.
use super::{target_matches, ConfigurationDiagnostic};
use crate::{MakeCatalog, PresenterSelection, ResourceBudget, TargetPresenter};
use std::collections::BTreeSet;

pub(super) struct ResolvedPresentation {
    pub presenter: Option<PresenterSelection>,
    pub host_calls: Vec<String>,
    pub facilities: Vec<String>,
}

pub(super) fn resolve(
    default: Option<&TargetPresenter>,
    existing_host_calls: &[String],
    target: &str,
    catalog: &MakeCatalog,
    bases: &[(String, String)],
    resources: &[ResourceBudget],
) -> Result<ResolvedPresentation, Vec<ConfigurationDiagnostic>> {
    // A legacy package default without metadata retains its existing contract.
    // A metadata-backed default participates in the same checked selection as
    // other realizations; it cannot hide a selected alternative or ambiguity.
    let can_derive =
        default.is_none_or(|value| catalog.presenters.contains_key(&value.implementation_id));
    let mut candidates = if can_derive {
        catalog
            .presenters
            .iter()
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
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    candidates.sort_by(|left, right| left.0.cmp(right.0));
    if candidates.len() > 1 {
        return Err(vec![ConfigurationDiagnostic::AmbiguousFaceRealization {
            implementations: candidates.into_iter().map(|(id, _)| id.clone()).collect(),
        }]);
    }
    let selected = candidates.pop();
    let mut host_calls = existing_host_calls.to_vec();
    let mut facilities = Vec::new();
    if let Some((_, metadata)) = selected {
        collect_derived_face_requirements(
            &metadata.prerequisites,
            catalog,
            &mut host_calls,
            &mut facilities,
            &mut BTreeSet::new(),
        );
    }
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
                    id: "presenter/main".into(),
                    implementation: id.clone(),
                    interactive: true,
                }
            }
        })
        .or_else(default_selection);
    Ok(ResolvedPresentation {
        presenter,
        host_calls,
        facilities,
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
            return true;
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
            crate::PrerequisiteNode::Implementation(_) => false,
        };
        selected
            && catalog.dependencies.get(node).is_none_or(|dependencies| {
                presenter_roots_are_selected(dependencies, catalog, bases, resources, visited)
            })
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

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
