//! Workspace-owned browser edges for the optional generative tutorial Presenter.

use super::factory::{validate_placement, BrowserHostResult, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    resource_requirement, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer,
    ExecutionProfileId, HostCallRequirement, ImplementationId, PlannedGear,
};
use conduit_kernel::HostedValueStore;

pub(crate) const REQUEST_IMPLEMENTATION: &str = "browser/workspace-tutorial-presenter-request@1";
pub(crate) const MANIFESTATION_IMPLEMENTATION: &str = "browser/workspace-tutorial-manifestation@1";
pub(crate) const REQUEST_OPERATION: &str =
    "conduit.host/workspace-current-tutorial-presenter-request@1";
const MANIFESTATION_OPERATION: &str = "conduit.host/browser-present-generated-manifestation@1";
const ARTIFACT: &str = "conduit-browser-runtime/workspace-tutorial-presenter@1";
const REQUEST_BYTES: u32 = conduit_ai::MAXIMUM_LLM_INPUT_BYTES as u32;
const MANIFESTATION_BYTES: u32 = conduit_ai::MAXIMUM_LLM_OUTPUT_BYTES as u32;

pub(super) static REQUEST: BrowserInstallation = BrowserInstallation {
    implementation_id: REQUEST_IMPLEMENTATION,
    offer: request_offer,
    prepare: prepare_request,
    perform: None,
};

pub(super) static MANIFESTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: MANIFESTATION_IMPLEMENTATION,
    offer: manifestation_offer,
    prepare: prepare_manifestation,
    perform: Some(perform_manifestation),
};

fn request_offer() -> CapabilityOffer {
    offer(
        conduit_tutorial_form::presenter::request_contract(),
        REQUEST_IMPLEMENTATION,
        REQUEST_OPERATION,
        0,
        REQUEST_BYTES,
        false,
    )
}

fn manifestation_offer() -> CapabilityOffer {
    offer(
        conduit_tutorial_form::presenter::manifestation_contract(),
        MANIFESTATION_IMPLEMENTATION,
        MANIFESTATION_OPERATION,
        MANIFESTATION_BYTES,
        0,
        true,
    )
}

fn offer(
    contract: conduit_core::Kind,
    implementation: &'static str,
    operation: &'static str,
    input: u32,
    output: u32,
    presentation: bool,
) -> CapabilityOffer {
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(
                "browser/workspace-tutorial-presenter@1",
            ),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: operation.into(),
                target_kind: None,
                maximum_in_flight: 1,
                maximum_input_bytes: input,
                maximum_output_bytes: output,
            }],
            resource_requirements: if presentation {
                vec![resource_requirement(
                    conduit_core::PRESENTATION_RESOURCE_CLASS,
                    1,
                )]
            } else {
                Vec::new()
            },
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn prepare_request(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &request_offer())?;
    BrowserBack::host_source(values, REQUEST_BYTES)
}

fn prepare_manifestation(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &manifestation_offer())?;
    Ok(BrowserBack::presentation(MANIFESTATION_BYTES, 1))
}

fn perform_manifestation(
    _placement: &PlannedGear,
    input: &[u8],
) -> Result<BrowserHostResult, String> {
    refuse_unvalidated_candidate(input)
}

fn refuse_unvalidated_candidate(input: &[u8]) -> Result<BrowserHostResult, String> {
    serde_json::from_slice::<conduit_presentation::GeneratedManifestationCandidate>(input)
        .map_err(|error| format!("decode generated tutorial manifestation candidate: {error}"))?;
    Err(
        "generated tutorial manifestation candidate requires semantic validation before browser presentation"
            .into(),
    )
}

pub(crate) fn install_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    conduit_tutorial_form::presenter::install_tutorial_presenter_catalog(startup, profile)
}

#[cfg(test)]
mod tests {
    use super::refuse_unvalidated_candidate;

    #[test]
    fn tutorial_presenter_form_uses_the_planned_llm_present_front() {
        let (startup, profile) = crate::installed_browser::catalogs().unwrap();
        let syntax = conduit_form::parse_syntax_document(include_str!(
            "../../../../../forms/orifina-tutorial-presenter/main.conduit"
        ));
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_form::expand_canonical_form_for_authoring(
            &checked,
            "orifina-tutorial-presenter",
            &profile,
        )
        .unwrap()
        .expanded;
        assert_eq!(
            expanded
                .gears
                .iter()
                .map(|gear| gear.kind_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                conduit_tutorial_form::presenter::MANIFESTATION_KIND,
                conduit_ai::LLM_PRESENT_KIND,
                conduit_tutorial_form::presenter::REQUEST_KIND,
            ]
        );
        assert!(conduit_planner::default_expanded_placements(
            &expanded,
            &[crate::installed_browser::advertisement(
                "browser/tutorial-presenter".into(),
                "boot/tutorial-presenter".into(),
            )],
        )
        .is_err());
    }

    #[test]
    fn provider_candidate_is_not_accepted_as_a_browser_manifestation() {
        let candidate = serde_json::json!({
            "candidate_identity": "candidate/provider-output",
            "request_identity": "request/tutorial",
            "source_presentation_identity": "presentation/tutorial",
            "source_presentation_revision": 1,
            "presenter_implementation_identity": "presenter/browser@1",
            "provider_identity": "provider/fixture@1",
            "model_identity": "model/fixture@1",
            "template_contract_revision": "template/tutorial@1",
            "mask_contract_revision": "mask/tutorial@1",
            "generation_run_identity": "run/tutorial-1",
            "disposition": "Produced",
            "content": [{ "role": "Speech", "source_text_index": 0, "bytes": [78, 69, 86, 69, 82, 32, 82, 69, 78, 68, 69, 82] }],
            "affordances": [],
            "correlations": []
        });
        let bytes = serde_json::to_vec(&candidate).unwrap();
        let error = match refuse_unvalidated_candidate(&bytes) {
            Ok(_) => panic!("provider candidate crossed validation boundary"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            "generated tutorial manifestation candidate requires semantic validation before browser presentation"
        );
        assert!(!error.contains("NEVER RENDER"));
    }
}
