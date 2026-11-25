//! Prepare wallpaper metadata from the hosted APOD deck.
use super::acquisition::{PreparedDeck, PreparedPhoto};
use conduit_core::{
    BoundedResourceRef, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity, StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape,
    StructuredInfoValue,
};
use conduit_face::{ImageColorProfile, ImageFormat, ImagePixelExtent, ImageResource};
use conduit_plot::{rust_binding, rust_binding::NativeRustBinding};
use conduit_semantic_catalog::{PreparedWallpaperSelection, SelectionRefusal};

type Result<T> = core::result::Result<T, SelectionRefusal>;

fn contract<T>(_: T) -> SelectionRefusal {
    SelectionRefusal::NativeContract
}

pub fn prepare(deck: &PreparedDeck, rotation_seconds: u64) -> Result<PreparedWallpaperSelection> {
    let types = conduit_semantic_catalog::prepare_ambient_display_types().map_err(contract)?;
    let ty = types
        .native_types
        .iter()
        .find(|ty| ty.name == "AmbientWallpaperImage")
        .ok_or(SelectionRefusal::NativeContract)?;
    let metadata = deck
        .photos()
        .iter()
        .map(|photo| photo_metadata(photo, &ty.value_type))
        .collect::<Result<Vec<_>>>()?;
    PreparedWallpaperSelection::prepare(metadata.try_into().map_err(contract)?, rotation_seconds)
}

fn record(
    ty: &StructuredInfoType,
    mut value: impl FnMut(&str, &StructuredInfoType) -> Result<StructuredInfoValue>,
) -> Result<StructuredInfoValue> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(contract(()));
    };
    let fields = fields
        .iter()
        .map(|f| {
            StructuredFieldValue::new(f.name(), value(f.name(), f.value_type())?).map_err(contract)
        })
        .collect::<Result<Vec<_>>>()?;
    StructuredInfoValue::record(ty.clone(), fields).map_err(contract)
}
fn photo_metadata(photo: &PreparedPhoto, ty: &StructuredInfoType) -> Result<StructuredInfoValue> {
    let pixel_bytes = u64::from(photo.width())
        .checked_mul(u64::from(photo.height()))
        .and_then(|n| n.checked_mul(4))
        .ok_or(SelectionRefusal::InvalidMetadata)?;
    if pixel_bytes != photo.rgba().len() as u64 || pixel_bytes == 0 {
        return Err(SelectionRefusal::InvalidMetadata);
    }
    let image = ImageResource::new(
        ImageColorProfile::absent(),
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(*photo.original_sha256()),
            content_profile: "media/image-rgba8@1".into(),
            access_class: "conduit.resource/image-content@1".into(),
            extent: ResourceExtent {
                bytes: pixel_bytes,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest(*photo.version_sha256()),
                expires_at: None,
            },
        },
        ImagePixelExtent::new(u64::from(photo.height()), u64::from(photo.width()))
            .map_err(contract)?,
        ImageFormat::rgba8(),
        format!("apod/{}", photo.candidate().date),
    )
    .map_err(contract)?
    .into_structured()
    .map_err(contract)?;
    let source = format!("NASA APOD/{}", photo.candidate().date);
    record(ty, |name, ty| {
        if name == "image" {
            return Ok(image.clone());
        }
        let text = match name {
            "title" => photo.candidate().title.as_str(),
            "caption" => "",
            "attribution" => photo
                .candidate()
                .credit
                .as_deref()
                .unwrap_or("Creator credit not supplied by NASA APOD"),
            "license" => "License not supplied by provider",
            "source" => &source,
            _ => return Err(contract(())),
        };
        rust_binding::primitive_into_structured(ty.clone(), &text.to_owned()).map_err(contract)
    })
}
#[cfg(test)]
#[path = "selection_wire/tests.rs"]
mod tests;
