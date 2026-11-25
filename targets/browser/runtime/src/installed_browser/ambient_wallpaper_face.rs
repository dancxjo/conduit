//! Present the admitted wallpaper observation through the ordinary browser Show.
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

const ID: &str = "browser/face-ambient-wallpaper@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: ID,
    offer,
    prepare,
    perform: Some(present),
};

fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        catalog::resident_wallpaper_face_kind().expect("checked ambient Face Kind"),
        Back {
            capability_id: CapabilityId::from(ID),
            execution_profile_id: ExecutionProfileId::from(ID),
            implementation_id: ImplementationId::from(ID),
            artifact_id: ArtifactId::from("conduit-browser-runtime/ambient-wallpaper-face@1"),
            host_calls: vec![present_host_call_requirement(
                kind_id(catalog::RESIDENT_WALLPAPER_FACE_KIND),
                catalog::RESIDENT_WALLPAPER_BYTES,
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
    Ok(BrowserBack::face(catalog::RESIDENT_WALLPAPER_BYTES, 1))
}

fn present(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            data_label: manifestation_label(placement)?,
            kind_id: catalog::RESIDENT_WALLPAPER_FACE_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_face::ImageResource;
use conduit_plot::rust_binding::{self, NativeRustBinding};

#[derive(Debug, serde::Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub(crate) enum WallpaperPresentation {
    Available {
        title: String,
        caption: String,
        attribution: String,
        license: String,
        source: String,
        reference: Box<WallpaperReference>,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WallpaperReference {
    identity: String,
    version: String,
    width: u64,
    height: u64,
    bytes: u64,
    content_profile: String,
    access_class: String,
}

pub(crate) fn decode(input: &[u8]) -> Result<WallpaperPresentation, String> {
    let value = StructuredInfoValue::from_canonical_bytes(input).map_err(debug_error)?;
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        return Err("expected a wallpaper availability observation".into());
    };
    match tag {
        "unavailable" => Ok(WallpaperPresentation::Unavailable {
            reason: rust_binding::primitive_from_structured(payload).map_err(debug_error)?,
        }),
        "available" => {
            let metadata = field(payload, "image")?;
            let image = ImageResource::from_structured(field(metadata, "image")?.clone())
                .map_err(debug_error)?;
            let resource = image.content();
            let text = |name| {
                rust_binding::primitive_from_structured(field(metadata, name)?).map_err(debug_error)
            };
            Ok(WallpaperPresentation::Available {
                title: text("title")?,
                caption: text("caption")?,
                attribution: text("attribution")?,
                license: text("license")?,
                source: text("source")?,
                reference: Box::new(WallpaperReference {
                    identity: digest_text(resource.identity.digest()),
                    version: digest_text(resource.lifetime.version.digest()),
                    width: *image.extent().width(),
                    height: *image.extent().height(),
                    bytes: resource.extent.bytes,
                    content_profile: resource.content_profile.as_str().into(),
                    access_class: resource.access_class.as_str().into(),
                }),
            })
        }
        _ => Err("unsupported browser wallpaper observation availability".into()),
    }
}

fn field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, String> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err("expected a wallpaper observation record".into());
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value())
        .ok_or_else(|| format!("missing wallpaper field {name}"))
}

fn digest_text(digest: [u8; 32]) -> String {
    use core::fmt::Write;
    let mut text = String::with_capacity(64);
    for byte in digest {
        write!(&mut text, "{byte:02x}").expect("writing to a String");
    }
    text
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("decode wallpaper presentation: {error:?}")
}
