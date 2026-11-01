//! The same stable Fore ordering for local and multi-fragment attachment.
use conduit_core::PlannedForePort;

// One concrete closure type keeps the two attachment paths from instantiating
// duplicate stable-sort families. Equal keys retain their original order.
#[inline(never)]
pub(super) fn sort_fore_ports(ports: &mut [PlannedForePort]) {
    ports.sort_by(|left, right| {
        (left.direction as u8, &left.front_port_id, left.track).cmp(&(
            right.direction as u8,
            &right.front_port_id,
            right.track,
        ))
    });
}
