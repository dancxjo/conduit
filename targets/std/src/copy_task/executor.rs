use super::base::{CopyFiles, ExecutionFaults};
use super::model::{CopyRequestId, CopyResult, CopyRunReceipt, CopyStopToken};
use super::registry::{ProtectedFileAvailability, ProtectedFileEntry, ProtectedFileRegistry};
use super::scheduler::{prepare_copy_scheduler, CopyScheduler};
use crate::{IssuedKernelPlay, StdHost};
use conduit_core::{
    PlanFragment, ProtectedResourceAccess, ProtectedResourceBinding, ProtectedResourceCommitPolicy,
};
use conduit_kernel::scheduler::{HostOperationRequest, SchedulerStatus};
use conduit_kernel::{HostOperationDisposition, HostOperationOutcome, SignSink, ValueStorage};
use conduit_plan_lowering::lowering::lower_plan_fragment;

const MAX_COPY_BYTES: u64 = 16 * 1024 * 1024;

pub(super) struct CopyRunContext<'a> {
    pub(super) play: IssuedKernelPlay,
    pub(super) request_id: CopyRequestId,
    pub(super) fragment: PlanFragment,
    pub(super) registry: &'a mut ProtectedFileRegistry,
    pub(super) stop: &'a CopyStopToken,
    pub(super) faults: ExecutionFaults,
}

impl StdHost {
    pub fn run_copy_fragment(
        &mut self,
        play: IssuedKernelPlay,
        request_id: CopyRequestId,
        fragment: PlanFragment,
        registry: &mut ProtectedFileRegistry,
        stop: &CopyStopToken,
    ) -> Result<CopyRunReceipt, String> {
        self.run_copy_fragment_with_faults(
            play,
            request_id,
            fragment,
            registry,
            stop,
            ExecutionFaults::default(),
        )
    }

    pub(super) fn run_copy_fragment_with_faults(
        &mut self,
        play: IssuedKernelPlay,
        request_id: CopyRequestId,
        fragment: PlanFragment,
        registry: &mut ProtectedFileRegistry,
        stop: &CopyStopToken,
        faults: ExecutionFaults,
    ) -> Result<CopyRunReceipt, String> {
        self.run_copy_fragment_with_use_hook(
            CopyRunContext {
                play,
                request_id,
                fragment,
                registry,
                stop,
                faults,
            },
            |_| {},
        )
    }

    pub(super) fn run_copy_fragment_with_use_hook(
        &mut self,
        context: CopyRunContext<'_>,
        before_use: impl FnOnce(&mut ProtectedFileRegistry),
    ) -> Result<CopyRunReceipt, String> {
        let CopyRunContext {
            play,
            request_id,
            fragment,
            registry,
            stop,
            faults,
        } = context;
        let placement = exact_copy_placement(&fragment)?;
        let source_binding =
            protected_binding(placement, conduit_semantic_catalog::COPY_SOURCE_ROLE)?;
        let destination_binding =
            protected_binding(placement, conduit_semantic_catalog::COPY_DESTINATION_ROLE)?;
        let source_id = source_binding.handle_id.clone();
        let destination_id = destination_binding.handle_id.clone();

        let active_play = play.identity();
        if active_play.plan_id != fragment.plan_id
            || active_play.host_id != fragment.host_id
            || active_play.boot_id != fragment.boot_id
        {
            return Err("copy Play identity does not match its immutable Plan fragment".into());
        }
        let make_receipt = |result, kernel_events, presented_result| CopyRunReceipt {
            request_id: request_id.clone(),
            run_id: active_play.active_play_id.clone(),
            plan_id: fragment.plan_id.clone(),
            source_binding_id: source_id.clone(),
            destination_binding_id: destination_id.clone(),
            result,
            kernel_events,
            presented_result,
        };

        for binding in [source_binding, destination_binding] {
            if let Err(result) = resolve_entry(registry, placement, binding) {
                return Ok(make_receipt(result, 0, None));
            }
        }

        let advertisement = self.advertisement.clone();
        let reservation = self
            .kernel_resources
            .prepare_and_reserve(&advertisement, &fragment)?;
        before_use(registry);
        let execution = use_protected_copy_resources(
            &fragment,
            registry,
            placement,
            source_binding,
            destination_binding,
            stop,
            faults,
        );
        let release = self.kernel_resources.release(reservation);
        let (result, kernel_events, presented_result) = execution?;
        release?;
        Ok(make_receipt(result, kernel_events, presented_result))
    }
}

fn use_protected_copy_resources(
    fragment: &PlanFragment,
    registry: &ProtectedFileRegistry,
    placement: &conduit_core::PlannedGear,
    source_binding: &ProtectedResourceBinding,
    destination_binding: &ProtectedResourceBinding,
    stop: &CopyStopToken,
    faults: ExecutionFaults,
) -> Result<(CopyResult, usize, Option<conduit_core::StructuredInfoValue>), String> {
    let source = match resolve_entry(registry, placement, source_binding) {
        Ok(entry) => entry,
        Err(result) => return Ok((result, 0, None)),
    };
    let destination = match resolve_entry(registry, placement, destination_binding) {
        Ok(entry) => entry,
        Err(result) => return Ok((result, 0, None)),
    };
    if source.path == destination.path {
        return Ok((CopyResult::Denied, 0, None));
    }
    let source_bytes = match source.path.metadata() {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return Ok((CopyResult::Denied, 0, None));
        }
        Err(_) => return Ok((CopyResult::StaleHandle, 0, None)),
    };
    let maximum_bytes = source_binding
        .maximum_bytes
        .min(destination_binding.maximum_bytes)
        .min(MAX_COPY_BYTES);
    if source_bytes > maximum_bytes {
        return Ok((
            CopyResult::Oversized {
                source_bytes,
                maximum_bytes,
            },
            0,
            None,
        ));
    }
    if destination_binding.commit_policy == ProtectedResourceCommitPolicy::CreateOnly
        && destination.path.exists()
    {
        return Ok((CopyResult::DestinationExists, 0, None));
    }
    execute_copy(
        fragment,
        source,
        destination,
        source_bytes,
        destination_binding.commit_policy,
        stop,
        faults,
    )
}

fn exact_copy_placement(fragment: &PlanFragment) -> Result<&conduit_core::PlannedGear, String> {
    if fragment.placements.len() != 2 || fragment.connections.len() != 1 {
        return Err("copy Plan fragment must contain copy, presentation, and one Cord".to_string());
    }
    let placement = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::COPY_FILE_KIND)
        .ok_or_else(|| "copy Plan has no copy placement".to_string())?;
    if placement.kind_id.as_str() != conduit_semantic_catalog::COPY_FILE_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::COPY_FILE_CONTRACT_REVISION
        || placement.execution_profile_id.as_str()
            != conduit_std_offers::COPY_FILE_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::COPY_FILE_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::COPY_FILE_ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs.len() != 1
        || placement.host_operations != conduit_std_offers::copy_file_offer().host_operations
        || placement.resources.len() != 2
    {
        return Err(
            "copy executable identity does not match the installed implementation".to_string(),
        );
    }
    let presenter = fragment
        .placements
        .iter()
        .find(|candidate| {
            candidate.implementation_id.as_str()
                == conduit_std_offers::COPY_RESULT_PRESENTATION_IMPLEMENTATION
        })
        .ok_or_else(|| "copy Plan has no result presentation placement".to_string())?;
    let presentation_offer = conduit_std_offers::copy_result_presentation_offer();
    if presenter.kind_id != presentation_offer.kind_id
        || presenter.kind_contract_revision != presentation_offer.kind_contract_revision
        || presenter.execution_profile_id != presentation_offer.implementation.execution_profile_id
        || presenter.artifact_id != presentation_offer.implementation.artifact_id
        || presenter.inputs != presentation_offer.inputs
        || !presenter.outputs.is_empty()
        || presenter.host_operations != presentation_offer.host_operations
        || presenter.resources.len() != 1
    {
        return Err(
            "copy result presentation identity does not match the installed implementation"
                .to_string(),
        );
    }
    let connection = &fragment.connections[0];
    if connection.source_placement_id != placement.placement_id
        || connection.source_port_id.as_str() != "result"
        || connection.sink_placement_id != presenter.placement_id
        || connection.sink_port_id.as_str() != "input"
        || connection.value_kind != placement.outputs[0].value_kind
        || connection.item_capacity != 1
        || connection.byte_capacity != conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        || connection.selected_line.is_some()
        || !connection.admitted_lines.is_empty()
    {
        return Err("copy result Cord does not match the exact planned local route".to_string());
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
        || grant.class_id.as_str() != conduit_semantic_catalog::PROTECTED_FILE_RESOURCE_CLASS
        || grant.access != binding.access
        || grant.maximum_bytes != binding.maximum_bytes
        || grant.commit_policy != binding.commit_policy
    {
        return Err(CopyResult::StaleHandle);
    }
    if entry.availability == ProtectedFileAvailability::Denied
        || (binding.role_id.as_str() == conduit_semantic_catalog::COPY_SOURCE_ROLE
            && binding.access != ProtectedResourceAccess::ReadExisting)
        || (binding.role_id.as_str() == conduit_semantic_catalog::COPY_DESTINATION_ROLE
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
    source_bytes: u64,
    policy: ProtectedResourceCommitPolicy,
    stop: &CopyStopToken,
    faults: ExecutionFaults,
) -> Result<(CopyResult, usize, Option<conduit_core::StructuredInfoValue>), String> {
    let maximum_bytes = source
        .grant
        .maximum_bytes
        .min(destination.grant.maximum_bytes)
        .min(MAX_COPY_BYTES);
    let mut files = match CopyFiles::prepare(
        &source.path,
        &destination.path,
        policy,
        maximum_bytes,
        faults,
    ) {
        Ok(files) => files,
        Err(result) => return Ok((result, 0, None)),
    };
    let (mut scheduler, success_encoded) = prepare_copy_scheduler(fragment, source_bytes)?;
    let lowered =
        lower_plan_fragment(fragment).map_err(|error| format!("lower copy: {error:?}"))?;
    let presentation_node = fragment
        .placements
        .iter()
        .find(|placement| {
            placement.implementation_id.as_str()
                == conduit_std_offers::COPY_RESULT_PRESENTATION_IMPLEMENTATION
        })
        .and_then(|placement| {
            lowered
                .nodes
                .iter()
                .find(|node| node.placement_id == placement.placement_id)
        })
        .map(|node| node.node)
        .ok_or_else(|| "copy result presentation node is missing".to_string())?;
    let mut result = None;
    let mut presented_encoded =
        Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    loop {
        while let Some(request) = scheduler.next_host_request() {
            if request.node == presentation_node {
                let encoded = scheduler
                    .host_value(request.input.value)
                    .map_err(|error| format!("read copy presentation: {error:?}"))?;
                presented_encoded.extend_from_slice(encoded);
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
                    .map_err(|error| format!("complete copy presentation: {error:?}"))?;
                continue;
            }
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
                    let value = scheduler
                        .store_host_value(&success_encoded)
                        .map_err(|error| format!("store admitted copy result: {error:?}"))?;
                    let success_value = conduit_kernel::BoundedValueRef::new(
                        value,
                        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                    )
                    .map_err(|_| "copy result exceeded its admitted bound")?;
                    scheduler
                        .complete_host_operation(
                            request.node,
                            request.request,
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Completed,
                                output: Some(success_value),
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
    let presented_result = if presented_encoded.is_empty() {
        None
    } else {
        let value = conduit_core::StructuredInfoValue::from_canonical_bytes(&presented_encoded)
            .map_err(|error| format!("decode copy presentation: {error:?}"))?;
        if value.value_type() != &conduit_semantic_catalog::copy_result_type() {
            return Err("copy presentation has the wrong exact type".into());
        }
        Some(value)
    };
    Ok((
        result.ok_or_else(|| "copy kernel terminated without a result".to_string())?,
        usize::from(scheduler.signs().len()),
        presented_result,
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
