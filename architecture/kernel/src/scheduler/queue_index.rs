//! Physical queue addresses for validated, bounded cord rings.
use super::CordSpec;

pub(super) fn queue_slot(spec: CordSpec, head: u16, offset: u16) -> usize {
    debug_assert!(head < spec.item_capacity);
    debug_assert!(offset < spec.item_capacity);
    let until_wrap = spec.item_capacity - head;
    let position = if offset < until_wrap {
        head + offset
    } else {
        offset - until_wrap
    };
    // Plan admission checks the arena end in usize; never narrow its address.
    usize::from(spec.slot_start) + usize::from(position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::CordCapacity;
    use crate::{CordId, NodeId, PortId, RemoteEndpointId};

    #[test]
    fn bounded_ring_positions_match_wide_arithmetic() {
        for capacity in 1..=32u16 {
            let spec = CordSpec::remote_ingress(
                CordId(0),
                RemoteEndpointId(0),
                (NodeId(0), PortId(0)),
                CordCapacity {
                    slot_start: 7,
                    item_capacity: capacity,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            );
            for head in 0..capacity {
                for offset in 0..capacity {
                    let expected = 7 + (u32::from(head) + u32::from(offset)) % u32::from(capacity);
                    assert_eq!(
                        queue_slot(spec, head, offset),
                        usize::try_from(expected).unwrap()
                    );
                }
            }
        }
    }
}
