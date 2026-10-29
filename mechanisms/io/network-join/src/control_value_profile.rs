//! Three physical values cover the seed and both maximum-sized results even
//! before a consumed Host input retires. This bound requires the exact one-shot
//! source → join → sink chain; it is not a general three-node graph budget.
use super::{ControlKernelRefusal, PreparedNetworkKernelParts, CONTROL_VALUE_BYTES};
use conduit_kernel::{self as k, scheduler::CordSpec};

pub(super) fn check(parts: &PreparedNetworkKernelParts) -> Result<(), ControlKernelRefusal> {
    let roles = parts.roles;
    for (source, sink) in [
        (roles.credentials, roles.join),
        (roles.join, roles.attachment_sign),
    ] {
        let mut matching = parts.cords.iter().filter(|cord| {
            cord.source_local() == Some((source, k::PortId(0)))
                && cord.sink_local() == Some((sink, k::PortId(0)))
        });
        let cord = matching.next().ok_or(ControlKernelRefusal::Shape)?;
        if matching.next().is_some() || !bounded(cord) {
            return Err(ControlKernelRefusal::Shape);
        }
        if parts.nodes[usize::from(sink.0)].input_cords != [Some(cord.cord)] {
            return Err(ControlKernelRefusal::Shape);
        }
    }
    if parts.nodes[usize::from(roles.credentials.0)].input_cords != [None] {
        return Err(ControlKernelRefusal::Shape);
    }
    Ok(())
}
fn bounded(cord: &CordSpec) -> bool {
    cord.item_capacity == 1
        && cord.byte_capacity <= CONTROL_VALUE_BYTES as u32
        && cord.maximum_value_bytes <= CONTROL_VALUE_BYTES as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parts() -> PreparedNetworkKernelParts {
        use k::scheduler::{CordCapacity, NodeSpec};
        PreparedNetworkKernelParts {
            nodes: core::array::from_fn(|i| NodeSpec {
                input_cords: [if i == 0 {
                    None
                } else {
                    Some(k::CordId(i as u16 - 1))
                }],
                maximum_step_fuel: 4,
            }),
            cords: core::array::from_fn(|i| {
                CordSpec::local(
                    k::CordId(i as u16),
                    (k::NodeId(i as u16), k::PortId(0)),
                    (k::NodeId(i as u16 + 1), k::PortId(0)),
                    CordCapacity {
                        slot_start: i as u16,
                        item_capacity: 1,
                        byte_capacity: 167,
                        pressure_policy: Default::default(),
                    },
                )
            }),
            routes: k::FixedRoutes::new(1),
            bindings: k::FixedHostCallBindings::new(1),
            roles: super::super::NetworkControlRoles {
                credentials: k::NodeId(0),
                join: k::NodeId(1),
                attachment_sign: k::NodeId(2),
            },
        }
    }
    #[test]
    fn exact_chain_accepts_two_maximum_cords_but_rejects_other_liveness() {
        let mut p = parts();
        assert_eq!(check(&p), Ok(()));
        for index in 0..2 {
            p.cords[index].item_capacity = 2;
            assert_eq!(check(&p), Err(ControlKernelRefusal::Shape));
            p = parts();
            p.cords[index].maximum_value_bytes = 168;
            assert_eq!(check(&p), Err(ControlKernelRefusal::Shape));
            p = parts();
            p.cords[index].byte_capacity = 168;
            assert_eq!(check(&p), Err(ControlKernelRefusal::Shape));
            p = parts();
        }
        for sink in [k::NodeId(0), k::NodeId(1)] {
            p.cords[1].sink = k::CordEndpoint::local(sink, k::PortId(0));
            assert_eq!(check(&p), Err(ControlKernelRefusal::Shape));
            p = parts();
        }
        p.cords[0].sink = k::CordEndpoint::local(k::NodeId(2), k::PortId(0));
        assert_eq!(check(&p), Err(ControlKernelRefusal::Shape));
        p = parts();
        p.nodes[0].input_cords = [Some(k::CordId(1))];
        assert_eq!(check(&p), Err(ControlKernelRefusal::Shape));
    }
}
