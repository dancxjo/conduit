//! Immutable Make-checked scan meaning; current possession is never encoded here.
mod material {
    include!(concat!(env!("OUT_DIR"), "/scan-software.rs"));
}
/// Borrow trusted Source pins directly from linked facts instead of retaining
/// another owned identity in each dormant Root/execution slot.
pub fn identity_ref(
) -> Result<(&'static str, &'static str, &'static str), conduit_plot::flat_image::ImageRefusal> {
    let f = material::linked_planning_facts()?;
    Ok((f.source_document_id, f.checked_plot_id, f.expanded_plot_id))
}
pub fn load() -> Result<
    conduit_plot::flat_image::indexed::IndexedSourceImage<'static>,
    conduit_plot::flat_image::ImageRefusal,
> {
    use sha2::{Digest, Sha256};
    let image = include_bytes!(concat!(env!("OUT_DIR"), "/scan-source.bin"));
    let expected = include_bytes!(concat!(env!("OUT_DIR"), "/scan-source.sha256"));
    let actual: [u8; 32] = Sha256::digest(image).into();
    if actual.as_slice() != expected {
        return Err(conduit_plot::flat_image::ImageRefusal::Representation);
    }
    // SAFETY: image, independently linked digest and complete constructors all
    // come from this build's shared checker/expander. No network input supplies
    // a graph or its claimed identity at this trust boundary.
    unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image_with_decoders(
            image,
            material::decode_planning_gear,
            material::decode_planning_configuration,
        )
        .map(|view| view.with_make_planning_facts(material::linked_planning_facts))
    }
}
