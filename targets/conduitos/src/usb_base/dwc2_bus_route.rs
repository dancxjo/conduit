//! Root-held finite address reservation, separate from a device-scoped claim.
use super::dwc2_control::RootControlRoute;

/// Move-only native permission. Wire addresses or copied identity metadata
/// cannot create this token. The native control claim must independently cover
/// the same entire controller bus; a device-scoped claim is insufficient.
pub(crate) struct BusControlReservation {
    hub: u8,
    first_child: u8,
    ports: u8,
    addresses: u8,
    child_translators: bool,
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
            addresses: ports,
            child_translators: false,
        })
    }
    /// # Safety
    /// Root exclusively owns the entire controller bus and reserves fourteen
    /// child addresses for two seven-port high-speed hubs. This grants no
    /// independent attachment or endpoint possession to a protocol value.
    pub(crate) unsafe fn admit_owned_hub_tree(hub: u8, first_child: u8) -> Option<Self> {
        let mut reservation = unsafe { Self::admit_owned_bus(hub, first_child, 7) }?;
        let last = first_child.checked_add(13)?;
        if last > 127 || (first_child..=last).contains(&hub) {
            return None;
        }
        reservation.addresses = 14;
        reservation.child_translators = true;
        Some(reservation)
    }
    pub(super) fn permits(&self, route: RootControlRoute) -> bool {
        let child = (self.first_child..self.first_child + self.addresses).contains(&route.address);
        let address = route.address == 0 || route.address == self.hub || child;
        address
            && route.translator.is_none_or(|tt| {
                (tt.hub_address == self.hub
                    || (self.child_translators
                        && (self.first_child..self.first_child + self.addresses)
                            .contains(&tt.hub_address)))
                    && tt.port > 0
                    && tt.port <= self.ports
                    && route.address != tt.hub_address
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::dwc2::TransactionTranslator;
    fn route(address: u8, hub: u8, port: u8) -> RootControlRoute {
        RootControlRoute {
            slot: 1,
            generation: 1,
            address,
            maximum_packet: 64,
            translator: (hub != 0).then_some(TransactionTranslator {
                hub_address: hub,
                port,
                low_speed: false,
            }),
        }
    }
    #[test]
    fn nested_hub_routes_require_the_larger_original_root_reservation() {
        let legacy = unsafe { BusControlReservation::admit_owned_bus(1, 2, 7) }.unwrap();
        let tree = unsafe { BusControlReservation::admit_owned_hub_tree(1, 2) }.unwrap();
        assert!(!legacy.permits(route(10, 2, 2)));
        assert!(tree.permits(route(10, 2, 2)));
        assert!(tree.permits(route(15, 0, 0)));
        for candidate in [
            route(16, 0, 0),
            route(10, 16, 2),
            route(10, 2, 8),
            route(2, 2, 2),
        ] {
            assert!(!tree.permits(candidate));
        }
        assert!(unsafe { BusControlReservation::admit_owned_hub_tree(1, 115) }.is_none());
    }
}
