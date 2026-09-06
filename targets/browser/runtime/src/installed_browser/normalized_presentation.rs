//! Exact normalized-duration realization of the ordinary structured presenter.
use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{CapabilityOffer, PlannedGear, StructuredInfoValue, StructuredInfoValueShape};

pub(crate) const IMPLEMENTATION: &str = "browser/presentation-normalized-durations@1";
pub(super) static PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: Some(present),
};
pub(super) fn offer() -> CapabilityOffer {
    let mut offer = crate::structured_offers::structured_presentation_offer(
        conduit_semantic_catalog::NORMALIZED_SEQUENCE_TYPE,
        &conduit_semantic_catalog::normalized_duration_sequence_type(),
        crate::structured_offers::BrowserOfferIdentity {
            capability: IMPLEMENTATION,
            profile: "browser/normalized-duration-presentation@1",
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/normalized-duration-presentation@1",
        },
    );
    offer.host_operations[0].maximum_input_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
    offer.limits.max_queue_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
    offer
}
pub(super) fn install_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    let offer = offer();
    startup.insert(conduit_form::KindSignature {
        kind: offer.kind_id.as_str().into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            inputs: offer.inputs,
            outputs: offer.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserOperation::presentation(
        super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}
pub(crate) fn text(input: &[u8]) -> Result<String, String> {
    let value = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode normalized presentation: {error:?}"))?;
    if value.value_type() != &conduit_semantic_catalog::normalized_duration_sequence_type() {
        return Err("normalized presentation has the wrong exact type".into());
    }
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err("normalized sequence must be a record".into());
    };
    let mut rendered = Vec::new();
    for field in fields {
        let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
            return Err("normalized field must be a leaf".into());
        };
        rendered.push(format!(
            "{}: {}",
            field.name(),
            core::str::from_utf8(bytes).map_err(|_| "normalized field is not UTF-8")?
        ));
    }
    Ok(rendered.join(" · "))
}
fn present(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    text(input)?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}
