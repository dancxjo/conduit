//! Import the original image resource declarations with their checked owners.
use alloc::{format, string::String};
use conduit_plot::{StartupCatalog, check_syntax_document, parse_syntax_document};

/// Preparation only. The returned catalog retains exact image schema and law
/// custody; it creates no image residence, acquisition or display authority.
pub fn image_resource_source_catalog() -> Result<StartupCatalog, String> {
    let mut source = parse_syntax_document(include_str!("../types.conduit"));
    // Select original declarations, preserving their owning Source bytes and
    // spans. Added dependencies fail checking until explicitly reviewed here.
    // There is no second authored image schema or reconstructed inspection row.
    source.types.retain(|ty| {
        matches!(
            ty.name.text.as_str(),
            "ImageResource" | "ImagePixelExtent" | "ImageFormat" | "ImageColorProfile"
        )
    });
    source.type_forms.clear();
    let mut primitives = StartupCatalog::new();
    primitives.insert_value_kind_alias(
        "ResourceRef",
        conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )?;
    let checked = check_syntax_document(&source, &primitives)
        .map_err(|error| format!("image resource owner Source: {error:?}"))?;
    let root = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "ImageResource")
        .ok_or("original image resource owner is missing")?;
    let generated = crate::ImageResource::semantic_type()
        .map_err(|error| format!("generated image resource Type: {error:?}"))?;
    if root.value_type != generated {
        return Err("image resource owner differs from generated semantic Type".into());
    }
    let mut catalog = StartupCatalog::new();
    catalog.insert_checked_native_type_from_document("ImageResource", &checked, &root.identity)?;
    Ok(catalog)
}
