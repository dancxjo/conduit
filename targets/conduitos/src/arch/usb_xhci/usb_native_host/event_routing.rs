//! Exact routing metadata for two independently live bulk reservations.
//! The original transfer validates again before any DMA or authority retirement.
use super::UsbNativeCallIndex;
use crate::arch::xhci_mechanism::Event;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PendingEventKey {
    pub index: UsbNativeCallIndex,
    pub slot: u8,
    pub endpoint: u8,
    pub pointer: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RoutingRefusal {
    Foreign,
    Ambiguous,
}

/// Foreign or duplicate keys cannot select a lease. Completion code and extent
/// remain the selected transfer's responsibility; a stall is routed by identity.
pub(super) fn route_event(
    event: Event,
    pending: [Option<PendingEventKey>; 2],
) -> Result<UsbNativeCallIndex, RoutingRefusal> {
    if event.event_data || event.event_type != 32 {
        return Err(RoutingRefusal::Foreign);
    }
    let mut selected = None;
    for key in pending.into_iter().flatten() {
        if (event.slot, event.endpoint, event.pointer) == (key.slot, key.endpoint, key.pointer) {
            if selected.is_some() {
                return Err(RoutingRefusal::Ambiguous);
            }
            selected = Some(key.index);
        }
    }
    selected.ok_or(RoutingRefusal::Foreign)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> [Option<PendingEventKey>; 2] {
        [
            Some(PendingEventKey {
                index: UsbNativeCallIndex(1),
                slot: 2,
                endpoint: 3,
                pointer: 0x1010,
            }),
            Some(PendingEventKey {
                index: UsbNativeCallIndex(2),
                slot: 2,
                endpoint: 4,
                pointer: 0x2020,
            }),
        ]
    }
    fn event() -> Event {
        Event {
            event_data: false,
            event_type: 32,
            completion_code: 1,
            slot: 2,
            endpoint: 4,
            residual: 0,
            pointer: 0x2020,
        }
    }

    #[test]
    fn either_direction_can_complete_first_without_consuming_its_sibling() {
        let pending = keys();
        assert_eq!(route_event(event(), pending), Ok(UsbNativeCallIndex(2)));
        let incoming = Event {
            endpoint: 3,
            pointer: 0x1010,
            residual: 1500,
            ..event()
        };
        assert_eq!(route_event(incoming, pending), Ok(UsbNativeCallIndex(1)));
        assert_eq!(
            route_event(incoming, [pending[0], None]),
            Ok(UsbNativeCallIndex(1))
        );
        assert_eq!(
            route_event(event(), [pending[0], None]),
            Err(RoutingRefusal::Foreign)
        );
    }

    #[test]
    fn foreign_and_late_events_never_select_a_live_lease() {
        for foreign in [
            Event { slot: 3, ..event() },
            Event {
                endpoint: 3,
                ..event()
            },
            Event {
                pointer: 0x2010,
                ..event()
            },
            Event {
                event_data: false,
                event_type: 33,
                ..event()
            },
        ] {
            assert_eq!(route_event(foreign, keys()), Err(RoutingRefusal::Foreign));
        }
        assert_eq!(
            route_event(event(), [None, None]),
            Err(RoutingRefusal::Foreign)
        );
        let key = keys()[1];
        assert_eq!(
            route_event(event(), [key, key]),
            Err(RoutingRefusal::Ambiguous)
        );
    }

    #[test]
    fn stall_identity_routes_but_does_not_grant_retirement() {
        assert_eq!(
            route_event(
                Event {
                    completion_code: 6,
                    ..event()
                },
                keys()
            ),
            Ok(UsbNativeCallIndex(2))
        );
    }
}

#[cfg(test)]
#[test]
fn event_data_cookie_cannot_route_as_a_pointer_lease() {
    let key = PendingEventKey {
        index: UsbNativeCallIndex(1),
        slot: 2,
        endpoint: 3,
        pointer: 0x1010,
    };
    let event = Event {
        event_data: true,
        event_type: 32,
        completion_code: 1,
        slot: 2,
        endpoint: 3,
        residual: 0,
        pointer: 0x1010,
    };
    assert_eq!(
        route_event(event, [Some(key), None]),
        Err(RoutingRefusal::Foreign)
    );
    assert_eq!(
        route_event(
            Event {
                event_data: false,
                ..event
            },
            [Some(key), None]
        ),
        Ok(key.index)
    );
}
