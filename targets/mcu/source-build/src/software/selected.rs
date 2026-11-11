//! Feed explicit definitions into shared checked metadata preparation.
//! Application role selection remains with the caller, not the shared helper.
use super::{p, SoftwareNode};
use conduit_embedded_build::{
    prepare_selected_software, SelectedSoftwareBounds, SelectedSoftwareOffer,
};

pub(super) fn prepare_images(
    source: &p::ExpandedCanonicalPlot,
    nodes: &mut [SoftwareNode],
    maximum_image_bytes: usize,
) -> Result<(), String> {
    let selected = nodes
        .iter()
        .filter_map(|node| {
            node.offer.as_ref().map(|offer| SelectedSoftwareOffer {
                node: node.node,
                offer,
                configuration: &node.configuration,
            })
        })
        .collect::<Vec<_>>();
    // Preserve the existing per-offer ceiling; the checked finite selection
    // determines its aggregate. This is fabrication extent, not a Root grant.
    let total = maximum_image_bytes
        .checked_mul(selected.len())
        .ok_or("software aggregate image ceiling overflow")?;
    let images = prepare_selected_software(
        source,
        &selected,
        SelectedSoftwareBounds {
            maximum_nodes: source.gears.len(),
            maximum_offers: nodes.len(),
            maximum_image_bytes,
            maximum_total_image_bytes: total,
        },
    )?
    .into_iter()
    .map(|prepared| (prepared.node, prepared.image))
    .collect::<Vec<_>>();
    for (ordinal, image) in images {
        let node = nodes
            .iter_mut()
            .find(|node| node.node == ordinal)
            .ok_or("prepared software node absent")?;
        node.offer_image = Some(image);
    }
    Ok(())
}
