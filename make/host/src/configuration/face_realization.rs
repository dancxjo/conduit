//! Resolve a fixed anchor and one uniquely selected optional Face realization.
use super::{target_matches, ConfigurationDiagnostic};
use crate::{MakeCatalog, PresenterSelection, ResourceBudget, TargetDescriptor};
use std::collections::BTreeSet;

pub(super) struct Resolution {
    pub presenters: Vec<PresenterSelection>,
    pub host_calls: Vec<String>,
    pub facilities: Vec<String>,
}

pub(super) fn resolve(
    descriptor: &TargetDescriptor,
    catalog: &MakeCatalog,
    bases: &[(String, String)],
    resources: &[ResourceBudget],
) -> Result<Resolution, Vec<ConfigurationDiagnostic>> {
    let target = descriptor.key();
    let anchor = descriptor.presenter.as_ref();
    let mut derived = catalog
        .presenters
        .iter()
        .filter(|(implementation, metadata)| {
            anchor.is_none_or(|anchor| anchor.implementation_id != **implementation)
                && target_matches(&metadata.targets, &target)
                && presenter_roots_are_selected(
                    &metadata.prerequisites,
                    catalog,
                    bases,
                    resources,
                    &mut BTreeSet::new(),
                )
        })
        .collect::<Vec<_>>();
    derived.sort_by(|left, right| left.0.cmp(right.0));
    if derived.len() > 1 {
        return Err(vec![ConfigurationDiagnostic::AmbiguousFaceRealization {
            implementations: derived.into_iter().map(|(id, _)| id.clone()).collect(),
        }]);
    }
    let mut presenters = anchor
        .map(|anchor| PresenterSelection {
            id: anchor.id.clone(),
            implementation: anchor.implementation_id.clone(),
            interactive: anchor.interactive,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let mut host_calls = descriptor.host_calls.clone();
    let mut facilities = Vec::new();
    if let Some((implementation, metadata)) = derived.pop() {
        presenters.push(PresenterSelection {
            id: anchor.map_or_else(
                || "presenter/main".into(),
                |anchor| format!("{}/derived", anchor.id),
            ),
            implementation: implementation.clone(),
            interactive: true,
        });
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
    Ok(Resolution {
        presenters,
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
mod tests;
