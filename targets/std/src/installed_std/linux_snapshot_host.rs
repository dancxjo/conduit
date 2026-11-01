//! Actual prepared file effects for original requests from the installed kernel.
#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
use super::InstalledScheduler;
#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
use conduit_core::PlanFragment;
#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
use conduit_kernel::{self as k, scheduler::HostCallRequest, ValueStorage};
#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
use conduit_linux_snapshot_store::{PreparedFileSnapshotOperation, SnapshotDirectory};
#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
use conduit_plan_lowering::lowering::LoweredPlanFragment;
#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
pub(super) struct PreparedSnapshots<'a> {
    files: Vec<(k::NodeId, PreparedFileSnapshotOperation<'a>)>,
}
#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
impl<'a> PreparedSnapshots<'a> {
    pub(super) fn prepare(
        directory: Option<&'a SnapshotDirectory>,
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, String> {
        let selected: Vec<_> = fragment
            .placements
            .iter()
            .filter(|placement| {
                matches!(
                    placement.implementation_id.as_str(),
                    crate::linux_snapshot::PUBLISH | crate::linux_snapshot::READ
                )
            })
            .collect();
        if selected.len() > super::MAX_NODES {
            return Err("snapshot executor capacity".into());
        }
        let mut files = Vec::with_capacity(selected.len());
        for placement in selected {
            let node = lowered
                .nodes
                .iter()
                .find(|node| node.placement_id == placement.placement_id)
                .ok_or("snapshot placement has no lowered node")?
                .node;
            let directory =
                directory.ok_or("snapshot operation has no admitted directory custody")?;
            files.push((
                node,
                PreparedFileSnapshotOperation::prepare(directory, placement, node)
                    .map_err(|error| format!("prepare snapshot file effect: {error:?}"))?,
            ));
        }
        Ok(Self { files })
    }
    /// Complete this original request, including on refusal. A refused external
    /// effect aborts the run with its actual uncertainty/candidate information;
    /// no success reference can reach downstream State.
    pub(super) fn perform(
        &mut self,
        kernel: &mut InstalledScheduler,
        request: HostCallRequest,
    ) -> Result<(), String> {
        let (_, files) = self
            .files
            .iter_mut()
            .find(|(node, _)| *node == request.node)
            .ok_or("snapshot request has no selected executor")?;
        perform_snapshot(kernel, request, files)
    }
}
#[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
pub(super) fn perform_snapshot(
    kernel: &mut InstalledScheduler,
    request: HostCallRequest,
    files: &mut PreparedFileSnapshotOperation<'_>,
) -> Result<(), String> {
    let input = kernel
        .values()
        .get(request.input.value)
        .map_err(|error| format!("snapshot input: {error:?}"))?;
    let mut refusal = None;
    let failed = |failure| k::HostCallOutcome {
        disposition: k::HostCallDisposition::Failed,
        output: None,
        failure: Some(failure),
    };
    let outcome = match files.perform(request, input) {
        Ok(bytes) => match kernel.store_host_value(bytes) {
            Ok(value) => k::HostCallOutcome {
                disposition: k::HostCallDisposition::Completed,
                output: Some(
                    k::BoundedValueRef::new(value, files.output_bound())
                        .map_err(|error| format!("snapshot output bound: {error:?}"))?,
                ),
                failure: None,
            },
            Err(error) => {
                refusal = Some(format!(
                    "snapshot effect completed; output custody refused: {error:?}"
                ));
                failed(k::Failure {
                    code: k::FailureCode::HostCallFailed,
                    detail: 217,
                })
            }
        },
        Err(error) => {
            let failure = match &error {
                conduit_linux_snapshot_store::FileOperationRefusal::Operation(failure) => *failure,
                _ => k::Failure {
                    code: k::FailureCode::HostCallFailed,
                    detail: 216,
                },
            };
            refusal = Some(format!("snapshot effect refused: {error:?}"));
            failed(failure)
        }
    };
    let output = outcome.output.map(|value| value.value);
    let completed = kernel.complete_host_call(request.node, request.request, outcome);
    let finished = files.finish(request);
    if completed.is_err() {
        if let Some(value) = output {
            kernel
                .discard_host_value(value)
                .map_err(|error| format!("discard snapshot output: {error:?}"))?;
        }
    }
    completed.map_err(|error| format!("complete snapshot request: {error:?}"))?;
    if let Some(error) = refusal {
        return Err(format!("{error}; correlation retirement: {finished:?}"));
    }
    finished.map_err(|error| format!("retire snapshot effect: {error:?}"))
}
pub(super) fn is_operation(contract: &str) -> bool {
    #[cfg(all(feature = "linux-snapshot", target_os = "linux"))]
    {
        matches!(
            contract,
            conduit_linux_snapshot_store::PUBLISH_OPERATION
                | conduit_linux_snapshot_store::READ_OPERATION
        )
    }
    #[cfg(not(all(feature = "linux-snapshot", target_os = "linux")))]
    {
        let _ = contract;
        false
    }
}
