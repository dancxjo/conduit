//! Mechanical prerequisite classification for one exact Host profile.

use std::collections::BTreeSet;

use conduit_core::HostAdvertisement;

use super::{inventory, GapClassification};

pub(crate) struct Classification {
    pub(crate) classification: GapClassification,
    pub(crate) reason_code: Option<&'static str>,
    pub(crate) required_host_operations: Vec<String>,
    pub(crate) required_resources: Vec<String>,
    pub(crate) required_bases: Vec<String>,
    pub(crate) unsatisfied: Vec<String>,
    pub(crate) machine_specific: bool,
}

pub(crate) fn classify(
    host: &HostAdvertisement,
    kind: &inventory::InventoryEntry,
    implemented: bool,
) -> Classification {
    let canonical = inventory::catalog_offers()
        .into_iter()
        .find(|offer| {
            offer.kind_id.as_str() == kind.kind_id
                && offer.kind_contract_revision.as_str() == kind.contract_revision
        })
        .expect("inventory and canonical offers share exact identities");
    let required_host_operations = canonical
        .host_operations
        .iter()
        .map(|requirement| requirement.contract_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let required_resources = canonical
        .resource_requirements
        .iter()
        .map(|requirement| requirement.class_id.as_str().to_owned())
        .collect::<Vec<_>>();
    let required_bases = required_resources
        .iter()
        .filter_map(|resource| resource_base(resource))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if implemented {
        return Classification {
            classification: GapClassification::Implemented,
            reason_code: None,
            required_host_operations,
            required_resources,
            required_bases,
            unsatisfied: Vec::new(),
            machine_specific: false,
        };
    }

    let available_operations = host
        .capabilities
        .iter()
        .flat_map(|capability| capability.host_operations.iter())
        .map(|requirement| requirement.contract_id.as_str())
        .collect::<BTreeSet<_>>();
    let available_resources = host
        .resources
        .iter()
        .map(|resource| resource.class_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut available_bases = available_bases(host.profile.as_str());
    for resource in &available_resources {
        if let Some(base) = resource_base(resource) {
            available_bases.insert(base);
        }
    }

    let missing_bases = required_bases
        .iter()
        .filter(|base| !available_bases.contains(base.as_str()))
        .map(|base| format!("base:{base}"))
        .collect::<Vec<_>>();
    if !missing_bases.is_empty() {
        return missing(
            GapClassification::MissingBase,
            "required-base-unavailable",
            required_host_operations,
            required_resources,
            required_bases,
            missing_bases,
        );
    }
    let missing_resources = required_resources
        .iter()
        .filter(|resource| !available_resources.contains(resource.as_str()))
        .map(|resource| format!("resource:{resource}"))
        .collect::<Vec<_>>();
    if !missing_resources.is_empty() {
        return missing(
            GapClassification::MissingResource,
            "required-resource-unavailable",
            required_host_operations,
            required_resources,
            required_bases,
            missing_resources,
        );
    }
    let missing_operations = required_host_operations
        .iter()
        .filter(|operation| !available_operations.contains(operation.as_str()))
        .map(|operation| format!("host-operation:{operation}"))
        .collect::<Vec<_>>();
    if !missing_operations.is_empty() {
        return missing(
            GapClassification::MissingHostOperation,
            "required-host-operation-unavailable",
            required_host_operations,
            required_resources,
            required_bases,
            missing_operations,
        );
    }
    Classification {
        classification: GapClassification::PortableImplementationMissing,
        reason_code: Some("portable-implementation-not-installed"),
        required_host_operations,
        required_resources,
        required_bases,
        unsatisfied: vec!["implementation:portable".to_owned()],
        machine_specific: false,
    }
}

fn missing(
    classification: GapClassification,
    reason_code: &'static str,
    required_host_operations: Vec<String>,
    required_resources: Vec<String>,
    required_bases: Vec<String>,
    unsatisfied: Vec<String>,
) -> Classification {
    Classification {
        classification,
        reason_code: Some(reason_code),
        required_host_operations,
        required_resources,
        required_bases,
        unsatisfied,
        machine_specific: true,
    }
}

fn resource_base(resource: &str) -> Option<&'static str> {
    match resource {
        conduit_core::TIMER_RESOURCE_CLASS
        | conduit_core::MONOTONIC_MILLISECOND_TIMER_RESOURCE_CLASS => Some("timer"),
        conduit_core::PRESENTATION_RESOURCE_CLASS => Some("serial"),
        conduit_semantic_catalog::PROTECTED_FILE_RESOURCE_CLASS => Some("storage"),
        _ => None,
    }
}

fn available_bases(profile: &str) -> BTreeSet<&'static str> {
    if profile == "conduitos/two-lane-cooperative@1" {
        [
            "clock",
            "timer",
            "serial",
            "interrupt",
            "idle",
            "execution-lane",
            "memory",
        ]
        .into_iter()
        .collect()
    } else {
        BTreeSet::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::catalog::profiles;

    #[test]
    fn distinctions_are_derived_from_exact_offer_prerequisites() {
        let host = profiles::conduitos_advertisement().unwrap();
        let inventory = inventory::derive().unwrap();
        let classify_kind = |kind_id: &str| {
            let kind = inventory
                .entries
                .iter()
                .find(|entry| entry.kind_id == kind_id)
                .unwrap();
            classify(&host, kind, false).classification
        };
        assert_eq!(
            classify_kind("logic/select"),
            GapClassification::PortableImplementationMissing
        );
        let mut without_text_join = host.clone();
        without_text_join
            .capabilities
            .retain(|offer| offer.kind_id.as_str() != "text/join");
        let text_join = inventory
            .entries
            .iter()
            .find(|entry| entry.kind_id == "text/join")
            .unwrap();
        assert_eq!(
            classify(&without_text_join, text_join, false).classification,
            GapClassification::MissingHostOperation
        );
        assert_eq!(
            classify_kind("time/debounce"),
            GapClassification::MissingResource
        );
        assert_eq!(classify_kind("file/copy"), GapClassification::MissingBase);
    }
}
