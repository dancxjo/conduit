//! Distinct initialized resident owners for repeated checked programs.
use conduit_core as c;
use conduit_embedded_build::PreparedResidentExpression;
use conduit_planner as p;

pub(super) fn select(
    plot: &conduit_plot::ExpandedCanonicalPlot,
    host: &c::HostAdvertisement,
    residents: &[&PreparedResidentExpression],
    used: &mut std::collections::BTreeSet<c::CapabilityId>,
) -> Result<p::PlacementChoices, String> {
    let mut choices = p::default_expanded_placements(plot, std::slice::from_ref(host))
        .map_err(|e| format!("resident placement: {e:?}"))?;
    for gear in &plot.gears {
        if !residents
            .iter()
            .any(|image| image.kind_id() == &gear.kind_id)
        {
            continue;
        }
        let image = residents
            .iter()
            .find(|image| {
                image.kind_id() == &gear.kind_id
                    && image.offer_description().semantic_contract == gear.semantic_contract
                    && !used.contains(&image.offer_description().capability_id)
            })
            .ok_or("distinct initialized resident owner absent")?;
        let offer = image.offer_description();
        if !host.capabilities.iter().any(|actual| actual == offer) {
            return Err("selected resident description absent from calibration Host".into());
        }
        choices
            .by_gear
            .get_mut(&gear.gear_id)
            .ok_or("resident placement absent")?
            .capability_id = offer.capability_id.clone();
        used.insert(offer.capability_id.clone());
    }
    Ok(choices)
}
