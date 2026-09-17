//! Catalog-derived browser Host configuration authoring and checking.

use std::collections::{BTreeMap, BTreeSet};

use conduit_host_browser_fabrication::{
    BrowserFabricationPackage, BROWSER_CAPABILITY_INTENTS, BROWSER_IMPLEMENTATIONS,
    REVIEWED_DISTRIBUTION_ID, REVIEWED_RUNTIME_ARTIFACT,
};
use conduit_host_fabrication::{
    canonical_host_configuration_conduit, check_host_configuration, resolve_host_capability_intent,
    CapabilityIntent, ConfigurationBase, ConfigurationTarget, FabricationCatalog,
    FabricationContribution, FabricationPackageSet, HostCapabilityIntent, HostCapabilityReview,
    HostConfiguration, HostFabricationPackage,
};
use serde::{Deserialize, Serialize};

mod presentation;

pub(super) const CATALOG_GENERATION: u32 = 1;
const CONFIGURATION_NAME: &str = "creche-browser-page";
const DEFAULT_IMPLEMENTATIONS: &[&str] = &[
    "browser/dom@1",
    "browser/keyboard-events@1",
    "browser/pointer-events@1",
];

#[derive(Serialize)]
pub(super) struct BrowserConfigurationCatalog {
    schema: &'static str,
    generation: u32,
    target_id: &'static str,
    distribution_id: &'static str,
    runtime_artifact: &'static str,
    defaults: Vec<&'static str>,
    entries: Vec<CatalogEntry>,
    semantics: CatalogSemantics,
}

#[derive(Serialize)]
struct CatalogEntry {
    group: &'static str,
    label: &'static str,
    base_kind: &'static str,
    implementation_id: &'static str,
    implementation_revision: u32,
    maximum_instances: u32,
    maximum_buffered_bytes: u64,
    runtime_prerequisites: Vec<RuntimePrerequisite>,
}

#[derive(Serialize)]
struct RuntimePrerequisite {
    kind: &'static str,
    detail: &'static str,
    satisfied: bool,
}

#[derive(Serialize)]
struct CatalogSemantics {
    selection_means: &'static str,
    does_not_create: [&'static str; 7],
}

#[derive(Serialize)]
struct BrowserCapabilityIntentCatalog {
    schema: &'static str,
    generation: u32,
    target_id: &'static str,
    defaults: Vec<&'static str>,
    entries: Vec<BrowserCapabilityIntentEntry>,
}

#[derive(Serialize)]
struct BrowserCapabilityIntentEntry {
    kind: &'static str,
    label: &'static str,
    implementation_id: &'static str,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BrowserCapabilityIntentSelection {
    catalog_generation: u32,
    capabilities: Vec<CapabilityIntent>,
}

#[derive(Serialize)]
struct BrowserCapabilityIntentConfigurationReview {
    schema: &'static str,
    capability_review: HostCapabilityReview,
    configuration_selection: BrowserConfigurationSelection,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BrowserConfigurationSelection {
    pub(super) catalog_generation: u32,
    pub(super) implementations: Vec<String>,
}

fn capability_intent_catalog() -> BrowserCapabilityIntentCatalog {
    BrowserCapabilityIntentCatalog {
        schema: "conduit.host/browser-capability-intent-catalog@1",
        generation: CATALOG_GENERATION,
        target_id: super::spore_target::BROWSER_PAGE_TARGET_ID,
        defaults: BROWSER_CAPABILITY_INTENTS[..2]
            .iter()
            .map(|descriptor| descriptor.kind)
            .collect(),
        entries: BROWSER_CAPABILITY_INTENTS
            .iter()
            .map(|descriptor| BrowserCapabilityIntentEntry {
                kind: descriptor.kind,
                label: descriptor.label,
                implementation_id: descriptor.implementation_id,
            })
            .collect(),
    }
}

fn review_capability_intent(
    selection: BrowserCapabilityIntentSelection,
) -> Result<BrowserCapabilityIntentConfigurationReview, String> {
    if selection.catalog_generation != CATALOG_GENERATION {
        return Err(format!(
            "StaleCatalogGeneration: saved browser intent uses generation {}, current generation is {}",
            selection.catalog_generation, CATALOG_GENERATION
        ));
    }
    let package = BrowserFabricationPackage;
    let packages = FabricationPackageSet::compose(&[&package])
        .map_err(|error| format!("compose browser fabrication package: {error:?}"))?;
    let catalog = FabricationCatalog::canonical().with_packages(&packages);
    let capability_review = resolve_host_capability_intent(
        &HostCapabilityIntent {
            target: super::spore_target::BROWSER_PAGE_TARGET_ID.into(),
            capabilities: selection.capabilities,
        },
        &catalog,
        &packages,
    )
    .map_err(|error| format!("CapabilityIntentRefused: {error:?}"))?;
    let mut implementations = BTreeSet::new();
    for capability in &capability_review.capabilities {
        let candidate = capability.candidates.first().ok_or_else(|| {
            format!(
                "CapabilityIntentRefused: {} has no candidate",
                capability.kind
            )
        })?;
        for base in &candidate.prerequisites.bases {
            let implementation = base.compatible_implementations.first().ok_or_else(|| {
                format!(
                    "CapabilityIntentRefused: {} has no exact implementation",
                    base.base_kind
                )
            })?;
            implementations.insert(implementation.implementation_id.clone());
        }
    }
    Ok(BrowserCapabilityIntentConfigurationReview {
        schema: "conduit.host/browser-capability-configuration-review@1",
        capability_review,
        configuration_selection: BrowserConfigurationSelection {
            catalog_generation: CATALOG_GENERATION,
            implementations: implementations.into_iter().collect(),
        },
    })
}

#[derive(Serialize)]
pub(super) struct BrowserConfigurationReview {
    schema: &'static str,
    catalog_generation: u32,
    target_id: &'static str,
    selected_implementations: Vec<String>,
    selected_bases: Vec<ReviewedBase>,
    pub(super) canonical_source: String,
    pub(super) configuration_id: String,
    pub(super) profile_id: String,
    limits: conduit_host_fabrication::HostBounds,
    output: &'static str,
    join_mode: &'static str,
    does_not_create: [&'static str; 7],
}

#[derive(Serialize)]
struct ReviewedBase {
    kind: String,
    implementations: Vec<String>,
}

pub(super) fn catalog() -> BrowserConfigurationCatalog {
    BrowserConfigurationCatalog {
        schema: "conduit.creche/browser-configuration-catalog@1",
        generation: CATALOG_GENERATION,
        target_id: super::spore_target::BROWSER_PAGE_TARGET_ID,
        distribution_id: REVIEWED_DISTRIBUTION_ID,
        runtime_artifact: REVIEWED_RUNTIME_ARTIFACT,
        defaults: DEFAULT_IMPLEMENTATIONS.to_vec(),
        entries: BROWSER_IMPLEMENTATIONS
            .iter()
            .map(|item| CatalogEntry {
                group: item.group,
                label: item.label,
                base_kind: item.base_kind,
                implementation_id: item.implementation_id,
                implementation_revision: item.implementation_revision,
                maximum_instances: item.maximum_instances,
                maximum_buffered_bytes: item.maximum_buffered_bytes,
                runtime_prerequisites: item
                    .prerequisites
                    .iter()
                    .map(|prerequisite| RuntimePrerequisite {
                        kind: prerequisite.kind,
                        detail: prerequisite.detail,
                        satisfied: false,
                    })
                    .collect(),
            })
            .collect(),
        semantics: CatalogSemantics {
            selection_means:
                "include this reviewed structural implementation in the Host PROFILE/IMAGE",
            does_not_create: lifecycle_absences(),
        },
    }
}

pub(super) fn review(
    selection: BrowserConfigurationSelection,
) -> Result<
    (
        BrowserConfigurationReview,
        conduit_host_fabrication::CheckedHostConfiguration,
        FabricationPackageSet,
    ),
    String,
> {
    if selection.catalog_generation != CATALOG_GENERATION {
        return Err(format!(
            "StaleCatalogGeneration: saved browser selection uses generation {}, current generation is {}",
            selection.catalog_generation, CATALOG_GENERATION
        ));
    }
    if selection.implementations.len() > BROWSER_IMPLEMENTATIONS.len() {
        return Err("SelectionBound: browser selection exceeds the reviewed catalog bound".into());
    }
    let mut seen = BTreeSet::new();
    let mut by_kind = BTreeMap::<String, Vec<String>>::new();
    for implementation in selection.implementations {
        if !seen.insert(implementation.clone()) {
            return Err(format!("DuplicateImplementation: {implementation}"));
        }
        let descriptor = BROWSER_IMPLEMENTATIONS
            .iter()
            .find(|item| item.implementation_id == implementation)
            .ok_or_else(|| format!("StaleImplementation: {implementation} is absent from catalog generation {CATALOG_GENERATION}"))?;
        by_kind
            .entry(descriptor.base_kind.into())
            .or_default()
            .push(implementation);
    }
    for implementations in by_kind.values_mut() {
        implementations.sort();
    }
    let package = BrowserFabricationPackage;
    let FabricationContribution::Anchor(anchor) = package.contribution() else {
        return Err("browser fabrication package is not an anchor".into());
    };
    let target = anchor
        .targets
        .first()
        .ok_or_else(|| "browser fabrication package omitted its page target".to_string())?;
    let configuration = HostConfiguration {
        schema: 1,
        name: CONFIGURATION_NAME.into(),
        target: ConfigurationTarget {
            architecture: target.architecture.clone(),
            machine: target.machine.clone(),
            board: target.board.clone(),
            os: target.os.clone(),
            fabrication_descriptor: None,
        },
        bases: by_kind
            .iter()
            .map(|(kind, implementations)| ConfigurationBase {
                kind: kind.clone(),
                implementation: implementations.first().cloned(),
                implementations: implementations.iter().skip(1).cloned().collect(),
            })
            .collect(),
        resources: Vec::new(),
        limits: target.maxima.clone(),
    };
    let canonical_source = canonical_host_configuration_conduit(&configuration)
        .map_err(|error| format!("encode canonical browser Host configuration: {error:?}"))?;
    let packages = FabricationPackageSet::compose(&[&package])
        .map_err(|error| format!("compose browser fabrication package: {error:?}"))?;
    let catalog = FabricationCatalog::canonical().with_packages(&packages);
    let checked = check_host_configuration(configuration, &catalog, &packages)
        .map_err(|errors| format!("IncompatibleSelection: {errors:?}"))?;
    let profile_id =
        conduit_host_fabrication::validate_profile(checked.profile().clone(), &catalog)
            .map_err(|errors| format!("IncompatibleSelection: {errors:?}"))?
            .profile_id()
            .as_str()
            .to_owned();
    let review = BrowserConfigurationReview {
        schema: "conduit.creche/checked-browser-configuration@1",
        catalog_generation: CATALOG_GENERATION,
        target_id: super::spore_target::BROWSER_PAGE_TARGET_ID,
        selected_implementations: seen.into_iter().collect(),
        selected_bases: by_kind
            .into_iter()
            .map(|(kind, implementations)| ReviewedBase {
                kind,
                implementations,
            })
            .collect(),
        canonical_source,
        configuration_id: checked.configuration_id().into(),
        profile_id,
        limits: checked.configuration().limits.clone(),
        output: "browser-bundle",
        join_mode: "self-joining Body spore (separate later step)",
        does_not_create: lifecycle_absences(),
    };
    Ok((review, checked, packages))
}

fn lifecycle_absences() -> [&'static str; 7] {
    [
        "HostId",
        "BootId",
        "current offer",
        "permission grant",
        "acquired resource",
        "Plan",
        "Play",
    ]
}

#[no_mangle]
pub extern "C" fn conduit_creche_browser_configuration_catalog() -> i32 {
    super::abi::clear_output();
    super::abi::write_output(&catalog())
        .map(|()| 0)
        .unwrap_or(super::abi::ERROR_OUTPUT)
}

#[no_mangle]
pub extern "C" fn conduit_browser_host_capability_intent_catalog() -> i32 {
    super::abi::clear_output();
    super::abi::write_output(&capability_intent_catalog())
        .map(|()| 0)
        .unwrap_or(super::abi::ERROR_OUTPUT)
}

#[no_mangle]
pub extern "C" fn conduit_browser_host_review_capability_intent(length: usize) -> i32 {
    super::abi::clear_output();
    let bytes = match super::abi::take_input(length) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    match serde_json::from_slice::<BrowserCapabilityIntentSelection>(&bytes)
        .map_err(|error| format!("InvalidCapabilityIntent: {error}"))
        .and_then(review_capability_intent)
    {
        Ok(review) => super::abi::write_output(&review)
            .map(|()| 0)
            .unwrap_or(super::abi::ERROR_OUTPUT),
        Err(message) => super::abi::refuse(message, super::abi::ERROR_SPORE),
    }
}

#[no_mangle]
pub extern "C" fn conduit_creche_review_browser_configuration(length: usize) -> i32 {
    super::abi::clear_output();
    let bytes = match super::abi::take_input(length) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    match serde_json::from_slice::<BrowserConfigurationSelection>(&bytes)
        .map_err(|error| format!("InvalidSelection: {error}"))
        .and_then(review)
    {
        Ok((review, _, _)) => super::abi::write_output(&review)
            .map(|()| 0)
            .unwrap_or(super::abi::ERROR_OUTPUT),
        Err(message) => super::abi::refuse(message, super::abi::ERROR_SPORE),
    }
}

#[no_mangle]
pub extern "C" fn conduit_creche_prepare_selected_browser_spore(
    digest_length: usize,
    selection_length: usize,
    now_millis: u64,
) -> i32 {
    super::abi::clear_output();
    let total_length = 32usize
        .checked_add(digest_length)
        .and_then(|length| length.checked_add(selection_length));
    if digest_length == 0
        || selection_length == 0
        || total_length.is_none_or(|length| length > super::abi::INPUT_BYTES)
    {
        return super::abi::ERROR_INPUT;
    }
    let bytes = match super::abi::take_input(total_length.expect("bounded input")) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    let mut entropy = [0; 32];
    entropy.copy_from_slice(&bytes[..32]);
    let digest_end = 32 + digest_length;
    let result = core::str::from_utf8(&bytes[32..digest_end])
        .map_err(|_| "selected IMAGE content digest is not UTF-8".to_string())
        .and_then(|digest| {
            serde_json::from_slice::<BrowserConfigurationSelection>(&bytes[digest_end..])
                .map_err(|error| format!("InvalidSelection: {error}"))
                .and_then(|selection| {
                    super::spore::prepare_selected_browser(
                        entropy,
                        now_millis,
                        Some(digest),
                        selection,
                    )
                })
        });
    match result {
        Ok(prepared) => super::abi::write_output(&prepared)
            .map(|()| 0)
            .unwrap_or(super::abi::ERROR_OUTPUT),
        Err(message) => super::abi::refuse(message, super::abi::ERROR_SPORE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_order_has_one_canonical_identity() {
        let first = review(BrowserConfigurationSelection {
            catalog_generation: 1,
            implementations: vec!["browser/pointer-events@1".into(), "browser/dom@1".into()],
        })
        .unwrap()
        .0;
        let second = review(BrowserConfigurationSelection {
            catalog_generation: 1,
            implementations: vec!["browser/dom@1".into(), "browser/pointer-events@1".into()],
        })
        .unwrap()
        .0;
        assert_eq!(first.configuration_id, second.configuration_id);
        assert_eq!(first.profile_id, second.profile_id);
        assert_eq!(first.canonical_source, second.canonical_source);
    }

    #[test]
    fn stale_and_unknown_selections_are_specific_refusals() {
        let stale = match review(BrowserConfigurationSelection {
            catalog_generation: 0,
            implementations: Vec::new(),
        }) {
            Err(error) => error,
            Ok(_) => panic!("stale selection passed"),
        };
        assert!(stale.contains("StaleCatalogGeneration"));
        let unknown = match review(BrowserConfigurationSelection {
            catalog_generation: 1,
            implementations: vec!["browser/retired@1".into()],
        }) {
            Err(error) => error,
            Ok(_) => panic!("unknown selection passed"),
        };
        assert!(unknown.contains("StaleImplementation"));
    }

    #[test]
    fn semantic_intent_resolves_before_exact_configuration_review() {
        let intent = review_capability_intent(BrowserCapabilityIntentSelection {
            catalog_generation: CATALOG_GENERATION,
            capabilities: vec![
                CapabilityIntent {
                    kind: "host-contribution/graphical-presentation".into(),
                    pinned_implementation: None,
                },
                CapabilityIntent {
                    kind: "host-contribution/keyboard-pointer-input".into(),
                    pinned_implementation: None,
                },
            ],
        })
        .unwrap();
        assert_eq!(intent.capability_review.capabilities.len(), 2);
        assert_eq!(
            intent.configuration_selection.implementations,
            vec![
                "browser/dom-presentation@1",
                "browser/dom@1",
                "browser/keyboard-events@1",
                "browser/pointer-events@1",
            ]
        );
        let exact = review(intent.configuration_selection).unwrap().0;
        assert_eq!(exact.selected_bases.len(), 4);
        assert!(exact.configuration_id.starts_with("sha256:"));
        assert!(exact.profile_id.starts_with("sha256:"));
    }
}
