//! Explicit software-definition inputs for fabrication, never Ready custody.
use conduit_core as c;
use conduit_plot as p;

pub struct SelectedSoftwareOffer<'a> {
    pub node: u16,
    pub offer: &'a c::CapabilityOffer,
    pub configuration: &'a [c::ConfigurationEntry],
}

#[derive(Clone, Copy)]
pub struct SelectedSoftwareBounds {
    pub maximum_nodes: usize,
    pub maximum_offers: usize,
    pub maximum_image_bytes: usize,
    pub maximum_total_image_bytes: usize,
}

pub struct PreparedSoftwareOffer<'a> {
    pub node: u16,
    pub offer: &'a c::CapabilityOffer,
    pub configuration: &'a [c::ConfigurationEntry],
    pub image: Vec<u8>,
}

/// Validate and encode only the explicit software subset. Other nodes remain
/// the caller's separate realization responsibility. This creates no provider,
/// chooses no Back, invents no implementation identity and admits no resources.
pub fn prepare_selected_software<'a>(
    source: &'a p::ExpandedCanonicalPlot,
    selected: &[SelectedSoftwareOffer<'a>],
    bounds: SelectedSoftwareBounds,
) -> Result<Vec<PreparedSoftwareOffer<'a>>, String> {
    if source.gears.len() > bounds.maximum_nodes || selected.len() > bounds.maximum_offers {
        return Err("selected software count bound".into());
    }
    source
        .validate_expansion()
        .map_err(|error| format!("selected software expansion: {error:?}"))?;
    // Check the whole selection before generating any retained images.
    for (i, item) in selected.iter().enumerate() {
        if selected[..i].iter().any(|old| old.node == item.node) {
            return Err("duplicate selected software node".into());
        }
        let gear = source
            .gears
            .get(usize::from(item.node))
            .ok_or("selected software node range")?;
        let offer = item.offer;
        if offer.kind_id != gear.kind_id
            || offer.kind_contract_revision != gear.kind_contract_revision
            || offer.startup_parameters != gear.startup_parameters
            || offer.shorthand != gear.shorthand
            || offer.inputs != gear.inputs
            || offer.outputs != gear.outputs
            || offer.semantic_contract != gear.semantic_contract
            || item.configuration != gear.configuration
        {
            return Err("selected software differs from checked Source".into());
        }
        offer
            .validate_constructed_semantic_contract()
            .map_err(|error| format!("selected software contract: {error:?}"))?;
    }
    let mut total = 0usize;
    let mut result = Vec::with_capacity(selected.len());
    for item in selected {
        let image = crate::generate_capability_offer_image(item.offer, bounds.maximum_image_bytes)
            .map_err(|error| format!("selected software image: {error:?}"))?;
        total = total
            .checked_add(image.len())
            .ok_or("software image overflow")?;
        if total > bounds.maximum_total_image_bytes {
            return Err("selected software aggregate image bound".into());
        }
        result.push(PreparedSoftwareOffer {
            node: item.node,
            offer: item.offer,
            configuration: &source.gears[usize::from(item.node)].configuration,
            image,
        });
    }
    Ok(result)
}
