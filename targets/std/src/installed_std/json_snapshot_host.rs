//! One selected JSON operation; exact lowered node precedes shared call IDs.
use crate::json_snapshot_file::{
    JsonSnapshotFileRefusal, PreparedJsonSnapshot, SelectedJsonSnapshotResidence,
};
use conduit_core::{AuthorityBinding, PlanFragment};
use conduit_kernel::{
    scheduler::HostCallRequest, BoundedValueRef, Failure, FailureCode, HostCallDisposition,
    HostCallId, HostCallOutcome, NodeId,
};
use conduit_plan_lowering::lowering::{KernelIdentityMap, LoweredHostCall};

pub(super) struct JsonSnapshotHost<'a> {
    node: NodeId,
    write: bool,
    authority: AuthorityBinding,
    provider: PreparedJsonSnapshot<'a>,
}
impl<'a> JsonSnapshotHost<'a> {
    pub(super) fn prepare(
        fragment: &PlanFragment,
        identity: &KernelIdentityMap,
        selected: Option<&'a SelectedJsonSnapshotResidence>,
    ) -> Result<Option<Self>, String> {
        let mut result = None;
        for placement in &fragment.placements {
            let write = match placement.implementation_id.as_str() {
                conduit_std_offers::JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION => true,
                conduit_std_offers::JSON_SNAPSHOT_READ_IMPLEMENTATION => false,
                _ => continue,
            };
            if result.is_some() {
                return Err("selected JSON snapshot admits one operation per Play".into());
            }
            let selected =
                selected.ok_or("JSON snapshot has no independently selected residence")?;
            let provider = PreparedJsonSnapshot::prepare(selected, placement)
                .map_err(|error| format!("prepare selected JSON snapshot: {error:?}"))?;
            let node = identity
                .node_for_placement(&placement.placement_id)
                .ok_or("JSON snapshot placement was not lowered")?;
            result = Some(Self {
                node,
                write,
                authority: placement.authority[0].clone(),
                provider,
            });
        }
        if selected.is_some() && result.is_none() {
            return Err("selected JSON residence has no exact operation in this fragment".into());
        }
        Ok(result)
    }
    pub(super) fn contains_node(&self, node: NodeId) -> bool {
        self.node == node
    }
    pub(super) fn execute(
        &mut self,
        request: HostCallRequest,
        input: &[u8],
    ) -> Result<&[u8], JsonSnapshotFileRefusal> {
        if request.node != self.node || request.call != HostCallId(0) {
            return Err(JsonSnapshotFileRefusal::InvalidBinding);
        }
        if self.write {
            self.provider.publish(&self.authority, input)
        } else {
            self.provider.read(&self.authority, input)
        }
    }
}
pub(super) fn complete(
    scheduler: &mut super::InstalledScheduler,
    operation: &LoweredHostCall,
    request: HostCallRequest,
    result: Result<&[u8], JsonSnapshotFileRefusal>,
) -> Result<(), String> {
    let outcome = match result {
        Ok(bytes) => {
            let value = scheduler
                .store_host_value(bytes)
                .map_err(|error| format!("store JSON snapshot output: {error:?}"))?;
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(value, operation.binding.maximum_output_bytes)
                        .map_err(|error| format!("bound JSON snapshot output: {error:?}"))?,
                ),
                failure: None,
            }
        }
        Err(error) => {
            let code = match &error {
                JsonSnapshotFileRefusal::WrongReference
                | JsonSnapshotFileRefusal::InvalidJson(_) => FailureCode::InvalidInput,
                JsonSnapshotFileRefusal::InvalidBinding
                | JsonSnapshotFileRefusal::File(
                    conduit_linux_snapshot_store::FileSnapshotRefusal::Binding(_),
                ) => FailureCode::HostCallDenied,
                _ => FailureCode::HostCallFailed,
            };
            HostCallOutcome {
                disposition: if code == FailureCode::HostCallDenied {
                    HostCallDisposition::Denied
                } else {
                    HostCallDisposition::Failed
                },
                output: None,
                failure: Some(Failure {
                    code,
                    detail: error.detail(),
                }),
            }
        }
    };
    scheduler
        .complete_host_call(request.node, request.request, outcome)
        .map_err(|error| format!("complete JSON snapshot operation: {error:?}"))
}
