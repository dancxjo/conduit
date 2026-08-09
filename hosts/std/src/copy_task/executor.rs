use super::base::{CopyFiles, ExecutionFaults};
use super::model::{CopyRequestId, CopyResult, CopyRunReceipt, CopyStopToken};
use super::operation::CopyOperation;
use super::registry::{ProtectedFileAvailability, ProtectedFileEntry, ProtectedFileRegistry};
use crate::StdHost;
use conduit_core::{
    bind_active_play, PlanFragment, ProtectedResourceAccess, ProtectedResourceBinding,
    ProtectedResourceCommitPolicy,
};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    ClueSink, CordEndpoint, CordId, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationOutcome, HostedClueLog, HostedValueStore, NodeId,
    PortId, ValueStorage,
};
use conduit_runtime::lowering::{lower_plan_fragment, MAXIMUM_KERNEL_PORTS_PER_NODE};

const MAX_COPY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CLUE_ITEMS: u16 = 20_000;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;

type CopyScheduler = FixedScheduler<
    OperationDriver<CopyOperation, PORTS>,
    HostedValueStore,
    HostedClueLog,
    1,
    1,
    PORTS,
    1,
    PORTS,
    1,
    1,
    1,
>;

impl StdHost {
    pub fn run_copy_fragment(
        &mut self,
        request_id: CopyRequestId,
        fragment: PlanFragment,
        registry: &ProtectedFileRegistry,
        stop: &CopyStopToken,
    ) -> Result<CopyRunReceipt, String> {
        self.run_copy_fragment_with_faults(
            request_id,
            fragment,
            registry,
            stop,
            ExecutionFaults::default(),
        )
    }

    pub(super) fn run_copy_fragment_with_faults(
        &mut self,
        request_id: CopyRequestId,
        fragment: PlanFragment,
        registry: &ProtectedFileRegistry,
        stop: &CopyStopToken,
        faults: ExecutionFaults,
    ) -> Result<CopyRunReceipt, String> {
        let placement = exact_copy_placement(&fragment)?;
        let source_binding = protected_binding(placement, conduit_std_catalog::COPY_SOURCE_ROLE)?;
        let destination_binding =
            protected_binding(placement, conduit_std_catalog::COPY_DESTINATION_ROLE)?;
        let source_id = source_binding.handle_id.clone();
        let destination_id = destination_binding.handle_id.clone();

        let play_sequence = self.next_kernel_play_sequence;
        self.next_kernel_play_sequence = play_sequence
            .checked_add(1)
            .ok_or_else(|| "copy Play sequence exhausted".to_string())?;
        let active_play = bind_active_play(
            &fragment.plan_id,
            &self.advertisement.host_id,
            &self.advertisement.boot_id,
            play_sequence,
        );
        let make_receipt = |result, kernel_events| CopyRunReceipt {
            request_id: request_id.clone(),
            run_id: active_play.active_play_id.clone(),
            plan_id: fragment.plan_id.clone(),
            source_binding_id: source_id.clone(),
            destination_binding_id: destination_id.clone(),
            result,
            kernel_events,
        };

        let source = match resolve_entry(registry, placement, source_binding) {
            Ok(entry) => entry,
            Err(result) => return Ok(make_receipt(result, 0)),
        };
        let destination = match resolve_entry(registry, placement, destination_binding) {
            Ok(entry) => entry,
            Err(result) => return Ok(make_receipt(result, 0)),
        };
        if source.path == destination.path {
            return Ok(make_receipt(CopyResult::Denied, 0));
        }
        let source_bytes = match source.path.metadata() {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok(make_receipt(CopyResult::Denied, 0));
            }
            Err(_) => return Ok(make_receipt(CopyResult::StaleHandle, 0)),
        };
        let maximum_bytes = source_binding
            .maximum_bytes
            .min(destination_binding.maximum_bytes)
            .min(MAX_COPY_BYTES);
        if source_bytes > maximum_bytes {
            return Ok(make_receipt(
                CopyResult::Oversized {
                    source_bytes,
                    maximum_bytes,
                },
                0,
            ));
        }
        if destination_binding.commit_policy == ProtectedResourceCommitPolicy::CreateOnly
            && destination.path.exists()
        {
            return Ok(make_receipt(CopyResult::DestinationExists, 0));
        }

        let advertisement = self.advertisement.clone();
        let reservation = self
            .kernel_resources
            .prepare_and_reserve(&advertisement, &fragment)?;
        let execution = execute_copy(
            &fragment,
            source,
            destination,
            destination_binding.commit_policy,
            maximum_bytes,
            stop,
            faults,
        );
        let release = self.kernel_resources.release(reservation);
        let (result, kernel_events) = execution?;
        release?;
        Ok(make_receipt(result, kernel_events))
    }
}

fn exact_copy_placement(fragment: &PlanFragment) -> Result<&conduit_core::PlannedGear, String> {
    if fragment.placements.len() != 1 || !fragment.connections.is_empty() {
        return Err("copy Plan fragment must contain one operation and zero cords".to_string());
    }
    let placement = &fragment.placements[0];
    if placement.kind_id.as_str() != conduit_std_catalog::COPY_FILE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_std_catalog::COPY_FILE_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_catalog::COPY_FILE_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_catalog::COPY_FILE_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_catalog::COPY_FILE_ARTIFACT
        || !placement.inputs.is_empty()
        || !placement.outputs.is_empty()
        || placement.host_operations != conduit_std_catalog::copy_file_offer().host_operations
        || placement.resources.len() != 2
    {
        return Err(
            "copy executable identity does not match the installed implementation".to_string(),
        );
    }
    Ok(placement)
}

fn protected_binding<'a>(
    placement: &'a conduit_core::PlannedGear,
    role: &str,
) -> Result<&'a ProtectedResourceBinding, String> {
    let mut bindings = placement.resources.iter().filter_map(|binding| {
        binding
            .protected
            .as_ref()
            .filter(|protected| protected.role_id.as_str() == role)
    });
    let binding = bindings
        .next()
        .ok_or_else(|| format!("copy Plan is missing protected role '{role}'"))?;
    if bindings.next().is_some() {
        return Err(format!("copy Plan repeats protected role '{role}'"));
    }
    Ok(binding)
}

fn resolve_entry<'a>(
    registry: &'a ProtectedFileRegistry,
    placement: &conduit_core::PlannedGear,
    binding: &ProtectedResourceBinding,
) -> Result<&'a ProtectedFileEntry, CopyResult> {
    let entry = registry
        .get(&binding.handle_id)
        .ok_or(CopyResult::StaleHandle)?;
    let grant = &entry.grant;
    if grant.handle_id != binding.handle_id
        || grant.role_id != binding.role_id
        || grant.gear_id != placement.gear_id
        || grant.host_id != placement.host_id
        || grant.boot_id != placement.boot_id
        || grant.capability_id != placement.capability_id
        || grant.class_id.as_str() != conduit_std_catalog::PROTECTED_FILE_RESOURCE_CLASS
        || grant.access != binding.access
        || grant.maximum_bytes != binding.maximum_bytes
        || grant.commit_policy != binding.commit_policy
    {
        return Err(CopyResult::StaleHandle);
    }
    if entry.availability == ProtectedFileAvailability::Denied
        || (binding.role_id.as_str() == conduit_std_catalog::COPY_SOURCE_ROLE
            && binding.access != ProtectedResourceAccess::ReadExisting)
        || (binding.role_id.as_str() == conduit_std_catalog::COPY_DESTINATION_ROLE
            && !matches!(
                binding.access,
                ProtectedResourceAccess::Create | ProtectedResourceAccess::Replace
            ))
    {
        return Err(CopyResult::Denied);
    }
    Ok(entry)
}

fn execute_copy(
    fragment: &PlanFragment,
    source: &ProtectedFileEntry,
    destination: &ProtectedFileEntry,
    policy: ProtectedResourceCommitPolicy,
    maximum_bytes: u64,
    stop: &CopyStopToken,
    faults: ExecutionFaults,
) -> Result<(CopyResult, usize), String> {
    let mut files = match CopyFiles::prepare(
        &source.path,
        &destination.path,
        policy,
        maximum_bytes,
        faults,
    ) {
        Ok(files) => files,
        Err(result) => return Ok((result, 0)),
    };
    let mut scheduler = copy_scheduler(fragment)?;
    let mut result = None;
    loop {
        while let Some(request) = scheduler.next_host_request() {
            if stop.is_requested()
                || files
                    .faults
                    .stop_after_bytes
                    .is_some_and(|limit| files.bytes_copied >= limit)
            {
                scheduler
                    .cancel()
                    .map_err(|error| format!("cancel copy kernel: {error:?}"))?;
                result = Some(if files.cleanup() {
                    CopyResult::Cancelled {
                        bytes_copied: files.bytes_copied,
                    }
                } else {
                    CopyResult::CleanupFailed {
                        bytes_copied: files.bytes_copied,
                    }
                });
                break;
            }
            match files.step() {
                Ok(true) => complete_continue(&mut scheduler, request)?,
                Ok(false) => {
                    scheduler
                        .complete_host_operation(
                            request.node,
                            request.request,
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Completed,
                                output: None,
                                failure: None,
                            },
                        )
                        .map_err(|error| format!("complete copy commit: {error:?}"))?;
                    result = Some(CopyResult::Success {
                        bytes_copied: files.bytes_copied,
                    });
                }
                Err(copy_result) => {
                    scheduler
                        .complete_host_operation(
                            request.node,
                            request.request,
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Failed,
                                output: None,
                                failure: None,
                            },
                        )
                        .map_err(|error| format!("complete failed copy step: {error:?}"))?;
                    result = Some(copy_result);
                }
            }
        }
        let status = match scheduler.step() {
            Ok(status) => status,
            Err(conduit_kernel::scheduler::SchedulerError::OperationFailed(_))
                if result.is_some() =>
            {
                break;
            }
            Err(error) => return Err(format!("copy kernel step: {error:?}")),
        };
        match status {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete | SchedulerStatus::Cancelled => break,
            SchedulerStatus::Idle => return Err("copy kernel became idle".to_string()),
        }
    }
    if scheduler.values().used_items() != 0 {
        return Err("copy kernel retained values after terminal state".to_string());
    }
    Ok((
        result.ok_or_else(|| "copy kernel terminated without a result".to_string())?,
        usize::from(scheduler.clues().len()),
    ))
}

fn complete_continue(
    scheduler: &mut CopyScheduler,
    request: HostOperationRequest,
) -> Result<(), String> {
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(request.input),
                failure: None,
            },
        )
        .map_err(|error| format!("complete copy chunk: {error:?}"))
}

fn copy_scheduler(fragment: &PlanFragment) -> Result<CopyScheduler, String> {
    let lowered =
        lower_plan_fragment(fragment).map_err(|error| format!("lower copy: {error:?}"))?;
    if lowered.nodes.len() != 1 || !lowered.cords.is_empty() || lowered.host_operations.len() != 1 {
        return Err(
            "lowered copy shape is not one node, zero cords, one host operation".to_string(),
        );
    }
    let mut values = HostedValueStore::new(1, 1, 1)
        .map_err(|error| format!("prepare copy values: {error:?}"))?;
    let command = values
        .store(&[0])
        .map_err(|error| format!("store copy command: {error:?}"))?;
    let driver = OperationDriver::new(CopyOperation::new(command))
        .map_err(|error| format!("prepare copy operation: {error:?}"))?;
    let mut routes = FixedRoutes::<PORTS, 1>::new(PORTS as u16);
    routes
        .seal()
        .map_err(|error| format!("seal empty copy routes: {error:?}"))?;
    let mut bindings = FixedHostOperationBindings::<1>::new(1);
    bindings
        .install(
            lowered.host_operations[0].node,
            lowered.host_operations[0].binding,
        )
        .map_err(|error| format!("install copy host operation: {error:?}"))?;
    bindings
        .seal()
        .map_err(|error| format!("seal copy host operation: {error:?}"))?;
    let inactive_cord = CordSpec {
        cord: CordId(u16::MAX),
        source: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        sink: CordEndpoint::local(NodeId(u16::MAX), PortId(u16::MAX)),
        slot_start: u16::MAX,
        item_capacity: 0,
        byte_capacity: 0,
    };
    let clue_bytes = u32::from(MAX_CLUE_ITEMS)
        .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
        .ok_or_else(|| "copy clue byte budget overflow".to_string())?;
    let clue = HostedClueLog::new(MAX_CLUE_ITEMS, clue_bytes)
        .map_err(|error| format!("prepare copy clue: {error:?}"))?;
    CopyScheduler::new_with_active_counts_and_host_operations(
        1,
        0,
        [lowered.node_specs[0]],
        [inactive_cord],
        routes,
        bindings,
        [driver],
        values,
        clue,
    )
    .map_err(|error| format!("prepare copy scheduler: {error:?}"))
}
