//! Conservative uniform queue preference for these small activation fixtures.
//! This is not a general per-Cord planner. Parent and child selected Backs
//! retain their original ceilings; full expected outputs prove sufficiency.
pub fn activation_capacity(
    hosts: &[conduit_core::HostAdvertisement],
    plots: &[&conduit_plot::ExpandedCanonicalPlot],
) -> u32 {
    let mut capacity = None;
    for plot in plots {
        let placements = conduit_planner::default_expanded_placements(plot, hosts).unwrap();
        for choice in placements.by_gear.values() {
            let host = hosts
                .iter()
                .find(|host| host.host_id == choice.host_id)
                .unwrap();
            let offer = host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == choice.capability_id)
                .unwrap();
            capacity = Some(
                capacity.map_or(offer.limits.max_queue_bytes, |current: u32| {
                    current.min(offer.limits.max_queue_bytes)
                }),
            );
        }
    }
    let capacity = capacity.expect("fixture selects finite providers");
    assert!(capacity > 0);
    capacity
}
