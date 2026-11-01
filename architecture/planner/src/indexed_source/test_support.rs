//! Tests use the same immutable full descriptions through both entrances.
use super::*;
struct Software<'a>(&'a [conduit_core::CapabilityOffer]);
// SAFETY: immutable test fixtures remain borrowed for the whole synchronous call.
unsafe impl IndexedSoftwareOffers for Software<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn decode(&self, index: usize) -> Result<conduit_core::CapabilityOffer, PlannerError> {
        Ok(self.0[index].clone())
    }
}
pub(crate) fn selection_parity(
    gears: &[conduit_plot::CheckedGear],
    connections: &[conduit_plot::CheckedConnection],
    hosts: &[HostAdvertisement],
) {
    let mut natives = hosts.to_vec();
    for host in &mut natives {
        host.capabilities.clear();
    }
    let software = hosts
        .iter()
        .map(|host| Software(&host.capabilities))
        .collect::<Vec<_>>();
    let inputs = natives
        .iter()
        .zip(&software)
        .map(|(native, software)| IndexedHostOffers { native, software })
        .collect::<Vec<_>>();
    assert_eq!(
        crate::functional_compatibility::default_access_placements(
            &PlanningGears::borrowed(gears),
            connections,
            &crate::offer_access::OfferAccess::Indexed(&inputs)
        ),
        crate::functional_compatibility::default_planning_placements(
            &PlanningGears::borrowed(gears),
            connections,
            hosts
        )
    );
}
