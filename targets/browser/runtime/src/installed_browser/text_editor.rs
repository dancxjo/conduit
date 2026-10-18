//! DOM editor realization: document in, one committed edit out through Host Call.
use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    kind_id, ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallRequirement, ImplementationId, PlannedGear, FACE_RESOURCE_CLASS,
};
use conduit_kernel::HostedValueStore;
pub(crate) const HOST_CALL: &str = "conduit.host/browser-text-editor@1";
const IMPLEMENTATION: &str = "browser/dom-text-editor@1";
pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};
fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::face_editor_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/text-editor@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HOST_CALL.into(),
                target_kind: Some(kind_id("face/dom-editor@1")),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES,
                maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES,
            }],
            resource_requirements: vec![conduit_core::resource_requirement(FACE_RESOURCE_CLASS, 1)],
            authority_requirements: Vec::new(),
        },
    )
    .build()
}
fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserBack::unary(
        conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES,
        1,
    ))
}
#[derive(Debug, serde::Serialize)]
pub(crate) struct EditorDocument {
    pub revision: String,
    pub text: String,
    pub label: Option<String>,
}
pub(crate) fn document(placement: &PlannedGear, input: &[u8]) -> Result<EditorDocument, String> {
    let document = conduit_semantic_catalog::TextDocument::decode(input)
        .map_err(|e| format!("editor document: {e:?}"))?;
    Ok(EditorDocument {
        revision: document.revision.to_string(),
        text: core::str::from_utf8(document.text)
            .map_err(|_| "editor UTF-8")?
            .into(),
        label: super::factory::manifestation_label(placement)?,
    })
}
