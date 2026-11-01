//! Exact partition-local witnesses for native work in the shared Body kernel.
use super::*;
use conduit_core::{CapabilityId, Kind, PlacementId};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};
use std::sync::Arc;

struct Entry {
    partition: usize,
    global: conduit_kernel::NodeId,
    placement: PlacementId,
    input: u32,
    output: u32,
    descriptor: Option<crate::body_execution::native::LocalNativeDescriptor>,
}
pub(crate) struct NativeRoute {
    fragments: Vec<Arc<PlanFragment>>,
    lowered: Vec<Arc<LoweredPlanFragment>>,
    entries: Vec<Entry>,
    active: Vec<conduit_core::ActivePlayIdentity>,
}
impl NativeRoute {
    pub(super) fn prepare(
        fragments: &[&PlanFragment],
        combined: &[LoweredPlanFragment],
        calls: &[(CapabilityId, Kind)],
    ) -> Result<Self, String> {
        let mut entries = Vec::new();
        for (partition, (fragment, lowered)) in fragments.iter().zip(combined).enumerate() {
            for gear in &fragment.placements {
                if !calls.iter().any(|(capability, kind)| {
                    &gear.capability_id == capability
                        && gear.kind_id == kind.kind_id
                        && gear.kind_contract_revision == kind.kind_contract_revision
                        && gear.inputs == kind.inputs
                        && gear.outputs == kind.outputs
                        && gear.semantic_contract == kind.semantic_contract()
                }) {
                    continue;
                }
                if gear.inputs.len() != 1 || gear.outputs.len() != 1 {
                    return Err("native local Back requires one input and one output".into());
                }
                let [call] = gear.host_calls.as_slice() else {
                    return Err("native selected Back requires one call".into());
                };
                if call.maximum_in_flight != 1 {
                    return Err("native local Back requires one bounded request".into());
                }
                let global = lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or("native placement has no exact lowered node")?;
                entries.push(Entry {
                    partition,
                    global,
                    placement: gear.placement_id.clone(),
                    input: call.maximum_input_bytes,
                    output: call.maximum_output_bytes,
                    descriptor: None,
                });
            }
        }
        if entries.is_empty() {
            return Ok(Self {
                fragments: Vec::new(),
                lowered: Vec::new(),
                entries,
                active: Vec::new(),
            });
        }
        let lowered = fragments
            .iter()
            .map(|f| {
                lower_plan_fragment(f)
                    .map(Arc::new)
                    .map_err(|e| format!("native local lowering: {e:?}"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            fragments: fragments.iter().map(|f| Arc::new((*f).clone())).collect(),
            lowered,
            entries,
            active: Vec::new(),
        })
    }
    pub(super) fn install(
        &mut self,
        inventory: &conduit_core::HostAdvertisement,
        sequence: u64,
    ) -> Result<(), String> {
        if !self.active.is_empty() {
            return Err("native slots already prepared".into());
        }
        self.active = self
            .fragments
            .iter()
            .map(|fragment| {
                conduit_core::bind_active_play(
                    &fragment.plan_id,
                    &fragment.host_id,
                    &fragment.boot_id,
                    sequence,
                )
            })
            .collect();
        // One immutable inventory for the Host and one canonical witness per
        // partition; each request slot owns only its bounded mutable input.
        let inventory = Arc::new(inventory.clone());
        for entry in &mut self.entries {
            let mut input = Vec::new();
            input
                .try_reserve_exact(entry.input as usize)
                .map_err(|_| "native input preparation exhausted memory")?;
            entry.descriptor = Some(crate::body_execution::native::LocalNativeDescriptor {
                inventory: inventory.clone(),
                fragment: self.fragments[entry.partition].clone(),
                lowered: self.lowered[entry.partition].clone(),
                active: self.active[entry.partition].clone(),
                placement: entry.placement.clone(),
                input,
                maximum_output_bytes: entry.output,
            });
        }
        Ok(())
    }
    pub(super) fn is_prepared(&self) -> bool {
        self.entries.iter().all(|entry| entry.descriptor.is_some())
    }
    pub(super) fn contains(&self, node: conduit_kernel::NodeId) -> bool {
        self.entries.iter().any(|entry| entry.global == node)
    }
    pub(super) fn matches(&self, partition: usize, placement: &PlacementId) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.partition == partition && &entry.placement == placement)
    }
    pub(super) fn input_bound(&self, node: conduit_kernel::NodeId) -> Result<u32, String> {
        self.entries
            .iter()
            .find(|entry| entry.global == node)
            .map(|entry| entry.input)
            .ok_or("native node absent".into())
    }
    pub(super) fn execute(
        &mut self,
        execution: &mut crate::body_execution::native::NativeExecution<'_>,
        request: HostCallRequest,
        input: &[u8],
    ) -> Result<Result<Vec<u8>, String>, String> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.global == request.node)
            .ok_or("native request has no retained placement")?;
        if input.len() > entry.input as usize {
            return Err("native input exceeds selected bound".into());
        }
        let fragment = &self.fragments[entry.partition];
        let lowered = &self.lowered[entry.partition];
        let node = lowered
            .identity
            .node_for_placement(&entry.placement)
            .ok_or("native local node absent")?;
        let call = lowered
            .host_calls
            .iter()
            .find(|call| call.node == node)
            .ok_or("native local call absent")?;
        let local_request = HostCallRequest {
            node,
            call: call.call,
            ..request
        };
        let descriptor = entry
            .descriptor
            .take()
            .ok_or("native work slot not prepared or already borrowed")?;
        if input.len() > descriptor.input.capacity() {
            return Err("native prepared input capacity differs".into());
        }
        let (descriptor, result) = execution.execute(
            entry.partition,
            fragment,
            &self.active[entry.partition],
            &entry.placement,
            local_request,
            input,
            entry.output,
            descriptor,
        )?;
        entry.descriptor = Some(descriptor);
        Ok(result)
    }
}
pub(super) fn budget(
    gear: &conduit_core::PlannedGear,
) -> Result<super::super::factory::BackBudget, String> {
    let [call] = gear.host_calls.as_slice() else {
        return Err("native Back needs one call".into());
    };
    Ok(super::super::factory::BackBudget {
        value_items: 2,
        value_bytes: call
            .maximum_input_bytes
            .checked_add(call.maximum_output_bytes)
            .ok_or("native budget overflow")?,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: call.maximum_input_bytes.max(call.maximum_output_bytes),
    })
}
