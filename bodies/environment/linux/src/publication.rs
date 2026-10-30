//! One checked/lowered publication using the production kernel and real files.
use conduit_core as c;
use conduit_environment_runtime::recording::{
    PreparedRecordingGeneration, PreparedRecordingSnapshot, RecordingCommitObserver,
    RecordingSnapshotBack, RecordingSnapshotRefusal,
};
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
use conduit_linux_snapshot_store::{
    FileOperationRefusal, PreparedFileSnapshotOperation, SnapshotDirectory,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredForePort, LoweredPlanFragment};

type Kernel = FixedScheduler<
    RecordingSnapshotBack,
    k::FixedValueStore<3, 4096>,
    k::HostedSignLog,
    1,
    2,
    1,
    2,
    1,
    1,
    1,
    1,
>;
#[derive(Debug)]
pub enum FilePublicationRefusal {
    Scope,
    Shape,
    Selection,
    Content(RecordingSnapshotRefusal),
    File(FileOperationRefusal),
    Kernel(SchedulerError),
}
pub struct FilePublicationCompletion {
    pub disposition: k::HostCallDisposition,
    /// Exact failure completed into the original kernel request, if any.
    pub failure: Option<k::Failure>,
    pub refusal: Option<FileOperationRefusal>,
}
/// Root retains this owner and borrows each current Play from its actual Body.
/// Preparation validates identities; that does not create lifecycle authority.
pub struct PreparedFileRecordingPublication<'a> {
    active: c::ActivePlayIdentity,
    kernel: Box<Kernel>,
    files: PreparedFileSnapshotOperation<'a>,
    encoder: PreparedRecordingSnapshot,
    observer: RecordingCommitObserver,
    input: LoweredForePort,
    output: LoweredForePort,
    admitted: bool,
    accepting: bool,
    cancelled: bool,
    delivered: [u8; 512],
}
impl<'a> PreparedFileRecordingPublication<'a> {
    /// All table/file preparation precedes transfer of the initialized generation.
    /// Root separately admits the complete memory/disk closure and directory's
    /// residence mapping, live selected providers and independent grants.
    pub fn prepare(
        directory: &'a SnapshotDirectory,
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &c::ActivePlayIdentity,
        generation: &mut Option<PreparedRecordingGeneration>,
    ) -> Result<Self, FilePublicationRefusal> {
        Self::prepare_generation(directory, fragment, lowered, active, generation, false)
    }
    /// Prepare all kernel/file backing before producer publications arrive.
    /// The generation must be empty. This creates no producer authentication,
    /// Body start, storage effect or durable acknowledgement.
    pub fn prepare_streaming(
        directory: &'a SnapshotDirectory,
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &c::ActivePlayIdentity,
        generation: &mut Option<PreparedRecordingGeneration>,
    ) -> Result<Self, FilePublicationRefusal> {
        Self::prepare_generation(directory, fragment, lowered, active, generation, true)
    }
    fn prepare_generation(
        directory: &'a SnapshotDirectory,
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &c::ActivePlayIdentity,
        generation: &mut Option<PreparedRecordingGeneration>,
        streaming: bool,
    ) -> Result<Self, FilePublicationRefusal> {
        use FilePublicationRefusal as E;
        if !c::verify_plan_fragment(fragment)
            || active.plan_id != fragment.plan_id
            || active.host_id != fragment.host_id
            || active.boot_id != fragment.boot_id
            || *active
                != c::bind_active_play(
                    &active.plan_id,
                    &active.host_id,
                    &active.boot_id,
                    active.play_sequence,
                )
        {
            return Err(E::Scope);
        }
        if lower_plan_fragment(fragment).map_err(|_| E::Shape)? != *lowered
            || lowered.nodes.len() != 1
            || lowered.node_specs.len() != 1
            || lowered.cords.len() != 2
            || lowered.routes.len() != 1
            || lowered.routes[0].targets.len() != 1
            || lowered.fore_ports.len() != 2
            || lowered.host_calls.len() != 1
            || lowered.resources.len() != 1
            || !lowered.remote_endpoints.is_empty()
            || !lowered.states.is_empty()
            || !lowered.fusions.is_empty()
            || !lowered.resource_cords.is_empty()
            || !lowered.shared_pools.is_empty()
            || !fragment.execution_regions.is_empty()
            || lowered.cord_value_slots > 3
            || lowered.cord_value_bytes > 3 * 4096
            || lowered.sign_items > 128
            || lowered.sign_bytes > (128 * core::mem::size_of::<k::KernelEvent>()) as u32
            || lowered.node_specs[0].input_cords[1..]
                .iter()
                .any(Option::is_some)
            || !lowered.nodes[0].terminal_transductions.is_empty()
        {
            return Err(E::Shape);
        }
        let placement = fragment.placements.first().ok_or(E::Shape)?;
        if placement.execution_profile_id.as_str() != crate::RECORDING_PUBLICATION_PROFILE
            || placement.implementation_id.as_str() != crate::RECORDING_PUBLICATION_IMPLEMENTATION
            || placement.artifact_id.as_str() != crate::RECORDING_PUBLICATION_ARTIFACT
        {
            return Err(E::Selection);
        }
        let call = &lowered.host_calls[0];
        if fragment.placements.len() != 1
            || lowered.nodes[0].node != k::NodeId(0)
            || lowered.nodes[0].placement_id != placement.placement_id
            || placement.kind_id.as_str() != conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND
            || call.node != k::NodeId(0)
            || call.call != k::HostCallId(0)
            || call.contract_id.as_str() != conduit_linux_snapshot_store::PUBLISH_OPERATION
            || call.maximum_in_flight != 1
            || call.binding.maximum_input_bytes != 4096
            || call.binding.maximum_output_bytes != 512
            || lowered.resources[0].node != k::NodeId(0)
        {
            return Err(E::Shape);
        }
        let fore = |direction, name, kind, bytes| {
            lowered
                .fore_ports
                .iter()
                .find(|port| {
                    port.direction == direction
                        && port.front_port_id.as_str() == name
                        && port.value_kind.as_str() == kind
                        && port.track == c::ConnectionTrack::Payload
                        && port.temporal == c::PortTemporal::Value
                        && port.item_capacity == 1
                        && port.byte_capacity == bytes
                        && port.selected_line.is_none()
                })
                .cloned()
                .ok_or(E::Shape)
        };
        let input = fore(c::PortDirection::Input, "chunk", "text/json-utf8@1", 4096)?;
        let output = fore(
            c::PortDirection::Output,
            "published",
            c::RESOURCE_REFERENCE_INFO_ID,
            512,
        )?;
        let selected =
            conduit_linux_snapshot_store::snapshot_reference(placement).map_err(|_| E::Shape)?;
        let ready = generation.as_mut().ok_or(E::Shape)?;
        let observer = ready.publication.commit_observer();
        if observer.committed().is_some()
            || observer.encoded_reference() != selected.encode().map_err(|_| E::Shape)?
            || ready.encoder.expected_reference() != observer.encoded_reference()
        {
            return Err(E::Shape);
        }
        if streaming {
            if !ready.encoder.is_empty() {
                return Err(E::Shape);
            }
        } else {
            let content = ready.encoder.seal().map_err(E::Content)?;
            input
                .validate_value(content.as_bytes())
                .map_err(|_| E::Shape)?;
        }
        let files = PreparedFileSnapshotOperation::prepare(directory, placement, k::NodeId(0))
            .map_err(E::File)?;
        let mut routes = k::FixedRoutes::<1, 1>::new(1);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|_| E::Shape)?;
        }
        routes.seal().map_err(|_| E::Shape)?;
        let mut calls = k::FixedHostCallBindings::<1>::new(1);
        calls
            .install(call.node, call.binding)
            .map_err(|_| E::Shape)?;
        calls.seal().map_err(|_| E::Shape)?;
        let values = k::FixedValueStore::<3, 4096>::new(3 * 4096).map_err(|_| E::Shape)?;
        // One sealed input, one output and one route: input admit/close (2),
        // output offer/accept/delivery (3), output close (1). Failed publication
        // is a prefix; cancellation adds ordinary signs only. Required evidence
        // remains retained: no eviction, acknowledgement or reset is introduced.
        let signs = k::HostedSignLog::new_with_remote_storage(
            128,
            (128 * core::mem::size_of::<k::KernelEvent>()) as u32,
            6,
            k::remote_sign_storage_bytes(6).ok_or(E::Shape)?,
        )
        .map_err(|_| E::Shape)?;
        let ready = generation.take().ok_or(E::Shape)?;
        let kernel = Kernel::new_with_host_calls(
            [NodeSpec {
                input_cords: [lowered.node_specs[0].input_cords[0]],
                maximum_step_fuel: lowered.node_specs[0].maximum_step_fuel,
            }],
            core::array::from_fn(|i| lowered.cords[i].spec),
            routes,
            calls,
            [ready.publication],
            values,
            signs,
        )
        .map_err(E::Kernel)?;
        Ok(Self {
            active: active.clone(),
            kernel: Box::new(kernel),
            files,
            encoder: ready.encoder,
            observer,
            input,
            output,
            admitted: false,
            accepting: streaming,
            cancelled: false,
            delivered: [0; 512],
        })
    }
    fn current(&self, active: &c::ActivePlayIdentity) -> Result<(), FilePublicationRefusal> {
        if active == &self.active {
            Ok(())
        } else {
            Err(FilePublicationRefusal::Scope)
        }
    }
    /// Append a committed source fact to the preallocated generation. Root
    /// supplies its authenticated producer context at preparation and the actual
    /// sample time in that producer Boot domain, never recorder receipt time.
    /// Acceptance here is staging only; inspect the ordinary commit observer
    /// after admission and storage completion for durable publication evidence.
    pub fn append_publication(
        &mut self,
        active: &c::ActivePlayIdentity,
        input: &mut conduit_environment_runtime::recording::PreparedRecordingInput,
        snapshot: conduit_environment_runtime::publication::PublicationSnapshot<'_>,
        sampled_at_boot_ms: u64,
    ) -> Result<conduit_environment_runtime::recording::ChunkAppend, FilePublicationRefusal> {
        self.current(active)?;
        if !self.accepting {
            return Err(FilePublicationRefusal::Shape);
        }
        self.encoder
            .append_publication(input, snapshot, sampled_at_boot_ms)
            .map_err(FilePublicationRefusal::Content)
    }
    /// Admit exactly the retained, sealed chunk once through the checked Fore.
    /// A Full result advances neither ingress sequence nor this custody marker.
    pub fn admit(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<RemoteIngressOutcome, FilePublicationRefusal> {
        self.current(active)?;
        if self.admitted || self.cancelled {
            return Err(FilePublicationRefusal::Shape);
        }
        let bytes = self
            .encoder
            .seal()
            .map_err(FilePublicationRefusal::Content)?
            .as_bytes();
        self.input
            .validate_value(bytes)
            .map_err(|_| FilePublicationRefusal::Shape)?;
        self.accepting = false;
        let outcome = self
            .kernel
            .admit_remote_input(self.input.endpoint, self.input.cord, 0, bytes)
            .map_err(FilePublicationRefusal::Kernel)?;
        if matches!(outcome, RemoteIngressOutcome::Accepted { .. }) {
            self.admitted = true;
            self.kernel
                .close_remote_input(self.input.endpoint, self.input.cord)
                .map_err(FilePublicationRefusal::Kernel)?;
        }
        Ok(outcome)
    }
    pub fn step(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<SchedulerStatus, FilePublicationRefusal> {
        self.current(active)?;
        self.kernel.step().map_err(FilePublicationRefusal::Kernel)
    }
    /// Service at most one original request. File completion does not update
    /// the observer until the ordinary Back's completion Step commits.
    pub fn service_next(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<Option<FilePublicationCompletion>, FilePublicationRefusal> {
        use FilePublicationRefusal as E;
        self.current(active)?;
        let Some(request) = self.kernel.next_host_request() else {
            return Ok(None);
        };
        let bytes = self
            .kernel
            .values()
            .get(request.input.value)
            .map_err(|_| E::Shape)?;
        let mut refusal = None;
        let outcome = match self.files.perform(request, bytes) {
            Ok(bytes) => match self.kernel.store_host_value(bytes) {
                Ok(value) => k::HostCallOutcome {
                    disposition: k::HostCallDisposition::Completed,
                    output: Some(
                        k::BoundedValueRef::new(value, self.files.output_bound())
                            .map_err(|_| E::Shape)?,
                    ),
                    failure: None,
                },
                Err(_) => failed(k::Failure {
                    code: k::FailureCode::HostCallFailed,
                    detail: 217,
                }),
            },
            Err(error) => {
                let failure = match &error {
                    FileOperationRefusal::Operation(failure) => *failure,
                    _ => k::Failure {
                        code: k::FailureCode::HostCallFailed,
                        detail: 216,
                    },
                };
                refusal = Some(error);
                failed(failure)
            }
        };
        let disposition = outcome.disposition;
        let failure = outcome.failure;
        let output = outcome.output.map(|value| value.value);
        let completed = self
            .kernel
            .complete_host_call(request.node, request.request, outcome);
        self.files.finish(request).map_err(E::File)?;
        if completed.is_err() {
            if let Some(value) = output {
                self.kernel
                    .discard_host_value(value)
                    .map_err(|_| E::Shape)?;
            }
        }
        completed.map_err(E::Kernel)?;
        Ok(Some(FilePublicationCompletion {
            disposition,
            failure,
            refusal,
        }))
    }
    /// Consume only the local Root Fore. This does not report a network Line or
    /// peer delivery. Bytes are copied into admitted storage before release.
    pub fn take_reference(
        &mut self,
        active: &c::ActivePlayIdentity,
    ) -> Result<Option<&[u8]>, FilePublicationRefusal> {
        use FilePublicationRefusal as E;
        self.current(active)?;
        let Some(offer) = self
            .kernel
            .remote_egress_offer(self.output.endpoint, self.output.cord)
            .map_err(E::Kernel)?
        else {
            return Ok(None);
        };
        let bytes = self
            .kernel
            .values()
            .get(offer.value)
            .map_err(|_| E::Shape)?;
        self.output.validate_value(bytes).map_err(|_| E::Shape)?;
        if c::BoundedResourceRef::validate_encoded(bytes).is_err()
            || bytes != self.observer.encoded_reference()
        {
            return Err(E::Shape);
        }
        let len = bytes.len();
        self.delivered[..len].copy_from_slice(bytes);
        self.kernel
            .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
            .map_err(E::Kernel)?;
        self.kernel
            .remote_egress_delivered(offer.endpoint, offer.cord, offer.sequence)
            .map_err(E::Kernel)?;
        Ok(Some(&self.delivered[..len]))
    }
    pub fn observer(&self) -> &RecordingCommitObserver {
        &self.observer
    }
    pub fn cancel(
        &mut self,
        reserved: &c::ActivePlayIdentity,
    ) -> Result<(), FilePublicationRefusal> {
        self.current(reserved)?;
        self.accepting = false;
        self.cancelled = true;
        self.kernel.cancel().map_err(FilePublicationRefusal::Kernel)
    }
}
fn failed(failure: k::Failure) -> k::HostCallOutcome {
    k::HostCallOutcome {
        disposition: k::HostCallDisposition::Failed,
        output: None,
        failure: Some(failure),
    }
}
