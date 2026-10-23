use conduit_browser_runtime::resource_snapshot::{
    PreparedSnapshotOperation, SnapshotStorageRequest,
};
use conduit_core as c;
use conduit_environment_runtime::recording::RecordingBatchKernel;
use conduit_kernel::{self as k, scheduler::HostCallRequest, ValueStorage};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

#[derive(Debug, PartialEq, Eq)]
pub enum BrowserReadRefusal {
    Identity,
    Shape,
    Operation(k::Failure),
    Kernel,
}
/// The existing browser storage adapter compares every field against its pending
/// scope before executing an effect. Numeric sequence is the kernel RequestId.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RecordingReadScope<'a> {
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub active_play_id: &'a str,
    pub placement_id: &'a str,
    pub request_sequence: u32,
}
#[derive(serde::Serialize)]
pub struct RecordingReadEffect<'a> {
    pub schema: &'static str,
    #[serde(flatten)]
    pub scope: RecordingReadScope<'a>,
    #[serde(flatten)]
    pub storage: SnapshotStorageRequest<'a>,
}
struct Read {
    node: k::NodeId,
    placement: c::PlacementId,
    operation: PreparedSnapshotOperation,
}
pub struct PreparedRecordingBrowserReads {
    active: c::ActivePlayIdentity,
    reads: Vec<Read>,
}
impl PreparedRecordingBrowserReads {
    /// Bind only existing selected reads. Identity consistency does not grant
    /// storage access or prove that Root has started the corresponding Body.
    pub fn prepare(
        fragment: &c::PlanFragment,
        lowered: &LoweredPlanFragment,
        nodes: &[k::NodeId],
        active: &c::ActivePlayIdentity,
    ) -> Result<Self, BrowserReadRefusal> {
        use BrowserReadRefusal as E;
        if !c::verify_plan_fragment(fragment)
            || active.plan_id != fragment.plan_id
            || active.host_id != fragment.host_id
            || active.boot_id != fragment.boot_id
            || *active
                != c::bind_active_play(
                    &fragment.plan_id,
                    &fragment.host_id,
                    &fragment.boot_id,
                    active.play_sequence,
                )
        {
            return Err(E::Identity);
        }
        if lower_plan_fragment(fragment).map_err(|_| E::Shape)? != *lowered
            || [
                active.host_id.as_str(),
                active.boot_id.as_str(),
                active.active_play_id.as_str(),
            ]
            .iter()
            .any(|value| value.is_empty() || value.len() > 256)
            || nodes.is_empty()
            || nodes.len() > 16
            || lowered.host_calls.len() != nodes.len()
        {
            return Err(E::Shape);
        }
        let mut reads: Vec<Read> = Vec::with_capacity(nodes.len());
        for node in nodes {
            if reads.iter().any(|read| read.node == *node) {
                return Err(E::Shape);
            }
            let lowered_node = lowered
                .nodes
                .iter()
                .find(|entry| entry.node == *node)
                .ok_or(E::Shape)?;
            let placement = fragment
                .placements
                .iter()
                .find(|entry| entry.placement_id == lowered_node.placement_id)
                .ok_or(E::Shape)?;
            let operation =
                PreparedSnapshotOperation::prepare(placement, *node).map_err(|_| E::Shape)?;
            if placement.placement_id.as_str().is_empty()
                || placement.placement_id.as_str().len() > 256
                || operation.is_publish()
            {
                return Err(E::Shape);
            }
            reads.push(Read {
                node: *node,
                placement: placement.placement_id.clone(),
                operation,
            });
        }
        Ok(Self {
            active: active.clone(),
            reads,
        })
    }
    fn index(&self, request: HostCallRequest) -> Result<usize, BrowserReadRefusal> {
        self.reads
            .iter()
            .position(|read| read.node == request.node)
            .ok_or(BrowserReadRefusal::Shape)
    }
    fn scope(&self, index: usize, request: HostCallRequest) -> RecordingReadScope<'_> {
        RecordingReadScope {
            host_id: self.active.host_id.as_str(),
            boot_id: self.active.boot_id.as_str(),
            active_play_id: self.active.active_play_id.as_str(),
            placement_id: self.reads[index].placement.as_str(),
            request_sequence: request.request.0,
        }
    }
    /// Caller supplies the original request polled from this exact kernel.
    /// No scheduler, request counter or storage executor is introduced here.
    pub fn begin<const N: usize, const C: usize>(
        &mut self,
        kernel: &RecordingBatchKernel<N, C>,
        request: HostCallRequest,
    ) -> Result<RecordingReadEffect<'_>, BrowserReadRefusal> {
        let index = self.index(request)?;
        let input = kernel
            .values()
            .get(request.input.value)
            .map_err(|_| BrowserReadRefusal::Kernel)?;
        self.reads[index]
            .operation
            .begin(request, input)
            .map_err(BrowserReadRefusal::Operation)?;
        Ok(RecordingReadEffect {
            schema: "conduit.browser/resource-effect@1",
            scope: self.scope(index, request),
            storage: self.reads[index]
                .operation
                .describe(request)
                .map_err(BrowserReadRefusal::Operation)?,
        })
    }
    /// Validate full pending request and external scope before touching kernel
    /// state. Validated bytes complete that original request; history changes only
    /// when the ordinary kernel subsequently advances and commits the sink.
    pub fn complete<const N: usize, const C: usize>(
        &mut self,
        kernel: &mut RecordingBatchKernel<N, C>,
        request: HostCallRequest,
        scope: RecordingReadScope<'_>,
        result: Result<Option<&[u8]>, k::Failure>,
    ) -> Result<k::HostCallDisposition, BrowserReadRefusal> {
        use BrowserReadRefusal as E;
        let index = self.index(request)?;
        self.reads[index]
            .operation
            .describe(request)
            .map_err(E::Operation)?;
        if scope != self.scope(index, request) {
            return Err(E::Identity);
        }
        let result = self.reads[index].operation.completion(request, result);
        let outcome = match result {
            Ok(bytes) => match kernel.store_host_value(bytes) {
                Ok(value) => k::HostCallOutcome {
                    disposition: k::HostCallDisposition::Completed,
                    output: Some(k::BoundedValueRef::new(value, 4096).map_err(|_| E::Kernel)?),
                    failure: None,
                },
                Err(_) => failed(k::Failure {
                    code: k::FailureCode::HostCallFailed,
                    detail: 214,
                }),
            },
            Err(failure) => failed(failure),
        };
        let disposition = outcome.disposition;
        let output = outcome.output.map(|value| value.value);
        let completion = kernel.complete_host_call(request.node, request.request, outcome);
        self.reads[index]
            .operation
            .finish(request)
            .map_err(E::Operation)?;
        if completion.is_err() {
            if let Some(value) = output {
                kernel.discard_host_value(value).map_err(|_| E::Kernel)?;
            }
        }
        completion.map_err(|_| E::Kernel)?;
        Ok(disposition)
    }
}
fn failed(failure: k::Failure) -> k::HostCallOutcome {
    k::HostCallOutcome {
        disposition: k::HostCallDisposition::Failed,
        output: None,
        failure: Some(failure),
    }
}
