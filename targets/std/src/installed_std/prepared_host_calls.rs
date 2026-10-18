//! Prepared exact contract lookup and first-observation retention for std Play.
use super::{HOST_BINDING_SLOTS, HOST_CALLS_PER_NODE, MAX_NODES};
use conduit_kernel::{scheduler::HostCallRequest, HostCallId, NodeId};
use conduit_plan_lowering::lowering::LoweredHostCall;
use std::num::NonZeroU16;

pub(super) struct PreparedHostCalls<'a> {
    operations: &'a [LoweredHostCall],
    slots: [Option<NonZeroU16>; HOST_BINDING_SLOTS],
    observed: [bool; HOST_BINDING_SLOTS],
    requests: Vec<HostCallRequest>,
}
fn slot(node: NodeId, call: HostCallId) -> Option<usize> {
    if usize::from(node.0) >= MAX_NODES || call.0 >= HOST_CALLS_PER_NODE {
        return None;
    }
    Some(usize::from(node.0) * usize::from(HOST_CALLS_PER_NODE) + usize::from(call.0))
}
impl<'a> PreparedHostCalls<'a> {
    pub(super) fn prepare(
        operations: &'a [LoweredHostCall],
        capacity: usize,
    ) -> Result<Self, &'static str> {
        if operations.len() > HOST_BINDING_SLOTS {
            return Err("lowered Host Call identity table exceeds std profile");
        }
        let mut prepared = Self {
            operations,
            slots: [None; HOST_BINDING_SLOTS],
            observed: [false; HOST_BINDING_SLOTS],
            requests: Vec::with_capacity(capacity.max(operations.len())),
        };
        for (index, operation) in operations.iter().enumerate() {
            let key = slot(operation.node, operation.call)
                .ok_or("lowered Host Call identity is outside std profile")?;
            let encoded = u16::try_from(index + 1)
                .ok()
                .and_then(NonZeroU16::new)
                .ok_or("lowered Host Call identity index is not representable")?;
            // Preserve the original linear lookup's first-match behavior. The
            // ordinary kernel table admission already refuses duplicate binds.
            prepared.slots[key].get_or_insert(encoded);
        }
        Ok(prepared)
    }
    fn index(&self, node: NodeId, call: HostCallId) -> Option<usize> {
        self.slots[slot(node, call)?].map(|index| usize::from(index.get() - 1))
    }
    // Borrow the original lowered identity, independently of this cache's
    // mutable observation state. No contract or authority is reconstructed.
    pub(super) fn get(&self, node: NodeId, call: HostCallId) -> Option<&'a LoweredHostCall> {
        self.operations.get(self.index(node, call)?)
    }
    pub(super) fn record(&mut self, request: HostCallRequest) -> Result<(), &'static str> {
        let index = self
            .index(request.node, request.call)
            .ok_or("host request has no prepared contract identity")?;
        if !self.observed[index] {
            self.requests.push(request);
            self.observed[index] = true;
        }
        Ok(())
    }
    // These adapters historically retain every completion, including repeats.
    // Mark the key seen so a later first-only record has the old Vec semantics.
    pub(super) fn push(&mut self, request: HostCallRequest) -> Result<(), &'static str> {
        let index = self
            .index(request.node, request.call)
            .ok_or("host request has no prepared contract identity")?;
        self.requests.push(request);
        self.observed[index] = true;
        Ok(())
    }
    pub(super) fn into_requests(self) -> Vec<HostCallRequest> {
        self.requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{BoundedValueRef, HostCallBinding, RequestId, ValueRef};
    fn operation(node: u16, call: u16) -> LoweredHostCall {
        LoweredHostCall {
            node: NodeId(node),
            call: HostCallId(call),
            contract_id: format!("contract/{node}/{call}").into(),
            target_kind: Some(format!("kind/{node}/{call}").into()),
            maximum_in_flight: 1,
            binding: HostCallBinding {
                call: HostCallId(call),
                maximum_input_bytes: 0,
                maximum_output_bytes: u32::from(node) + 1,
            },
        }
    }
    fn request(node: NodeId, call: HostCallId, sequence: u32) -> HostCallRequest {
        HostCallRequest {
            node,
            call,
            request: RequestId(sequence),
            input: BoundedValueRef::new(
                ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: 0,
                },
                0,
            )
            .unwrap(),
        }
    }
    #[test]
    fn lookup_and_first_observations_match_linear_oracles_without_play_allocations() {
        let mut operations: Vec<_> = (0..MAX_NODES as u16)
            .flat_map(|node| (0..HOST_CALLS_PER_NODE).map(move |call| operation(node, call)))
            .collect();
        operations.reverse();
        let mut prepared = PreparedHostCalls::prepare(&operations, 4096).unwrap();
        let capacity = prepared.requests.capacity();
        let mut expected: Vec<HostCallRequest> = Vec::with_capacity(4096);
        let probe = crate::allocation_probe::begin();
        for sequence in 0..4096 {
            let key = (sequence as usize * 37) % HOST_BINDING_SLOTS;
            let node = NodeId((key / usize::from(HOST_CALLS_PER_NODE)) as u16);
            let call = HostCallId((key % usize::from(HOST_CALLS_PER_NODE)) as u16);
            let original = operations
                .iter()
                .find(|op| op.node == node && op.call == call)
                .unwrap();
            assert!(std::ptr::eq(prepared.get(node, call).unwrap(), original));
            let request = request(node, call, sequence);
            if sequence % 5 == 0 || !expected.iter().any(|r| r.node == node && r.call == call) {
                expected.push(request);
            }
            if sequence % 5 == 0 {
                prepared.push(request).unwrap();
            } else {
                prepared.record(request).unwrap();
            }
        }
        assert_eq!(probe.finish(), 0);
        assert_eq!(prepared.requests.capacity(), capacity);
        assert_eq!(prepared.into_requests(), expected);
    }
    #[test]
    fn absent_and_out_of_range_keys_cannot_alias_another_nodes_call() {
        let operations = [operation(1, 0)];
        let mut prepared = PreparedHostCalls::prepare(&operations, 0).unwrap();
        for (node, call) in [
            (0, 0),
            (0, HOST_CALLS_PER_NODE),
            (MAX_NODES as u16, 0),
            (u16::MAX, u16::MAX),
        ] {
            assert!(prepared.get(NodeId(node), HostCallId(call)).is_none());
            assert!(prepared
                .record(request(NodeId(node), HostCallId(call), 0))
                .is_err());
            assert!(prepared
                .push(request(NodeId(node), HostCallId(call), 0))
                .is_err());
        }
        assert!(prepared.into_requests().is_empty());
        assert!(PreparedHostCalls::prepare(&[operation(0, HOST_CALLS_PER_NODE)], 0).is_err());
        assert!(PreparedHostCalls::prepare(&[operation(MAX_NODES as u16, 0)], 0).is_err());
        let empty = PreparedHostCalls::prepare(&[], 0).unwrap();
        assert!(empty.get(NodeId(0), HostCallId(0)).is_none());
    }
    #[test]
    fn first_match_duplicates_and_finite_index_storage_remain_exact() {
        let operations = [operation(0, 0), operation(0, 0)];
        let mut prepared = PreparedHostCalls::prepare(&operations, 0).unwrap();
        assert!(std::ptr::eq(
            prepared.get(NodeId(0), HostCallId(0)).unwrap(),
            &operations[0]
        ));
        prepared
            .record(request(NodeId(0), HostCallId(0), 7))
            .unwrap();
        prepared
            .record(request(NodeId(0), HostCallId(0), 9))
            .unwrap();
        assert_eq!(prepared.into_requests()[0].request, RequestId(7));
        assert_eq!(std::mem::size_of::<Option<NonZeroU16>>(), 2);
        assert_eq!(
            std::mem::size_of::<PreparedHostCalls<'_>>(),
            HOST_BINDING_SLOTS * 3
                + std::mem::size_of::<&[LoweredHostCall]>()
                + std::mem::size_of::<Vec<HostCallRequest>>()
        );
        let too_many = vec![operation(0, 0); HOST_BINDING_SLOTS + 1];
        assert!(PreparedHostCalls::prepare(&too_many, 0).is_err());
    }
}
