//! Local multi-partition composition of the existing kernel and Host effects.
use super::{
    fore_sign_storage, kernel_preparation::KernelTables, preparation, simple_face_host,
    InstalledBack, InstalledScheduler, MAX_CORDS, MAX_NODES, MAX_QUEUE_SLOTS, PENDING_REQUESTS,
};
use crate::{
    body_execution::BodyForeOutputAdapter, hosted_keyboard::HostedKeyboardAdapter,
    BodyLiveForeQueue, ExternalForeDelivery, ExternalForeInput, RunControl, TimerAdapter,
};
use conduit_body::BodyPlotPlan;
use conduit_core::{
    BodyClockCorrelation, BodyTimeQuality, BodyTimeRefusal, BodyTimeRequirement,
    CancellationReason, FailureReason, Plan, PlanFragment, TerminalDisposition,
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

mod chart_face_route;
mod creche_route;
use creche_route::CrecheRoute;
mod clock_observation;
mod data_text;
mod fore_route;
mod parsec_acknowledged_map;
mod parsec_sources;
mod parsec_terminal_owner;
mod scalar_math;
pub(crate) use parsec_terminal_owner::QualifiedParsecInstruction;
mod each_route;
mod scan_route;
use crate::body_execution::ObservedKernelEvent;
use clock_observation::KernelClockObservations;
use fore_route::BodyForeRoute;

mod diagnostic;
mod native_source;
mod preparation_body;
pub(crate) use native_source::NativeBodySource;
pub(crate) mod memory;

pub(crate) struct BodyKernel<'a> {
    terminal_events: Option<Vec<KernelEvent>>,
    error_diagnostic: Option<diagnostic::Diagnostic>,
    cleanup_diagnostic: Option<diagnostic::Diagnostic>,
    creche: Option<CrecheRoute>,
    chart_face: Option<chart_face_route::ChartFaceRoute>,
    native_source: Option<native_source::NativeSourceRoute<'a>>,
    scheduler: Box<InstalledScheduler>,
    fore: BodyForeRoute<'a>,
    activations: Vec<LoweredFragmentActivations>,
    partitions: Vec<KernelIdentityMap>,
    operations: Vec<LoweredHostCall>,
    face_labels: Vec<Option<String>>,
    input_origin: Option<super::keyboard_input_binding::KeyboardRequestAdmission>,
    typed_record_hosts: Vec<Option<super::typed_record_back::TypedRecordHost>>,
    image_text_hosts: Vec<Option<super::image_text_back::ImageTextHost>>,
    image_text_record_hosts: Vec<Option<super::image_text_record_back::ImageTextRecordHost>>,
    text_state_hosts: Vec<Option<super::text_state_back::TextStateHost>>,
    data_text_hosts: Vec<super::data_text_host::DataTextGenerationHosts<'a>>,
    input_keymaps: [Option<conduit_human::PreparedKeymapChoices>; MAX_NODES],
    requests: Vec<HostCallRequest>,
    clock_observations: KernelClockObservations,
    supported_pure_activation: bool,
    supported_parsec_input_scan: bool,
    parsec_map: Option<parsec_acknowledged_map::ParsecAcknowledgedMap>,
    parsec_terminal: Option<parsec_terminal_owner::ParsecTerminalOwner>,
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
    pub each_child_signs: Result<
        Vec<conduit_composite::ScanChildSignReceipt>,
        conduit_composite::BoundedActivationError,
    >,
    pub each_cancellation_failed: bool,
    pub each_output_completion_failed: bool,
    pub fore_deliveries: Vec<ExternalForeDelivery>,
    pub fore_pending_output: Option<crate::body_execution::BodyForePendingOutput>,
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
        conduit_std_offers::KEYMAP_HOST_CALL
            | conduit_std_offers::KEYMAP_SELECT_HOST_CALL
            | conduit_std_offers::CHORDS_HOST_CALL
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
fn face(operation: &LoweredHostCall) -> bool {
    operation.target_kind.as_ref().is_some_and(|target| {
        matches!(
            target.as_str(),
            "face/stdout-text"
                | conduit_std_offers::TICK_FACE_TARGET
                | conduit_std_offers::COUNT_FACE_TARGET
                | conduit_std_offers::BOOL_FACE_TARGET
        ) && operation.contract_id
            == conduit_core::present_host_call_requirement(
                target.clone(),
                operation.binding.maximum_input_bytes,
            )
            .contract_id
    })
}

impl<'a> BodyKernel<'a> {
    #[cfg(test)]
    pub(crate) fn prepare(
        partitions: &[BodyPlotPlan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
    ) -> Result<Self, String> {
        let plans = partitions
            .iter()
            .map(|partition| &partition.plan)
            .collect::<Vec<_>>();
        Self::prepare_with_live(
            None,
            &plans,
            has_keyboard,
            parent_play,
            fore_inputs,
            sequential_fore,
            has_fore_output,
            None,
            None,
            #[cfg(target_os = "linux")]
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_on_host(
        partitions: &[&Plan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        fore_inputs: &'a [ExternalForeInput],
        sequential_fore: bool,
        has_fore_output: bool,
        host: &conduit_core::HostAdvertisement,
        native: Option<NativeBodySource<'a, '_>>,
        #[cfg(target_os = "linux")] selected_file: Option<
            &'a crate::text_file::SelectedTextFileResidence,
        >,
    ) -> Result<Self, String> {
        let sources = parsec_sources::prepare(partitions, host)?;
        Self::prepare_with_live(
            native,
            partitions,
            has_keyboard,
            parent_play,
            fore_inputs,
            sequential_fore,
            has_fore_output,
            None,
            sources.as_ref(),
            #[cfg(target_os = "linux")]
            selected_file,
        )
    }

    pub(crate) fn prepare_live_on_host(
        partitions: &[&Plan],
        has_keyboard: bool,
        parent_play: &conduit_core::ActivePlayId,
        queue: &BodyLiveForeQueue,
        control: &RunControl,
        host: &conduit_core::HostAdvertisement,
        #[cfg(target_os = "linux")] selected_file: Option<
            &'a crate::text_file::SelectedTextFileResidence,
        >,
    ) -> Result<Self, String> {
        if partitions.len() != 1 || !queue.matches_plan(partitions[0], control) {
            return Err("live Fore queue differs from the exact Body Plan or stop control".into());
        }
        if has_keyboard {
            BodyLiveForeQueue::validate_parsec_input_plan(partitions[0], host)?;
        }
        let sources = parsec_sources::prepare(partitions, host)?;
        Self::prepare_with_live(
            None,
            partitions,
            has_keyboard,
            parent_play,
            &[],
            true,
            true,
            Some(queue),
            sources.as_ref(),
            #[cfg(target_os = "linux")]
            selected_file,
        )
    }

    /// Prepare input origin descriptors without granting issued-Play authority.
    pub(crate) fn prepare_input_origin(
        &mut self,
        advertisement: &conduit_core::HostAdvertisement,
        plan: &conduit_body::BodyPlan,
        prospective: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), String> {
        if !super::keyboard_input_binding::has_input_source(&self.operations) {
            // Ordinary non-input admission retains its own refusals and ordering.
            return Ok(());
        }
        if self.input_origin.is_some() {
            return Err("Body input origin was prepared twice".into());
        }
        self.input_origin = Some(
            super::keyboard_input_binding::KeyboardRequestAdmission::prepare_body(
                advertisement,
                plan,
                prospective,
                &self.partitions,
                &self.operations,
            )?,
        );
        if self.supported_parsec_input_scan {
            self.input_origin
                .as_mut()
                .ok_or("Parsec input origin is absent")?
                .enable_completed_event_custody()?;
            self.parsec_map = Some(parsec_acknowledged_map::ParsecAcknowledgedMap::prepare(
                plan,
                advertisement,
                prospective,
                self.partitions
                    .first()
                    .ok_or("Parsec map partition is absent")?,
            )?);
        }
        Ok(())
    }

    pub(crate) fn confirm_input_play(
        &mut self,
        advertisement: &conduit_core::HostAdvertisement,
        issued: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), String> {
        match self.input_origin.as_mut() {
            Some(origin) => origin.confirm_body_play(advertisement, issued),
            None if !super::keyboard_input_binding::has_input_source(&self.operations) => Ok(()),
            None => Err("Body input origin was not prepared".into()),
        }?;
        if let Some(map) = self.parsec_map.as_mut() {
            map.confirm(advertisement, issued)?;
        }
        Ok(())
    }

    pub(crate) fn attach_parsec_terminal(
        &mut self,
        plan: &conduit_body::BodyPlan,
        host: &conduit_core::HostAdvertisement,
        columns: usize,
        rows: usize,
    ) -> Result<(), String> {
        if !self.supported_parsec_input_scan
            || self.parsec_map.is_none()
            || self.parsec_terminal.is_some()
        {
            return Err("terminal attachment requires the exact prepared Parsec input Body".into());
        }
        self.parsec_terminal = Some(parsec_terminal_owner::ParsecTerminalOwner::prepare(
            plan, host, columns, rows,
        )?);
        Ok(())
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

    /// Exact finite pure scans and the separately validated live Parsec input
    /// profile have installed Body routes. This admits execution shape only;
    /// Face action grants and current input-map authority remain separate.
    pub(crate) fn require_supported_execution(&self) -> Result<(), String> {
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str()
                == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
        }) && self.creche.is_none()
        {
            return Err("planned Creche has no scoped admitted encounter owner".into());
        }
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str()
                == crate::measurement_chart_offer::MEASUREMENT_CHART_FACE_HOST_CALL
                || operation.contract_id.as_str()
                    == crate::measurement_chart_flow_offer::MEASUREMENT_CHART_FLOW_FACE_HOST_CALL
        }) && self.chart_face.is_none()
        {
            return Err("planned chart Face has no exact scoped committed observer route".into());
        }
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
            && !self.supported_pure_activation
            && !self.supported_parsec_input_scan
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
        mut terminal: Option<crate::body_execution::ParsecTerminalAttachment<'_>>,
        mut creche: Option<&mut crate::hosted_creche::CrecheHost<'_>>,
        mut face_observer: Option<&mut dyn crate::body_execution::BodyFaceContributionObserver>,
    ) -> BodyKernelResult {
        let mut keys = super::keyboard_input_host::KeyboardInputHost::new(
            input,
            self.operations
                .iter()
                .map(|operation| &operation.contract_id),
        );
        if let Some(admission) = self.input_origin.take() {
            // Installation can only fail for a reused or already-active host;
            // this host is freshly constructed before its first request.
            keys.install_request_admission(admission)
                .expect("fresh Body keyboard input host");
        }
        let mut deadlines = super::deadline_host::InstalledDeadlineHost::<PENDING_REQUESTS>::new();
        let mut clock_quality = admitted_clock_quality;
        let mut kernel_failure = None;
        let result = (|| -> Result<TerminalDisposition, String> {
            let mut cancelling = false;
            loop {
                if !cancelling && !control.stop_requested() {
                    self.poll_creche(creche.as_deref_mut())?;
                    self.poll_chart_face(&mut face_observer)?;
                }
                self.fore.drain(
                    &mut self.scheduler,
                    &mut fore_output,
                    self.supported_parsec_input_scan.then_some(&mut keys),
                    self.parsec_map.as_mut(),
                    &mut self.error_diagnostic,
                )?;
                if !cancelling && !control.stop_requested() {
                    if let (Some(owner), Some(map), Some(attachment)) = (
                        self.parsec_terminal.as_mut(),
                        self.parsec_map.as_ref(),
                        terminal.as_mut(),
                    ) {
                        if owner.ensure_present(map, &attachment.host, attachment.output)? {
                            let (face, show) = owner.current_face_show(map, &attachment.host)?;
                            attachment.observer.showed(face, show)?;
                        }
                    }
                }
                // Drain every source state before checking an input. An event
                // observed against an earlier Show must never be reinterpreted
                // against a newer source map merely because Fore order differs.
                while let Some(observed) = self.fore.take_acknowledged_input() {
                    if cancelling || control.stop_requested() {
                        continue;
                    }
                    if let (Some(owner), Some(map), Some(attachment)) = (
                        self.parsec_terminal.as_mut(),
                        self.parsec_map.as_ref(),
                        terminal.as_mut(),
                    ) {
                        let input_plan = keys.require_completed_event_scope(
                            &observed.event,
                            owner.body_plan(),
                            map.current_play()?,
                            &attachment.host,
                        )?;
                        if input_plan != &owner.body_plan().plots[0].plan.plan_id {
                            return Err(
                                "Parsec completed event belongs to a different input Plan".into()
                            );
                        }
                        if !owner.authorize_prompted_event(
                            map,
                            &attachment.host,
                            &keys,
                            &observed.event,
                        )? {
                            owner.sync_prompt(map, &attachment.host, &keys, attachment.output)?;
                            continue;
                        }
                        owner.admit_completed_input(
                            map,
                            &attachment.host,
                            &observed.event,
                            self.fore.acknowledged_input_bytes(&observed),
                            attachment.queue,
                        )?;
                        owner.sync_prompt(map, &attachment.host, &keys, attachment.output)?;
                    }
                }
                if !cancelling && !control.stop_requested() {
                    self.poll_chart_face(&mut face_observer)?;
                }
                self.clock_observations.capture_new(
                    self.scheduler.signs().events(),
                    clock,
                    host_id,
                    boot_id,
                );
                if !cancelling && control.requested_stop().is_some() {
                    if let Some(owner) = creche.as_deref_mut() {
                        owner.retire()?;
                    }
                    self.scheduler.cancel().map_err(|error| {
                        diagnostic::error(
                            &mut self.error_diagnostic,
                            format_args!("Body cancel: {error:?}"),
                        )
                    })?;
                    keys.cancel();
                    // No acknowledged map remains current while the owning
                    // Body Play is being cancelled.
                    if let (Some(owner), Some(attachment)) =
                        (self.parsec_terminal.as_mut(), terminal.as_mut())
                    {
                        owner.withdraw_prompt(attachment.output)?;
                    }
                    keys.set_input_view_epoch(None);
                    self.parsec_map = None;
                    self.parsec_terminal = None;
                    deadlines.clear();
                    cancelling = true;
                }
                let live_observed = self.fore.activity_generation();
                if !cancelling {
                    self.fore
                        .start_and_feed(&mut self.scheduler, &mut self.error_diagnostic)?;
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
                                ));
                            }
                        }
                    }
                }
                while let Some(cancellation) = self.scheduler.next_host_cancellation() {
                    if self.cancel_native_source(cancellation)? {
                        continue;
                    }
                    let operation = self
                        .operations
                        .iter()
                        .find(|op| op.node == cancellation.node && op.call == cancellation.call)
                        .ok_or("Body cancellation has no exact operation")?;
                    if operation.contract_id.as_str()
                        == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
                    {
                        self.cancel_creche_call(cancellation, creche.as_deref_mut())?;
                    } else if keyboard(&operation.contract_id) || button(&operation.contract_id) {
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
                        deadlines
                            .cancel(cancellation, &mut self.scheduler)
                            .map_err(|error| {
                                diagnostic::error(
                                    &mut self.error_diagnostic,
                                    format_args!("{error}"),
                                )
                            })?;
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
                            return Err(diagnostic::error(
                                &mut self.error_diagnostic,
                                format_args!("Body observed-operation capacity exceeded"),
                            ));
                        }
                        self.requests.push(request);
                    }
                    if self.chart_face_call(request)? {
                        continue;
                    }
                    if self.begin_native_source(request)? {
                        continue;
                    }
                    let operation = self
                        .operations
                        .iter()
                        .find(|op| op.node == request.node && op.call == request.call)
                        .ok_or_else(|| {
                            diagnostic::error(
                                &mut self.error_diagnostic,
                                format_args!("Body request has no exact partition operation"),
                            )
                        })?;
                    if operation.contract_id.as_str()
                        == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
                    {
                        self.begin_creche(request, creche.as_deref_mut())?;
                        continue;
                    }
                    if super::measurement_chart_constructor_back::complete(
                        operation,
                        request,
                        &mut self.scheduler,
                    )? {
                        continue;
                    }
                    if super::measurement_plot_back::complete(
                        operation,
                        request,
                        &mut self.scheduler,
                    )? {
                        continue;
                    }
                    if scalar_math::complete(operation, request, &mut self.scheduler)? {
                        continue;
                    }
                    let input =
                        self.scheduler
                            .host_value(request.input.value)
                            .map_err(|error| {
                                diagnostic::error(
                                    &mut self.error_diagnostic,
                                    format_args!("Body request value: {error:?}"),
                                )
                            })?;
                    if data_text::is_file(&operation.contract_id) {
                        let source = self
                            .data_text_hosts
                            .iter_mut()
                            .find(|host| host.contains_node(request.node))
                            .ok_or("Body file Text operation has no prepared selected provider")?;
                        let direction =
                            super::data_text_host::operation_for_contract(&operation.contract_id)
                                .ok_or("Body file Text operation is unknown")?;
                        let completion = source.execute(request.node, direction, input);
                        super::data_text_host::complete(
                            &mut self.scheduler,
                            operation,
                            request,
                            completion,
                        )?;
                        continue;
                    }
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
                                    .map_err(|error| format!("Body text state output: {error:?}"))?
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
                            Err(refusal) => (HostCallDisposition::Failed, None, Some(refusal)),
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
                        let completion = if operation.contract_id.as_str()
                            == conduit_std_offers::KEYMAP_SELECT_HOST_CALL
                        {
                            super::input_semantic_backs::select_host(
                                &mut self.input_keymaps[node],
                                input,
                            )
                        } else {
                            super::input_semantic_backs::execute_host(
                                operation.contract_id.as_str()
                                    == conduit_std_offers::KEYMAP_HOST_CALL,
                                &mut self.input_keymaps[node],
                                input,
                            )
                        };
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
                        let duration = conduit_time::decode_tick(input).map_err(|error| {
                            diagnostic::error(&mut self.error_diagnostic, format_args!("{error}"))
                        })?;
                        let now = clock.monotonic_now_ms().ok_or_else(|| {
                            diagnostic::error(
                                &mut self.error_diagnostic,
                                format_args!("admitted monotonic wait Base is unavailable"),
                            )
                        })?;
                        deadlines.arm(request, duration, now).map_err(|error| {
                            diagnostic::error(&mut self.error_diagnostic, format_args!("{error}"))
                        })?;
                        continue;
                    } else if !simple_face_host::present_named(
                        operation.target_kind.as_ref(),
                        input,
                        output,
                        self.face_labels[usize::from(request.node.0)].as_deref(),
                    )? {
                        return Err("Body face contract became unsupported".into());
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
                        .map_err(|error| {
                            diagnostic::error(
                                &mut self.error_diagnostic,
                                format_args!("Body Host completion: {error:?}"),
                            )
                        })?;
                }
                let status = self.scheduler.step().map_err(|error| {
                    if let conduit_kernel::scheduler::SchedulerError::BackFailed(failure) = error {
                        kernel_failure = Some(failure);
                    }
                    diagnostic::error(
                        &mut self.error_diagnostic,
                        format_args!("Body kernel: {error:?}"),
                    )
                })?;
                self.clock_observations.capture_new(
                    self.scheduler.signs().events(),
                    clock,
                    host_id,
                    boot_id,
                );
                // Snapshot the privately owned, acknowledged Show immediately
                // before any adapter poll. This is observation metadata only;
                // later action admission must still recheck exact current custody.
                let input_view_epoch = if !cancelling && !control.stop_requested() {
                    match (
                        self.parsec_terminal.as_mut(),
                        self.parsec_map.as_ref(),
                        terminal.as_mut(),
                    ) {
                        (Some(owner), Some(map), Some(attachment)) => {
                            if owner.sync_prompt(map, &attachment.host, &keys, attachment.output)? {
                                if let Some(observation) =
                                    owner.instruction_observation(map, &attachment.host, &keys)?
                                {
                                    attachment.observer.instruction_observed(&observation)?;
                                }
                            }
                            owner.prompt_input_view(map, &attachment.host, &keys)?
                        }
                        _ => None,
                    }
                } else {
                    if let (Some(owner), Some(attachment)) =
                        (self.parsec_terminal.as_mut(), terminal.as_mut())
                    {
                        owner.withdraw_prompt(attachment.output)?;
                    }
                    None
                };
                keys.set_input_view_epoch(input_view_epoch);
                match status {
                    SchedulerStatus::Drained => {
                        if self.chart_face_pending() {
                            std::thread::park_timeout(std::time::Duration::from_millis(1));
                            continue;
                        }
                        self.fore
                            .require_normal_terminal(&self.scheduler, &mut self.error_diagnostic)?;
                        return Ok(TerminalDisposition::Completed);
                    }
                    SchedulerStatus::Cancelled => {
                        return Ok(TerminalDisposition::Cancelled {
                            reason: CancellationReason::OperatorRequested,
                        });
                    }
                    SchedulerStatus::Progress { .. } => {
                        if keys.is_pending() {
                            keys.poll(&mut self.scheduler)?;
                        }
                    }
                    SchedulerStatus::Idle => {
                        if keys.poll(&mut self.scheduler)?
                            || deadlines
                                .complete_next(&mut self.scheduler, clock)
                                .map_err(|error| {
                                    diagnostic::error(
                                        &mut self.error_diagnostic,
                                        format_args!("{error}"),
                                    )
                                })?
                        {
                            continue;
                        }
                        // A pending admitted keyboard request owns progress even
                        // when no command has reached the live Fore queue yet.
                        if !keys.is_pending()
                            && !self.creche_pending()
                            && !self.chart_face_pending()
                        {
                            if let Some(observed) = live_observed {
                                if self.fore.wait_for_activity(observed)? {
                                    continue;
                                }
                            }
                        }
                        if !keys.is_pending()
                            && deadlines.is_empty()
                            && !self.creche_pending()
                            && !self.chart_face_pending()
                        {
                            return Err(diagnostic::error(
                                &mut self.error_diagnostic,
                                format_args!("Body kernel has no admitted progress source"),
                            ));
                        }
                        if self.creche_pending() {
                            if let Some(owner) = creche.as_deref() {
                                std::thread::park_timeout(
                                    owner.poll_wait().min(std::time::Duration::from_millis(10)),
                                );
                            }
                        }
                        if self.chart_face_pending() {
                            std::thread::park_timeout(std::time::Duration::from_millis(1));
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
        // Revoke the prompt on every terminal outcome, including failures
        // before the next adapter poll. A withdrawal failure is itself a failure.
        let withdrawal = match (self.parsec_terminal.as_mut(), terminal.as_mut()) {
            (Some(owner), Some(attachment)) => owner.withdraw_prompt(attachment.output).map(|_| ()),
            _ => Ok(()),
        };
        keys.set_input_view_epoch(None);
        let result = result.and_then(|outcome| withdrawal.map(|()| outcome));
        let retirement = match creche {
            Some(owner) => owner.retire(),
            None => Ok(()),
        };
        let mut cleanup_failure = retirement
            .as_ref()
            .err()
            .map(|e| format!("Creche owner cleanup: {e}"));
        let result = result.and_then(|outcome| {
            retirement
                .as_ref()
                .map(|()| outcome)
                .map_err(|e| format!("Creche owner retirement: {e}"))
        });
        let (terminal, failure) = match result {
            Ok(terminal) => (terminal, None),
            Err(error) => {
                let error = diagnostic::retained_error(&mut self.error_diagnostic, error);
                keys.cancel();
                deadlines.clear();
                if retirement.is_ok() {
                    let failure = self.scheduler.cancel().err().map(|error| {
                        diagnostic::error(
                            &mut self.cleanup_diagnostic,
                            format_args!("Body failure cleanup: {error:?}"),
                        )
                    });
                    if cleanup_failure.is_none() {
                        cleanup_failure = failure;
                    }
                }
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
        // Face snapshots run after the sealed Play. Preserve child
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
        let each_child_signs = self
            .scheduler
            .drivers()
            .iter()
            .filter_map(|driver| match driver {
                InstalledBack::BodyEach(each) => Some(each.as_ref()),
                _ => None,
            })
            .try_fold(Vec::new(), |mut receipts, each| {
                receipts.extend(each.child_sign_receipts()?);
                Ok(receipts)
            });
        let each_cancellation_failed = self.scheduler.drivers().iter().any(
            |driver| matches!(driver,InstalledBack::BodyEach(each) if each.cancellation_failed()),
        );
        let each_output_completion_failed = self.scheduler.drivers().iter().any(|driver| matches!(driver,InstalledBack::BodyEach(each) if each.output_completion_failed()));
        let (fore_deliveries, fore_pending_output) = self.fore.into_custody();
        BodyKernelResult {
            terminal,
            failure,
            kernel_failure,
            cleanup_failure,
            partitions: self.partitions,
            requests: self.requests,
            events: if let Some(mut prepared) = self.terminal_events.take() {
                prepared.extend(self.scheduler.signs().events());
                prepared
            } else {
                self.scheduler.signs().events().collect()
            },
            scan_child_signs,
            scan_cancellation_failed,
            scan_output_completion_failed,
            each_child_signs,
            each_cancellation_failed,
            each_output_completion_failed,
            fore_deliveries,
            fore_pending_output,
            clock_observations: self.clock_observations.into_observations(),
            clock_quality,
            clock_execution_bounds: execution_bounds,
        }
    }
}

fn bind_body_activations(
    partitions: &[&Plan],
    fragments: &[&PlanFragment],
) -> Result<Vec<LoweredFragmentActivations>, String> {
    if partitions.len() != fragments.len() {
        return Err("Body activation binding partition count differs".into());
    }
    partitions
        .iter()
        .zip(fragments)
        .map(|(partition, fragment)| {
            if partition.fragments.len() != 1
                || partition.fragments[0].fragment_id != fragment.fragment_id
                || partition.fragments[0].plan_id != fragment.plan_id
            {
                return Err("Body activation binding fragment differs from exact Plan".into());
            }
            lower_fragment_activations(partition, &fragment.fragment_id)
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

#[cfg(test)]
#[path = "body_kernel/thermostat_live_fore_tests.rs"]
mod thermostat_live_fore_tests;

#[cfg(test)]
#[path = "body_kernel/parsec_live_fore_tests.rs"]
mod parsec_live_fore_tests;

#[cfg(test)]
#[path = "body_kernel/parsec_instruction_source_tests.rs"]
mod parsec_instruction_source_tests;
