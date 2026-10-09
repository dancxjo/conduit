//! Local multi-partition composition of the existing kernel and Host effects.
use super::{
    fore_sign_storage, kernel_preparation::KernelTables, preparation, simple_presentation_host,
    InstalledBack, InstalledScheduler, MAX_CORDS, MAX_NODES, MAX_QUEUE_SLOTS, PENDING_REQUESTS,
};
use crate::{
    body_execution::BodyForeOutputAdapter, hosted_keyboard::HostedKeyboardAdapter,
    BodyLiveForeQueue, ExternalForeDelivery, ExternalForeInput, RunControl, TimerAdapter,
};
use conduit_body::BodyPlotPlan;
use conduit_core::{
    BodyClockCorrelation, BodyTimeQuality, BodyTimeRefusal, BodyTimeRequirement,
    CancellationReason, FailureReason, PlanFragment, TerminalDisposition,
};
use conduit_kernel::{
    scheduler::{HostCallRequest, SchedulerStatus},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome, HostedValueStore,
    KernelEvent,
};
use conduit_plan_lowering::{
    activation_fragment::{lower_fragment_activations, LoweredFragmentActivations},
    fragment_set::{lower_local_fragment_set_from_plans, FragmentSetBounds},
    lowering::{KernelIdentityMap, LoweredHostCall, FIXED_KERNEL_STORAGE_PROFILE},
};
use std::io::Write;

mod clock_observation;
mod fore_route;
mod scan_route;
use crate::body_execution::ObservedKernelEvent;
use clock_observation::KernelClockObservations;
use fore_route::BodyForeRoute;

pub(crate) struct BodyKernel<'a> {
    scheduler: InstalledScheduler,
    fore: BodyForeRoute<'a>,
    activations: Vec<LoweredFragmentActivations>,
    partitions: Vec<KernelIdentityMap>,
    operations: Vec<LoweredHostCall>,
    typed_record_hosts: Vec<Option<super::typed_record_back::TypedRecordHost>>,
    image_text_hosts: Vec<Option<super::image_text_back::ImageTextHost>>,
    image_text_record_hosts: Vec<Option<super::image_text_record_back::ImageTextRecordHost>>,
    text_state_hosts: Vec<Option<super::text_state_back::TextStateHost>>,
    input_keymaps: [conduit_human::ConduitIntlKeymap; MAX_NODES],
    requests: Vec<HostCallRequest>,
    clock_observations: KernelClockObservations,
    supported_todo_scan: bool,
    todo_checkpoint: Option<crate::todo_checkpoint_call::TodoCheckpointHost>,
    todo_checkpoint_read: Option<crate::todo_checkpoint_read_call::TodoCheckpointReadHost>,
}

pub(crate) struct BodyKernelResult {
    pub terminal: TerminalDisposition,
    pub failure: Option<String>,
    pub kernel_failure: Option<Failure>,
    pub cleanup_failure: Option<String>,
    pub partitions: Vec<KernelIdentityMap>,
    pub requests: Vec<HostCallRequest>,
    pub events: Vec<KernelEvent>,
    pub scan_child_signs:
        Result<Vec<conduit_composite::ScanChildSignReceipt>, conduit_composite::BoundedScanError>,
    pub scan_cancellation_failed: bool,
    pub scan_output_completion_failed: bool,
    pub fore_deliveries: Vec<ExternalForeDelivery>,
    pub clock_observations: Vec<ObservedKernelEvent>,
    pub clock_quality: Option<BodyTimeQuality>,
    pub clock_execution_bounds: Option<(
        conduit_core::MonotonicDuration,
        conduit_core::MonotonicDuration,
    )>,
}

fn keyboard(contract: &conduit_core::HostCallContractId) -> bool {
    contract.as_str() == conduit_std_offers::NEXT_KEY_EVENT_HOST_CALL_CONTRACT
}
fn button(contract: &conduit_core::HostCallContractId) -> bool {
    contract.as_str() == conduit_std_offers::button::NEXT_TRANSITION_HOST_CALL
}
fn text_state(contract: &conduit_core::HostCallContractId) -> bool {
    contract.as_str() == conduit_std_offers::TEXT_STATE_HOST_CALL
}
fn input_semantic(contract: &conduit_core::HostCallContractId) -> bool {
    matches!(
        contract.as_str(),
        conduit_std_offers::KEYMAP_HOST_CALL | conduit_std_offers::CHORDS_HOST_CALL
    )
}
fn todo_checkpoint(operation: &LoweredHostCall) -> bool {
    operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL
        && operation
            .target_kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == conduit_todo_plot::TODO_CHECKPOINT_KIND)
        && operation.binding.maximum_input_bytes == 4096
        && operation.binding.maximum_output_bytes == 4096
}
fn todo_checkpoint_read(operation: &LoweredHostCall) -> bool {
    operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_READ_CALL
        && operation
            .target_kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == conduit_todo_plot::TODO_CHECKPOINT_READ_KIND)
        && operation.binding.maximum_input_bytes == 0
        && operation.binding.maximum_output_bytes == conduit_todo_plot::STATE_MAX_BYTES as u32
}
fn timer(contract: &conduit_core::HostCallContractId) -> bool {
    contract.as_str() == conduit_core::WAIT_HOST_CALL_CONTRACT
}
fn typed_record_codec(contract: &conduit_core::HostCallContractId) -> bool {
    [
        conduit_std_offers::TYPED_RECORD_FRAME_HOST_CALL,
        conduit_std_offers::TYPED_RECORD_DEFRAME_HOST_CALL,
        conduit_std_offers::TEXT_TO_TYPED_RECORD_HOST_CALL,
        conduit_std_offers::TYPED_RECORD_TO_TEXT_HOST_CALL,
    ]
    .contains(&contract.as_str())
}
fn image_text(contract: &conduit_core::HostCallContractId) -> bool {
    matches!(
        contract.as_str(),
        conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION
            | conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION
            | conduit_std_offers::IMAGE_TEXT_RECORD_OPERATION
    )
}
fn presentation(operation: &LoweredHostCall) -> bool {
    operation.target_kind.as_ref().is_some_and(|target| {
        matches!(
            target.as_str(),
            "presentation/stdout-text"
                | conduit_std_offers::TICK_PRESENTATION_TARGET
                | conduit_std_offers::COUNT_PRESENTATION_TARGET
                | conduit_std_offers::BOOL_PRESENTATION_TARGET
        ) && operation.contract_id
            == conduit_core::present_host_call_requirement(
                target.clone(),
                operation.binding.maximum_input_bytes,
            )
            .contract_id
    })
}

impl<'a> BodyKernel<'a> {
    pub(crate) fn prepare(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
    ) -> Result<Self, String> {
        Self::prepare_with_live(
            partitions,
            has_keyboard,
            parent_play,
            fore_inputs,
            sequential_fore,
            has_fore_output,
            None,
        )
    }

    pub(crate) fn prepare_live(
        partitions: &[BodyPlotPlan],
        parent_play: &conduit_core::ActivePlayId,
        queue: &BodyLiveForeQueue,
        control: &RunControl,
    ) -> Result<Self, String> {
        if partitions.len() != 1 || !queue.matches_plan(&partitions[0].plan, control) {
            return Err("live Fore queue differs from the exact Body Plan or stop control".into());
        }
        Self::prepare_with_live(partitions, false, parent_play, &[], true, true, Some(queue))
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_with_live(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
        live: Option<&BodyLiveForeQueue>,
    ) -> Result<Self, String> {
        let fragments = partitions
            .iter()
            .map(|partition| {
                partition
                    .plan
                    .fragments
                    .first()
                    .filter(|_| partition.plan.fragments.len() == 1)
                    .ok_or("local body execution requires one local fragment per Plot")
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Activation coordinators belong to the whole sealed Plan. Bind each
        // entry to its exact local fragment before lowering ordinary nodes.
        let activations = bind_body_activations(partitions, &fragments)?;
        let mut scans = scan_route::prepare(partitions, parent_play)?;
        if scans.len() == 1 {
            let conduit_core::PlannedActivationEntry::Scan(planned) =
                &partitions[scans[0].partition].plan.activations[0]
            else {
                return Err("installed scan activation changed during preparation".into());
            };
            if fore_inputs.len() > usize::from(planned.limits.maximum_items) {
                return Err("preloaded Todo commands exceed the selected scan bound".into());
            }
        }
        let supported_todo_scan = scans.len() == 1
            && partitions.len() == 1
            && fragments[0].placements.len() == 1
            && !has_keyboard
            && sequential_fore
            && has_fore_output
            && (live.is_some() || fore_inputs.len() <= 64)
            && fragments[0].placements[0].host_calls.is_empty();
        let plans = partitions
            .iter()
            .map(|partition| &partition.plan)
            .collect::<Vec<_>>();
        let lowered = lower_local_fragment_set_from_plans(
            &plans,
            FIXED_KERNEL_STORAGE_PROFILE,
            FragmentSetBounds {
                fragments: conduit_body::MAX_BODY_PLOTS as u16,
                nodes: MAX_NODES as u16,
                cords: MAX_CORDS as u16,
                queue_slots: MAX_QUEUE_SLOTS as u16,
                value_bytes: 16 * 1024 * 1024,
                sign_items: 4096,
                sign_bytes: 4 * 1024 * 1024,
            },
        )
        .map_err(|error| format!("Body fragment lowering: {error:?}"))?;
        let fore = if let Some(queue) = live {
            BodyForeRoute::prepare_live(&lowered.partitions, queue)?
        } else {
            BodyForeRoute::prepare(
                &lowered.partitions,
                fore_inputs,
                sequential_fore,
                has_fore_output,
            )?
        };
        for operation in lowered.partitions.iter().flat_map(|part| &part.host_calls) {
            if keyboard(&operation.contract_id) || button(&operation.contract_id) {
                if !has_keyboard {
                    return Err("Body keyboard has no admitted adapter".into());
                }
            } else if !timer(&operation.contract_id)
                && !typed_record_codec(&operation.contract_id)
                && !image_text(&operation.contract_id)
                && !text_state(&operation.contract_id)
                && !input_semantic(&operation.contract_id)
                && !todo_checkpoint(operation)
                && !todo_checkpoint_read(operation)
                && !presentation(operation)
            {
                return Err(format!(
                    "Body Host Call is unsupported: {}",
                    operation.contract_id.as_str()
                ));
            }
        }
        let mut items = 0_u16;
        let mut bytes = 0_u32;
        let mut maximum = 1_u32;
        let mut sign_items = 32_u16;
        let mut request_capacity = 0_usize;
        for (partition_index, fragment) in fragments.iter().enumerate() {
            for placement in &fragment.placements {
                let ordinary;
                let budget = if let Some(scan) = scans.iter().find(|scan| {
                    scan.partition == partition_index && scan.owner == placement.placement_id
                }) {
                    &scan.budget
                } else {
                    // Ordinary Backs are budgeted only after exact activation
                    // owners have been consumed by the whole-Plan scan route.
                    ordinary = preparation::back_budget(placement)?;
                    &ordinary
                };
                items = items
                    .checked_add(budget.value_items)
                    .ok_or("Body value item overflow")?;
                bytes = bytes
                    .checked_add(budget.value_bytes)
                    .filter(|bytes| *bytes <= 16 * 1024 * 1024)
                    .ok_or("Body value byte capacity exceeded")?;
                maximum = maximum.max(budget.maximum_value_bytes);
                sign_items = sign_items
                    .checked_add(budget.sign_items)
                    .ok_or("Body Sign overflow")?;
                request_capacity = request_capacity
                    .checked_add(budget.host_requests)
                    .ok_or("Body request overflow")?;
            }
        }
        for port in fore.input_ports() {
            items = items
                .checked_add(port.item_capacity)
                .ok_or("Body Fore value item overflow")?;
            bytes = bytes
                .checked_add(port.byte_capacity)
                .filter(|bytes| *bytes <= 16 * 1024 * 1024)
                .ok_or("Body Fore value byte capacity exceeded")?;
            maximum = maximum.max(port.byte_capacity);
        }
        sign_items = sign_items
            .checked_add(fore.sign_items()?)
            .ok_or("Body Fore Sign overflow")?;
        let mut values = HostedValueStore::new(items.max(1), maximum, bytes.max(1))
            .map_err(|error| format!("Body value store: {error:?}"))?;
        let mut drivers = core::array::from_fn(|_| InstalledBack::inactive());
        for (partition_index, (fragment, part)) in
            fragments.iter().zip(&lowered.partitions).enumerate()
        {
            for node in &part.nodes {
                drivers[usize::from(node.node.0)] = if let Some(scan) =
                    scans.iter_mut().find(|scan| {
                        scan.partition == partition_index && scan.owner == node.placement_id
                    }) {
                    InstalledBack::BodyScan(Box::new(
                        scan.back
                            .take()
                            .ok_or("installed scan owner was prepared twice")?,
                    ))
                } else {
                    preparation::prepare_ordinary_operation(
                        fragment,
                        &node.placement_id,
                        &mut values,
                    )?
                };
            }
        }
        if scans.iter().any(|scan| scan.back.is_some()) {
            return Err("installed scan has no lowered owner node".into());
        }
        let tables = KernelTables::prepare(&lowered.partitions.iter().collect::<Vec<_>>())?;
        let signs = fore_sign_storage::prepare(sign_items, fore.has_ports())?;
        let typed_record_hosts = fragments
            .iter()
            .flat_map(|fragment| super::typed_record_back::prepare_hosts(fragment))
            .collect();
        let image_text_hosts = fragments
            .iter()
            .flat_map(|fragment| super::image_text_back::prepare_hosts(fragment))
            .collect();
        let image_text_record_hosts = fragments
            .iter()
            .flat_map(|fragment| super::image_text_record_back::prepare_hosts(fragment))
            .collect();
        let text_state_hosts = fragments
            .iter()
            .flat_map(|fragment| {
                fragment
                    .placements
                    .iter()
                    .map(super::text_state_back::TextStateHost::from_placement)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let scheduler = tables.install(drivers, values, signs)?;
        Ok(Self {
            scheduler,
            fore,
            activations,
            operations: lowered
                .partitions
                .iter()
                .flat_map(|part| part.host_calls.clone())
                .collect(),
            partitions: lowered
                .partitions
                .into_iter()
                .map(|part| part.identity)
                .collect(),
            typed_record_hosts,
            image_text_hosts,
            image_text_record_hosts,
            text_state_hosts,
            input_keymaps: [conduit_human::ConduitIntlKeymap::new(); MAX_NODES],
            requests: Vec::with_capacity(request_capacity),
            clock_observations: KernelClockObservations::with_capacity(usize::from(sign_items)),
            supported_todo_scan,
            todo_checkpoint: None,
            todo_checkpoint_read: None,
        })
    }

    /// Bind at most one selected reader and publisher to their exact lowered
    /// placements before Play. No ambient storage fallback is available.
    pub(crate) fn attach_todo_checkpoint(
        &mut self,
        partitions: &[BodyPlotPlan],
        root: &std::path::Path,
        checkpoint: crate::todo_durable_resource::CheckpointIdentity,
    ) -> Result<(), String> {
        let mut selected = 0;
        for (partition, body_plot) in partitions.iter().enumerate() {
            let lowered = self
                .partitions
                .get(partition)
                .ok_or("Todo checkpoint partition missing")?;
            for placement in body_plot
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
            {
                match placement.implementation_id.as_str() {
                    conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION => {
                        if self.todo_checkpoint.is_some() {
                            return Err("duplicate Todo checkpoint publisher".into());
                        }
                        self.todo_checkpoint = Some(
                            crate::todo_checkpoint_call::TodoCheckpointHost::prepare(
                                root,
                                placement,
                                lowered,
                                checkpoint.clone(),
                            )
                            .map_err(|error| {
                                format!("prepare selected Todo checkpoint: {error:?}")
                            })?,
                        );
                        selected += 1;
                    }
                    conduit_std_offers::TODO_CHECKPOINT_READ_IMPLEMENTATION => {
                        if self.todo_checkpoint_read.is_some() {
                            return Err("duplicate Todo checkpoint reader".into());
                        }
                        self.todo_checkpoint_read = Some(
                            crate::todo_checkpoint_read_call::TodoCheckpointReadHost::prepare(
                                root,
                                placement,
                                lowered,
                                checkpoint.clone(),
                            )
                            .map_err(|error| {
                                format!("prepare selected Todo checkpoint read: {error:?}")
                            })?,
                        );
                        selected += 1;
                    }
                    _ => {}
                }
            }
        }
        if selected == 0 {
            return Err("Body has no selected Todo checkpoint placement".into());
        }
        Ok(())
    }

    /// Only the exact finite pure Todo coordinator has an installed Body
    /// route, with preloaded or admitted live typed Fore values. Face action
    /// routing and other activations remain separate or refused.
    pub(crate) fn require_supported_execution(&self) -> Result<(), String> {
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL
        }) && self.todo_checkpoint.is_none()
        {
            return Err("planned Todo checkpoint has no selected durable Host residence".into());
        }
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_READ_CALL
        }) && self.todo_checkpoint_read.is_none()
        {
            return Err(
                "planned Todo checkpoint read has no selected durable Host residence".into(),
            );
        }
        if self
            .activations
            .iter()
            .any(|bound| !bound.entries.is_empty())
            && !self.supported_todo_scan
        {
            return Err("Body activation coordinator is not installed".into());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run<W: Write, T: TimerAdapter>(
        mut self,
        output: &mut W,
        clock: &mut T,
        input: Option<&mut dyn HostedKeyboardAdapter>,
        mut fore_output: Option<&mut dyn BodyForeOutputAdapter>,
        control: &RunControl,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
        body_time: Option<(&BodyTimeRequirement, &BodyClockCorrelation)>,
        execution_bounds: Option<(
            conduit_core::MonotonicDuration,
            conduit_core::MonotonicDuration,
        )>,
        admitted_clock_quality: Option<BodyTimeQuality>,
    ) -> BodyKernelResult {
        let mut keys = super::keyboard_input_host::KeyboardInputHost::new(
            input,
            self.operations
                .iter()
                .map(|operation| &operation.contract_id),
        );
        let mut deadlines = super::deadline_host::InstalledDeadlineHost::<PENDING_REQUESTS>::new();
        let mut clock_quality = admitted_clock_quality;
        let mut kernel_failure = None;
        let result = (|| -> Result<TerminalDisposition, String> {
            let mut cancelling = false;
            loop {
                self.fore.drain(&mut self.scheduler, &mut fore_output)?;
                self.clock_observations.capture_new(
                    self.scheduler.signs().events(),
                    clock,
                    host_id,
                    boot_id,
                );
                if !cancelling && control.requested_stop().is_some() {
                    self.scheduler
                        .cancel()
                        .map_err(|error| format!("Body cancel: {error:?}"))?;
                    keys.cancel();
                    deadlines.clear();
                    cancelling = true;
                }
                let live_observed = self.fore.activity_generation();
                if !cancelling {
                    self.fore.start_and_feed(&mut self.scheduler)?;
                    if let Some((requirement, correlation)) = body_time {
                        let sample = match clock.monotonic_observation(host_id, boot_id) {
                            Some(sample) => sample,
                            None => {
                                clock_quality = Some(BodyTimeQuality::Unsupported {
                                    reason: BodyTimeRefusal::Unavailable,
                                });
                                return Err(
                                    "BodyTime-qualified Play lost its local monotonic clock".into(),
                                );
                            }
                        };
                        let quality = crate::body_execution::assess_continuing_body_clock(
                            requirement,
                            correlation,
                            &sample,
                            execution_bounds,
                            clock_quality.as_ref(),
                        );
                        clock_quality = Some(quality.clone());
                        match quality {
                            BodyTimeQuality::Ready { .. } => {}
                            quality => {
                                return Err(format!(
                                    "BodyTime-qualified Play lost clock quality: {quality:?}"
                                ))
                            }
                        }
                    }
                }
                while let Some(cancellation) = self.scheduler.next_host_cancellation() {
                    let operation = self
                        .operations
                        .iter()
                        .find(|op| op.node == cancellation.node && op.call == cancellation.call)
                        .ok_or("Body cancellation has no exact operation")?;
                    if keyboard(&operation.contract_id) || button(&operation.contract_id) {
                        keys.cancel();
                        self.scheduler
                            .complete_host_call(
                                cancellation.node,
                                cancellation.request,
                                HostCallOutcome {
                                    disposition: HostCallDisposition::Cancelled,
                                    output: None,
                                    failure: None,
                                },
                            )
                            .map_err(|error| format!("Body keyboard cancellation: {error:?}"))?;
                    } else {
                        deadlines.cancel(cancellation, &mut self.scheduler)?;
                    }
                }
                while let Some(request) = self.scheduler.next_host_request() {
                    self.clock_observations.capture_new(
                        self.scheduler.signs().events(),
                        clock,
                        host_id,
                        boot_id,
                    );
                    if !self.requests.iter().any(|observed| {
                        observed.node == request.node && observed.call == request.call
                    }) {
                        if self.requests.len() == self.requests.capacity() {
                            return Err("Body observed-operation capacity exceeded".into());
                        }
                        self.requests.push(request);
                    }
                    let operation = self
                        .operations
                        .iter()
                        .find(|op| op.node == request.node && op.call == request.call)
                        .ok_or("Body request has no exact partition operation")?;
                    let input = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|error| format!("Body request value: {error:?}"))?;
                    if operation.contract_id.as_str()
                        == conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL
                    {
                        let checkpoint = self
                            .todo_checkpoint
                            .as_ref()
                            .ok_or("Todo checkpoint Host Call has no selected residence")?;
                        let outcome = checkpoint.perform(request, input);
                        self.scheduler
                            .complete_host_call(request.node, request.request, outcome)
                            .map_err(|error| format!("Todo checkpoint completion: {error:?}"))?;
                        continue;
                    }
                    if operation.contract_id.as_str()
                        == conduit_std_offers::TODO_CHECKPOINT_READ_CALL
                    {
                        let reader = self
                            .todo_checkpoint_read
                            .as_ref()
                            .ok_or("Todo checkpoint read Host Call has no selected residence")?;
                        let result = reader.read(request, input);
                        let outcome = match result {
                            Ok(bytes) => {
                                let value =
                                    self.scheduler.store_host_value(&bytes).map_err(|error| {
                                        format!("Todo checkpoint read value storage: {error:?}")
                                    })?;
                                let output = BoundedValueRef::new(
                                    value,
                                    conduit_todo_plot::STATE_MAX_BYTES as u32,
                                )
                                .map_err(|error| {
                                    format!("Todo checkpoint read bound: {error:?}")
                                })?;
                                HostCallOutcome {
                                    disposition: HostCallDisposition::Completed,
                                    output: Some(output),
                                    failure: None,
                                }
                            }
                            Err(error) => HostCallOutcome {
                                disposition: if error
                                    == crate::todo_durable_resource::Refusal::InvalidBinding
                                {
                                    HostCallDisposition::Denied
                                } else {
                                    HostCallDisposition::Failed
                                },
                                output: None,
                                failure: Some(Failure {
                                    code: if error
                                        == crate::todo_durable_resource::Refusal::InvalidBinding
                                    {
                                        FailureCode::HostCallDenied
                                    } else {
                                        FailureCode::HostCallFailed
                                    },
                                    detail: crate::todo_checkpoint_read_call::read_failure_detail(
                                        &error,
                                    ),
                                }),
                            },
                        };
                        self.scheduler
                            .complete_host_call(request.node, request.request, outcome)
                            .map_err(|error| {
                                format!("Todo checkpoint read completion: {error:?}")
                            })?;
                        continue;
                    }
                    if keyboard(&operation.contract_id) {
                        keys.accept(
                            request,
                            input,
                            super::keyboard_input_host::InputRequestKind::Keyboard,
                        )?;
                        continue;
                    }
                    if button(&operation.contract_id) {
                        keys.accept(
                            request,
                            input,
                            super::keyboard_input_host::InputRequestKind::SpaceButton,
                        )?;
                        continue;
                    }
                    if text_state(&operation.contract_id) {
                        let completion = self
                            .text_state_hosts
                            .get_mut(usize::from(request.node.0))
                            .and_then(Option::as_mut)
                            .ok_or("Body text state has no admitted host")?
                            .execute(input);
                        let (disposition, output, failure) = match completion {
                            Ok(encoded) => {
                                let output = encoded
                                    .map(|encoded| self.scheduler.store_host_value(encoded))
                                    .transpose()
                                    .map_err(|error| {
                                        format!("Body text state output: {error:?}")
                                    })?
                                    .map(|value| {
                                        BoundedValueRef::new(
                                            value,
                                            operation.binding.maximum_output_bytes,
                                        )
                                    })
                                    .transpose()
                                    .map_err(|error| {
                                        format!("Body text state output bound: {error:?}")
                                    })?;
                                (HostCallDisposition::Completed, output, None)
                            }
                            Err(refusal) => (
                                HostCallDisposition::Failed,
                                None,
                                Some(conduit_kernel::Failure {
                                    code: match refusal {
                                        conduit_semantic_catalog::TextStateRefusal::CapacityExhausted => conduit_kernel::FailureCode::StateCapacityExhausted,
                                        conduit_semantic_catalog::TextStateRefusal::InvalidCapacity | conduit_semantic_catalog::TextStateRefusal::InvalidUtf8 => conduit_kernel::FailureCode::InvalidInput,
                                    },
                                    detail: 1,
                                }),
                            ),
                        };
                        self.scheduler
                            .complete_host_call(
                                request.node,
                                request.request,
                                HostCallOutcome {
                                    disposition,
                                    output,
                                    failure,
                                },
                            )
                            .map_err(|error| format!("Body text state completion: {error:?}"))?;
                        continue;
                    }
                    if input_semantic(&operation.contract_id) {
                        let node = usize::from(request.node.0);
                        let completion = super::input_semantic_backs::execute_host(
                            operation.contract_id.as_str() == conduit_std_offers::KEYMAP_HOST_CALL,
                            &mut self.input_keymaps[node],
                            input,
                        );
                        let outcome =
                            match completion {
                                Ok(Some(encoded)) => {
                                    let value = self
                                        .scheduler
                                        .store_host_value(encoded.as_slice())
                                        .map_err(|error| {
                                            format!("Body input semantic output: {error:?}")
                                        })?;
                                    HostCallOutcome {
                                    disposition: HostCallDisposition::Completed,
                                    output: Some(
                                        BoundedValueRef::new(
                                            value,
                                            operation.binding.maximum_output_bytes,
                                        )
                                        .map_err(|error| {
                                            format!("Body input semantic output bound: {error:?}")
                                        })?,
                                    ),
                                    failure: None,
                                }
                                }
                                Ok(None) => HostCallOutcome {
                                    disposition: HostCallDisposition::Completed,
                                    output: None,
                                    failure: None,
                                },
                                Err(failure) => HostCallOutcome {
                                    disposition: HostCallDisposition::Failed,
                                    output: None,
                                    failure: Some(failure),
                                },
                            };
                        self.scheduler
                            .complete_host_call(request.node, request.request, outcome)
                            .map_err(|error| {
                                format!("Body input semantic completion: {error:?}")
                            })?;
                        continue;
                    }
                    if typed_record_codec(&operation.contract_id) {
                        let completion = self
                            .typed_record_hosts
                            .get_mut(usize::from(request.node.0))
                            .and_then(Option::as_mut)
                            .ok_or("Body typed-record codec has no admitted host")?
                            .execute(input);
                        let (disposition, output, failure) = match completion {
                            Ok(encoded) => {
                                let value =
                                    self.scheduler.store_host_value(encoded).map_err(|error| {
                                        format!("Body typed-record output: {error:?}")
                                    })?;
                                let output = BoundedValueRef::new(
                                    value,
                                    operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("Body typed-record output bound: {error:?}")
                                })?;
                                (HostCallDisposition::Completed, Some(output), None)
                            }
                            Err(detail) => (
                                HostCallDisposition::Failed,
                                None,
                                Some(conduit_kernel::Failure {
                                    code: conduit_kernel::FailureCode::HostCallFailed,
                                    detail,
                                }),
                            ),
                        };
                        self.scheduler
                            .complete_host_call(
                                request.node,
                                request.request,
                                HostCallOutcome {
                                    disposition,
                                    output,
                                    failure,
                                },
                            )
                            .map_err(|error| format!("Body typed-record completion: {error:?}"))?;
                        continue;
                    }
                    if operation.contract_id.as_str()
                        == conduit_std_offers::IMAGE_TEXT_RECORD_OPERATION
                    {
                        let completion = self
                            .image_text_record_hosts
                            .get_mut(usize::from(request.node.0))
                            .and_then(Option::as_mut)
                            .ok_or("Body image-text record operation has no admitted host")?
                            .execute(input);
                        let (disposition, output, failure) = match completion {
                            Ok(encoded) => {
                                let value =
                                    self.scheduler.store_host_value(encoded).map_err(|error| {
                                        format!("Body image-text record output: {error:?}")
                                    })?;
                                let output = BoundedValueRef::new(
                                    value,
                                    operation.binding.maximum_output_bytes,
                                )
                                .map_err(|error| {
                                    format!("Body image-text record output bound: {error:?}")
                                })?;
                                (HostCallDisposition::Completed, Some(output), None)
                            }
                            Err(_) => (
                                HostCallDisposition::Failed,
                                None,
                                Some(conduit_kernel::Failure {
                                    code: conduit_kernel::FailureCode::HostCallFailed,
                                    detail: 1,
                                }),
                            ),
                        };
                        self.scheduler
                            .complete_host_call(
                                request.node,
                                request.request,
                                HostCallOutcome {
                                    disposition,
                                    output,
                                    failure,
                                },
                            )
                            .map_err(|error| {
                                format!("Body image-text record completion: {error:?}")
                            })?;
                        continue;
                    }
                    if matches!(
                        operation.contract_id.as_str(),
                        conduit_std_offers::IMAGE_TEXT_IMAGE_OPERATION
                            | conduit_std_offers::IMAGE_TEXT_CAPTION_OPERATION
                    ) {
                        let completion = self
                            .image_text_hosts
                            .get_mut(usize::from(request.node.0))
                            .and_then(Option::as_mut)
                            .ok_or("Body image-text operation has no admitted host")?
                            .execute(operation.contract_id.as_str(), input);
                        let (disposition, output, failure) = match completion {
                            Ok(encoded) => {
                                let output = encoded
                                    .map(|encoded| self.scheduler.store_host_value(encoded))
                                    .transpose()
                                    .map_err(|error| format!("Body image-text output: {error:?}"))?
                                    .map(|value| {
                                        BoundedValueRef::new(
                                            value,
                                            operation.binding.maximum_output_bytes,
                                        )
                                    })
                                    .transpose()
                                    .map_err(|error| {
                                        format!("Body image-text output bound: {error:?}")
                                    })?;
                                (HostCallDisposition::Completed, output, None)
                            }
                            Err(_) => (
                                HostCallDisposition::Failed,
                                None,
                                Some(conduit_kernel::Failure {
                                    code: conduit_kernel::FailureCode::HostCallFailed,
                                    detail: 1,
                                }),
                            ),
                        };
                        self.scheduler
                            .complete_host_call(
                                request.node,
                                request.request,
                                HostCallOutcome {
                                    disposition,
                                    output,
                                    failure,
                                },
                            )
                            .map_err(|error| format!("Body image-text completion: {error:?}"))?;
                        continue;
                    }
                    if timer(&operation.contract_id) {
                        let duration =
                            conduit_time::decode_tick(input).map_err(|error| error.to_string())?;
                        let now = clock.monotonic_now_ms().ok_or_else(|| {
                            "admitted monotonic wait Base is unavailable".to_string()
                        })?;
                        deadlines.arm(request, duration, now)?;
                        continue;
                    } else if !simple_presentation_host::present(
                        operation.target_kind.as_ref(),
                        input,
                        output,
                    )? {
                        return Err("Body presentation contract became unsupported".into());
                    }
                    self.scheduler
                        .complete_host_call(
                            request.node,
                            request.request,
                            HostCallOutcome {
                                disposition: HostCallDisposition::Completed,
                                output: None,
                                failure: None,
                            },
                        )
                        .map_err(|error| format!("Body Host completion: {error:?}"))?;
                }
                let status = self.scheduler.step().map_err(|error| {
                    if let conduit_kernel::scheduler::SchedulerError::BackFailed(failure) = error {
                        kernel_failure = Some(failure);
                    }
                    format!("Body kernel: {error:?}")
                })?;
                self.clock_observations.capture_new(
                    self.scheduler.signs().events(),
                    clock,
                    host_id,
                    boot_id,
                );
                match status {
                    SchedulerStatus::Drained => {
                        self.fore.require_normal_terminal(&self.scheduler)?;
                        return Ok(TerminalDisposition::Completed);
                    }
                    SchedulerStatus::Cancelled => {
                        return Ok(TerminalDisposition::Cancelled {
                            reason: CancellationReason::OperatorRequested,
                        })
                    }
                    SchedulerStatus::Progress { .. } => {
                        if keys.is_pending() {
                            keys.poll(&mut self.scheduler)?;
                        }
                    }
                    SchedulerStatus::Idle => {
                        if keys.poll(&mut self.scheduler)?
                            || deadlines.complete_next(&mut self.scheduler, clock)?
                        {
                            continue;
                        }
                        if let Some(observed) = live_observed {
                            if self.fore.wait_for_activity(observed)? {
                                continue;
                            }
                        }
                        if !keys.is_pending() && deadlines.is_empty() {
                            return Err("Body kernel has no admitted progress source".into());
                        }
                        std::thread::yield_now();
                    }
                }
            }
        })();
        self.clock_observations.capture_new(
            self.scheduler.signs().events(),
            clock,
            host_id,
            boot_id,
        );
        let mut cleanup_failure = None;
        let (terminal, failure) = match result {
            Ok(terminal) => (terminal, None),
            Err(error) => {
                keys.cancel();
                deadlines.clear();
                cleanup_failure = self
                    .scheduler
                    .cancel()
                    .err()
                    .map(|error| format!("Body failure cleanup: {error:?}"));
                (
                    TerminalDisposition::Failed {
                        reason: FailureReason::RequiredBranchFailed,
                    },
                    Some(error),
                )
            }
        };
        self.clock_observations.capture_new(
            self.scheduler.signs().events(),
            clock,
            host_id,
            boot_id,
        );
        // Presentation snapshots run after the sealed Play. Preserve child
        // identity and any receipt refusal instead of mixing child events
        // into the parent's unqualified kernel Sign stream.
        let scan_child_signs = self
            .scheduler
            .drivers()
            .iter()
            .filter_map(|driver| match driver {
                InstalledBack::BodyScan(scan) => Some(scan.as_ref()),
                _ => None,
            })
            .try_fold(Vec::new(), |mut receipts, scan| {
                receipts.extend(scan.child_sign_receipts()?);
                Ok(receipts)
            });
        let scan_cancellation_failed = self.scheduler.drivers().iter().any(
            |driver| matches!(driver, InstalledBack::BodyScan(scan) if scan.cancellation_failed()),
        );
        let scan_output_completion_failed = self.scheduler.drivers().iter().any(|driver| {
            matches!(driver, InstalledBack::BodyScan(scan) if scan.output_completion_failed())
        });
        BodyKernelResult {
            terminal,
            failure,
            kernel_failure,
            cleanup_failure,
            partitions: self.partitions,
            requests: self.requests,
            events: self.scheduler.signs().events().collect(),
            scan_child_signs,
            scan_cancellation_failed,
            scan_output_completion_failed,
            fore_deliveries: self.fore.into_deliveries(),
            clock_observations: self.clock_observations.into_observations(),
            clock_quality,
            clock_execution_bounds: execution_bounds,
        }
    }
}

fn bind_body_activations(
    partitions: &[BodyPlotPlan],
    fragments: &[&PlanFragment],
) -> Result<Vec<LoweredFragmentActivations>, String> {
    if partitions.len() != fragments.len() {
        return Err("Body activation binding partition count differs".into());
    }
    partitions
        .iter()
        .zip(fragments)
        .map(|(partition, fragment)| {
            if partition.plan.fragments.len() != 1
                || partition.plan.fragments[0].fragment_id != fragment.fragment_id
                || partition.plan.fragments[0].plan_id != fragment.plan_id
            {
                return Err("Body activation binding fragment differs from exact Plan".into());
            }
            lower_fragment_activations(&partition.plan, &fragment.fragment_id)
                .map_err(|error| format!("Body activation binding: {error:?}"))
        })
        .collect()
}

#[cfg(test)]
#[path = "body_kernel/activation_binding_tests.rs"]
mod activation_binding_tests;

#[cfg(test)]
#[path = "body_kernel/live_fore_tests.rs"]
mod live_fore_tests;

#[cfg(test)]
#[path = "body_kernel/todo_core_replay_tests.rs"]
mod todo_core_replay_tests;
