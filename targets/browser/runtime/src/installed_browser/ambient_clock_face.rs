//! Present the admitted clock observation through the ordinary browser Show.
use super::{
    factory::{
        manifestation_label, validate_placement, BrowserHostResult, BrowserInstallation,
        BrowserManifestation,
    },
    BrowserBack,
};
use conduit_core::{
    kind_id, present_host_call_requirement, resource_requirement, ArtifactId, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, ImplementationId,
    PlannedGear, FACE_RESOURCE_CLASS,
};
use conduit_semantic_catalog as catalog;

const ID: &str = "browser/face-ambient-clock-weather@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: ID,
    offer,
    prepare,
    perform: Some(present),
};

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        catalog::ambient_clock_face_kind().expect("checked ambient Face Kind"),
        Back {
            capability_id: CapabilityId::from(ID),
            execution_profile_id: ExecutionProfileId::from(ID),
            implementation_id: ImplementationId::from(ID),
            artifact_id: ArtifactId::from("conduit-browser-runtime/ambient-clock-face@1"),
            host_calls: vec![present_host_call_requirement(
                kind_id(catalog::AMBIENT_CLOCK_FACE_KIND),
                catalog::AMBIENT_CLOCK_OBSERVATION_BYTES,
            )],
            resource_requirements: vec![resource_requirement(FACE_RESOURCE_CLASS, 1)],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserBack::face(
        catalog::AMBIENT_CLOCK_OBSERVATION_BYTES,
        1,
    ))
}

fn present(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            data_label: manifestation_label(placement)?,
            kind_id: catalog::AMBIENT_CLOCK_FACE_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

pub(crate) fn text(input: &[u8]) -> Result<String, String> {
    let value = conduit_core::validate_canonical_structured_value(input)
        .map_err(|error| format!("decode ambient Face: {error:?}"))?;
    let field = |name| {
        value
            .record_field(name)
            .map_err(|error| format!("{error:?}"))?
            .ok_or_else(|| format!("missing ambient Face field: {name}"))
    };
    let label = |name| {
        let bytes = field(name)?
            .primitive_bytes(conduit_core::TEXT_INFO_ID)
            .map_err(|error| format!("{error:?}"))?;
        core::str::from_utf8(bytes).map_err(|error| format!("{error:?}"))
    };
    let date = label("local_date")?;
    let time = label("local_time")?;
    let timezone = label("timezone")?;
    let weather = field("weather")?
        .variant_tag()
        .map_err(|error| format!("{error:?}"))?;
    Ok(format!("{date} · {time} · {timezone} · Weather {weather}"))
}
