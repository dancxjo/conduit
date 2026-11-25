//! Prepare wallpaper metadata from the hosted APOD deck.
use super::acquisition::{PreparedDeck, PreparedPhoto};
#[cfg(test)]
use conduit_core::StructuredInfoTypeShape;
use conduit_core::{
    BoundedResourceRef, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity, StructuredInfoType, StructuredInfoValue,
};
use conduit_face::{ImageColorProfile, ImageFormat, ImagePixelExtent, ImageResource};
#[cfg(test)]
use conduit_plot::{rust_binding, rust_binding::NativeRustBinding};
use conduit_semantic_catalog::{
    PreparedWallpaperSelection, SelectionRefusal, WallpaperImageMetadata,
};

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
    .map_err(contract)?;
    let source = format!("NASA APOD/{}", photo.candidate().date);
    WallpaperImageMetadata {
        title: &photo.candidate().title,
        caption: "",
        attribution: photo
            .candidate()
            .credit
            .as_deref()
            .unwrap_or("Creator credit not supplied by NASA APOD"),
        license: "License not supplied by provider",
        source: &source,
    }
    .compose(ty, image)
}

#[cfg(test)]
#[path = "selection_wire/tests.rs"]
mod tests;
