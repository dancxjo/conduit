//! Exact normalized-duration realization of the ordinary structured presenter.
use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserBack;
use conduit_core::{CapabilityOffer, PlannedGear, StructuredInfoValue, StructuredInfoValueShape};

pub(crate) const IMPLEMENTATION: &str = "browser/face-normalized-durations@1";
pub(super) static FACE: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: Some(present),
};
pub(super) fn offer() -> CapabilityOffer {
    let mut offer = crate::structured_offers::structured_face_offer(
        conduit_semantic_catalog::NORMALIZED_SEQUENCE_TYPE,
        &conduit_semantic_catalog::normalized_duration_sequence_type(),
        crate::structured_offers::BrowserOfferIdentity {
            capability: IMPLEMENTATION,
            profile: "browser/normalized-duration-face@1",
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/normalized-duration-face@1",
        },
    );
    offer.host_calls[0].maximum_input_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
    offer.limits.max_queue_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
    offer
}
pub(super) fn install_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    let contract = conduit_semantic_catalog::structured_face_semantic_contract(
        conduit_semantic_catalog::NORMALIZED_SEQUENCE_TYPE,
        &conduit_semantic_catalog::normalized_duration_sequence_type(),
    );
    startup.insert(conduit_plot::KindSignature {
        kind: contract.kind_id.as_str().into(),
        startup_parameters: conduit_semantic_catalog::data_label_startup_signature(),
    })?;
    profile
        .insert_kind(contract)
        .map_err(|error| error.to_string())
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserBack::face(
        super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}
pub(crate) fn text(input: &[u8]) -> Result<String, String> {
    let value = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode normalized face: {error:?}"))?;
    if value.value_type() != &conduit_semantic_catalog::normalized_duration_sequence_type() {
        return Err("normalized face has the wrong exact type".into());
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
fn present(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    text(input)?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            data_label: super::factory::manifestation_label(placement)?,
            kind_id: conduit_semantic_catalog::STRUCTURED_FACE_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}
