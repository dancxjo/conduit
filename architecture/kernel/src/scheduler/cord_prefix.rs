//! Preparation-only rejection of overlapping queues and duplicate remote IDs.
use super::{CordSpec, RemoteEndpointId, SchedulerError};

#[derive(Default)]
pub(super) struct CordPrefix {
    maximum_queue_end: usize,
    maximum_remote: Option<RemoteEndpointId>,
    #[cfg(test)]
    pair_probes: usize,
}
impl CordPrefix {
    pub(super) fn admit(
        &mut self,
        cord: CordSpec,
        previous: &[CordSpec],
        end: usize,
    ) -> Result<(), SchedulerError> {
        let endpoint = cord.remote_endpoint();
        let queue_disjoint = usize::from(cord.slot_start) >= self.maximum_queue_end;
        let endpoint_unique = match (endpoint, self.maximum_remote) {
            (Some(endpoint), Some(previous)) => endpoint.0 > previous.0,
            _ => true,
        };
        if !queue_disjoint || !endpoint_unique {
            for other in previous {
                #[cfg(test)]
                {
                    self.pair_probes += 1;
                }
                if !endpoint_unique
                    && endpoint
                        .zip(other.remote_endpoint())
                        .is_some_and(|(left, right)| left == right)
                {
                    return Err(SchedulerError::InvalidPlan);
                }
                let other_end = usize::from(other.slot_start) + usize::from(other.item_capacity);
                if !queue_disjoint
                    && usize::from(cord.slot_start) < other_end
                    && usize::from(other.slot_start) < end
                {
                    return Err(SchedulerError::InvalidPlan);
                }
            }
        }
        self.maximum_queue_end = self.maximum_queue_end.max(end);
        if let Some(endpoint) = cord.remote_endpoint() {
            self.maximum_remote = Some(self.maximum_remote.map_or(endpoint, |previous| {
                if previous.0 > endpoint.0 {
                    previous
                } else {
                    endpoint
                }
            }));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::scheduler::CordCapacity;
    use crate::{CordId, NodeId, PortId};

    fn cord(index: u16, start: u16, capacity: u16, endpoint: u16) -> CordSpec {
        CordSpec::remote_ingress(
            CordId(index),
            RemoteEndpointId(endpoint),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: start,
                item_capacity: capacity,
                byte_capacity: 1,
                pressure_policy: Default::default(),
            },
        )
    }
    fn end(cord: CordSpec) -> usize {
        usize::from(cord.slot_start) + usize::from(cord.item_capacity)
    }

    #[test]
    fn ordered_ten_thousand_cords_need_no_pairwise_prefix_probes() {
        let mut previous = std::vec::Vec::with_capacity(10_000);
        let mut prefix = CordPrefix::default();
        for index in 0..10_000 {
            let cord = cord(index, index, 1, index);
            prefix.admit(cord, &previous, end(cord)).unwrap();
            previous.push(cord);
        }
        assert_eq!(prefix.pair_probes, 0);
    }

    #[test]
    fn prefix_acceptance_matches_pairwise_oracle_for_unordered_and_invalid_layouts() {
        let cases = [
            cord(0, 0, 2, 9),
            cord(1, 2, 1, 3),
            cord(2, 4, 1, 7),
            cord(3, 1, 2, 8),
            cord(4, 8, 1, 9),
            cord(5, 65_000, 600, u16::MAX),
            CordSpec::local(
                CordId(6),
                (NodeId(0), PortId(0)),
                (NodeId(1), PortId(0)),
                CordCapacity {
                    slot_start: 10,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            ),
            CordSpec::remote_egress(
                CordId(7),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(3),
                CordCapacity {
                    slot_start: 11,
                    item_capacity: 1,
                    byte_capacity: 1,
                    pressure_policy: Default::default(),
                },
            ),
        ];
        for a in cases {
            for b in cases {
                for c in cases {
                    let mut prefix = CordPrefix::default();
                    let sequence = [a, b, c];
                    for (index, current) in sequence.iter().copied().enumerate() {
                        let previous = &sequence[..index];
                        let conflict = previous.iter().any(|other| {
                            current
                                .remote_endpoint()
                                .zip(other.remote_endpoint())
                                .is_some_and(|(left, right)| left == right)
                                || usize::from(current.slot_start) < end(*other)
                                    && usize::from(other.slot_start) < end(current)
                        });
                        assert_eq!(
                            prefix.admit(current, previous, end(current)),
                            if conflict {
                                Err(SchedulerError::InvalidPlan)
                            } else {
                                Ok(())
                            }
                        );
                        if conflict {
                            break;
                        }
                    }
                }
            }
        }
    }
}
