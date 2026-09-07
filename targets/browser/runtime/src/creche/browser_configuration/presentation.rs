//! Maps authoritative Crèche configuration truth into product-owned semantics.

use std::collections::BTreeSet;

use conduit_creche_model::{
    BrowserConfigurationActions, BrowserConfigurationChoice, BrowserConfigurationGroup,
    BrowserConfigurationReview as BrowserConfigurationReviewPresentation,
};
use conduit_host_browser_fabrication::BROWSER_IMPLEMENTATIONS;
use serde::Deserialize;

use super::{review, BrowserConfigurationSelection, CATALOG_GENERATION};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserConfigurationViewRequest {
    revision: u32,
    mode: String,
    catalog_generation: u32,
    implementations: Vec<String>,
    group: Option<String>,
    diagnostic: Option<String>,
}

fn presentation_view(request: BrowserConfigurationViewRequest) -> Result<Vec<u8>, String> {
    let semantic = match request.mode.as_str() {
        "actions" => BrowserConfigurationActions {
            revision: request.revision,
            diagnostic: request.diagnostic,
        }
        .presentation(),
        "group" => {
            if request.catalog_generation != CATALOG_GENERATION {
                return Err("StaleCatalogGeneration: configuration presentation is stale".into());
            }
            let selected = reviewed_selection(&request.implementations)?;
            let group = request.group.ok_or_else(|| {
                "MissingConfigurationGroup: group view omitted its group".to_string()
            })?;
            let choices = BROWSER_IMPLEMENTATIONS
                .iter()
                .enumerate()
                .filter(|(_, descriptor)| descriptor.group == group)
                .map(|(catalog_index, descriptor)| BrowserConfigurationChoice {
                    catalog_index,
                    label: descriptor.label.into(),
                    implementation_id: descriptor.implementation_id.into(),
                    selected: selected.contains(descriptor.implementation_id),
                    prerequisites: descriptor
                        .prerequisites
                        .iter()
                        .map(|prerequisite| prerequisite.detail.into())
                        .collect(),
                })
                .collect::<Vec<_>>();
            if choices.is_empty() {
                return Err(format!("UnknownConfigurationGroup: {group}"));
            }
            BrowserConfigurationGroup {
                revision: request.revision,
                label: group,
                choices,
            }
            .presentation()
        }
        "review" => {
            let (reviewed, _, _) = review(BrowserConfigurationSelection {
                catalog_generation: request.catalog_generation,
                implementations: request.implementations,
            })?;
            BrowserConfigurationReviewPresentation {
                revision: request.revision,
                target_id: reviewed.target_id.into(),
                selected_implementations: reviewed.selected_implementations.clone(),
                configuration_id: reviewed.configuration_id.clone(),
                profile_id: reviewed.profile_id.clone(),
                output: reviewed.output.into(),
                join_mode: reviewed.join_mode.into(),
                canonical_source: reviewed.canonical_source.clone(),
                does_not_create: reviewed
                    .does_not_create
                    .iter()
                    .map(|value| (*value).into())
                    .collect(),
            }
            .presentation()
        }
        mode => return Err(format!("UnknownConfigurationPresentation: {mode}")),
    }
    .map_err(|error| format!("describe browser configuration: {error:?}"))?;
    semantic
        .lower()
        .map_err(|error| format!("lower browser configuration: {error:?}"))?
        .encode()
        .map_err(|error| format!("encode browser configuration: {error:?}"))
}

fn reviewed_selection(implementations: &[String]) -> Result<BTreeSet<&str>, String> {
    if implementations.len() > BROWSER_IMPLEMENTATIONS.len() {
        return Err("SelectionBound: browser selection exceeds the reviewed catalog bound".into());
    }
    let mut selected = BTreeSet::new();
    for implementation in implementations {
        if !BROWSER_IMPLEMENTATIONS
            .iter()
            .any(|descriptor| descriptor.implementation_id == implementation)
        {
            return Err(format!("StaleImplementation: {implementation}"));
        }
        if !selected.insert(implementation.as_str()) {
            return Err(format!("DuplicateImplementation: {implementation}"));
        }
    }
    Ok(selected)
}

#[no_mangle]
pub extern "C" fn conduit_creche_browser_configuration_view(length: usize) -> i32 {
    super::super::abi::clear_output();
    let bytes = match super::super::abi::take_input(length) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    match serde_json::from_slice::<BrowserConfigurationViewRequest>(&bytes)
        .map_err(|error| format!("InvalidConfigurationPresentation: {error}"))
        .and_then(presentation_view)
        .and_then(|encoded| {
            super::super::abi::write_output_bytes(&encoded)
                .map_err(|_| "ConfigurationPresentationOutputBound".to_string())
        }) {
        Ok(()) => 0,
        Err(message) => super::super::abi::refuse(message, super::super::abi::ERROR_SPORE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creche::browser_configuration::DEFAULT_IMPLEMENTATIONS;

    #[test]
    fn configuration_views_are_product_semantics_with_bounded_catalog_inputs() {
        let encoded = presentation_view(BrowserConfigurationViewRequest {
            revision: 1,
            mode: "group".into(),
            catalog_generation: CATALOG_GENERATION,
            implementations: DEFAULT_IMPLEMENTATIONS
                .iter()
                .map(|value| (*value).into())
                .collect(),
            group: Some("Presentation".into()),
            diagnostic: None,
        })
        .unwrap();
        let view = conduit_presentation::ApplicationView::decode(&encoded).unwrap();
        assert!(view
            .nodes
            .iter()
            .any(|node| node.key == "configuration-group-options"));
        assert!(view
            .actions
            .iter()
            .all(|action| action.id.starts_with("implementation.change-")));

        let unknown = presentation_view(BrowserConfigurationViewRequest {
            revision: 2,
            mode: "group".into(),
            catalog_generation: CATALOG_GENERATION,
            implementations: vec!["browser/unknown@1".into()],
            group: Some("Presentation".into()),
            diagnostic: None,
        })
        .unwrap_err();
        assert!(unknown.contains("StaleImplementation"));
    }
}
