//! Root-held finite address reservation, separate from a device-scoped claim.
use super::dwc2_control::RootControlRoute;

/// Move-only native permission. Wire addresses or copied identity metadata
/// cannot create this token. The native control claim must independently cover
/// the same entire controller bus; a device-scoped claim is insufficient.
pub(crate) struct BusControlReservation {
    hub: u8,
    first_child: u8,
    ports: u8,
}
impl BusControlReservation {
    /// # Safety
    /// The trusted Root exclusively owns the original physical controller and
    /// its electrical/default-address namespace. It has reserved this hub and
    /// child-address range under the retained bus-wide native control scope.
    /// No other host/driver may enumerate or route transactions on this bus.
    /// Source recognition and planning metadata do not establish these facts.
    pub(crate) unsafe fn admit_owned_bus(hub: u8, first_child: u8, ports: u8) -> Option<Self> {
        let last = first_child.checked_add(ports.checked_sub(1)?)?;
        if hub == 0
            || hub > 127
            || first_child == 0
            || last > 127
            || ports > 7
            || (first_child..=last).contains(&hub)
        {
            return None;
        }
        Some(Self {
            hub,
            first_child,
            ports,
        })
    }
    pub(super) fn permits(&self, route: RootControlRoute) -> bool {
        let child = (self.first_child..self.first_child + self.ports).contains(&route.address);
        let address = route.address == 0 || route.address == self.hub || child;
        address
            && route.translator.is_none_or(|tt| {
                tt.hub_address == self.hub
                    && tt.port > 0
                    && tt.port <= self.ports
                    && route.address != self.hub
            })
    }
}
