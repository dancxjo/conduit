//! Cooperative actual Std local-Source execution for the shared Language cursor.
//! Child Plays/ACKs are evidence of execution; Language retains semantic authority.
use conduit_composite::{
    KernelCompositeDefinition, KernelCompositeError, KernelCompositeHost,
    KernelCompositeSignStorage, KernelCompositeStatus, KernelOperationRegistry,
};
use conduit_core::{CapabilityOffer, HostAdvertisement};
use conduit_language::analysis_epoch_owner::fallback_round::{
    cursor::*, PreparedFallbackRoundPrograms,
};
use conduit_plot::CheckedSyntaxDocument;
#[path = "analysis_round_driver_plan.rs"]
mod plan;
#[path = "analysis_round_driver_preparation.rs"]
mod preparation;
#[path = "analysis_round_driver_source.rs"]
mod source;
pub use preparation::*;
pub use source::SourceExecution;
use source::*;
const TRANSPORT: usize = 65536;
const MAX_RECORDS: usize = 1024;
const MAX_ARENA: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriverRefusal {
    Preparation,
    PreparationDetail { stage: &'static str, detail: String },
    Pressure,
    Closed,
    ForeignCursor,
    Kernel,
    Sequence,
    UnsupportedOperation,
    UnexpectedHostCall,
    SourceTransport,
    Cursor(CursorRefusal),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriverProgress {
    Progress,
    WaitingSource,
    WaitingNumerical,
    Complete,
}
#[derive(Clone, Copy, Debug)]
pub struct DriverBounds {
    pub observations: usize,
    pub evidence_bytes: usize,
    pub signs: KernelCompositeSignStorage,
}
impl Default for DriverBounds {
    fn default() -> Self {
        Self {
            observations: MAX_RECORDS,
            evidence_bytes: MAX_ARENA,
            signs: KernelCompositeSignStorage {
                additional_local_items: 4096,
                additional_remote_items: 4096,
            },
        }
    }
}
/// Preparation-only offer inventory. The caller must advertise these exact offers.
pub fn required_round_source_offers(
    programs: &PreparedFallbackRoundPrograms,
) -> Result<Vec<CapabilityOffer>, DriverRefusal> {
    let names: Vec<_> = programs.programs().iter().map(|entry| entry.0).collect();
    plan::offers(programs.checked_document(), &names)
}
/// One private owner for each actual Source kernel and its outstanding lease.
/// Terminal failures retain all pending custody and actual error objects.
pub struct PreparedAnalysisRoundDriver {
    document: CheckedSyntaxDocument,
    operations: Vec<Operation>,
    records: Vec<Record>,
    arena: Vec<u8>,
    bounds: DriverBounds,
    binding: Option<CursorInstanceHandle>,
    pending: Option<Pending>,
    unqueued: Option<CursorSourceLease>,
    failure: Option<DriverRefusal>,
    closed: bool,
}
impl PreparedAnalysisRoundDriver {
    pub fn prepare(
        programs: &PreparedFallbackRoundPrograms,
        host: &HostAdvertisement,
        bounds: DriverBounds,
    ) -> Result<Self, DriverRefusal> {
        if bounds.observations == 0
            || bounds.observations > MAX_RECORDS
            || bounds.evidence_bytes < TRANSPORT * 2
            || bounds.evidence_bytes > MAX_ARENA
        {
            return Err(DriverRefusal::Preparation);
        }
        for offer in required_round_source_offers(programs)? {
            if !host.capabilities.contains(&offer) {
                return Err(DriverRefusal::Preparation);
            }
        }
        let mut operations = Vec::new();
        for (name, _) in programs.programs() {
            operations.push(Operation::prepare(
                name,
                plan::definition(programs.checked_document(), host, name)?,
                bounds.signs,
            )?);
        }
        Ok(Self {
            document: programs.checked_document().clone(),
            operations,
            records: Vec::with_capacity(bounds.observations),
            arena: Vec::with_capacity(bounds.evidence_bytes),
            bounds,
            binding: None,
            pending: None,
            unqueued: None,
            failure: None,
            closed: false,
        })
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn checked_document(&self) -> &CheckedSyntaxDocument {
        &self.document
    }
    pub fn failure(&self) -> Option<&DriverRefusal> {
        self.failure.as_ref()
    }
    pub fn pending_lease(&self) -> Option<&CursorSourceLease> {
        self.pending
            .as_ref()
            .and_then(|pending| pending.lease.as_ref())
            .or(self.unqueued.as_ref())
    }
    pub fn execution_count(&self) -> usize {
        self.records.len()
    }
    pub fn execution(&self, index: usize) -> Option<SourceExecution<'_>> {
        let record = self.records.get(index)?;
        let operation = &self.operations[record.operation];
        Some(SourceExecution {
            operation: operation.name,
            ordinal: record.ordinal,
            input: &self.arena[record.input.clone()],
            output: &self.arena[record.output.clone()],
            acknowledged_sequence: record.ack,
            submitted_to_cursor: record.submitted,
            definition: operation.kernel.definition(),
            active_plays: &operation.plays,
        })
    }
    /// Allocating diagnostic snapshot, never used by poll.
    pub fn signs(
        &self,
        operation: &str,
    ) -> Option<std::collections::BTreeMap<conduit_core::HostId, Vec<conduit_kernel::KernelEvent>>>
    {
        self.operations
            .iter()
            .find(|entry| entry.name == operation)
            .map(|entry| entry.kernel.signs())
    }
    pub fn last_kernel_error(&self, operation: &str) -> Option<&KernelCompositeError> {
        self.operations
            .iter()
            .find(|entry| entry.name == operation)?
            .error
            .as_ref()
    }
    /// Includes a refused oversized result, still unACKed in its actual Fore buffer.
    pub fn pending_actual_output(&self) -> Option<&[u8]> {
        let pending = self.pending.as_ref()?;
        Some(self.operations[pending.operation].retained_output())
    }
    pub fn unexpected_host_request(
        &self,
        operation: &str,
    ) -> Option<&conduit_composite::KernelCompositeHostRequest> {
        self.operations
            .iter()
            .find(|entry| entry.name == operation)?
            .unexpected
            .as_ref()
    }
    fn correlate(&mut self, cursor: &PreparedRoundCursor<'_>) -> Result<(), DriverRefusal> {
        if let Some(binding) = &self.binding {
            if !binding.matches(cursor) {
                return Err(DriverRefusal::ForeignCursor);
            }
        } else {
            self.binding = Some(cursor.instance_handle());
        }
        Ok(())
    }
    fn finish<T>(&mut self, result: Result<T, DriverRefusal>) -> Result<T, DriverRefusal> {
        if let Err(reason) = &result {
            if !matches!(
                reason,
                DriverRefusal::Pressure
                    | DriverRefusal::ForeignCursor
                    | DriverRefusal::Cursor(CursorRefusal::Pressure | CursorRefusal::ForeignLease)
            ) {
                self.failure = Some(reason.clone());
                self.closed = true;
            }
        }
        result
    }
    /// At most one Source kernel step or one cursor admission step. No replay prefix.
    pub fn poll_fallback(
        &mut self,
        cursor: &mut PreparedRoundCursor<'_>,
    ) -> Result<DriverProgress, DriverRefusal> {
        self.correlate(cursor)?;
        let result = if cursor.mode() != conduit_language::analysis_epoch_owner::fallback_round::AnalysisRoundMode::ZeroScoreFallback {
            Err(DriverRefusal::Preparation)
        } else { self.poll_source_or_cursor(cursor) };
        self.finish(result)
    }
    #[cfg(all(
        feature = "analysis-round-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub fn poll_learned(
        &mut self,
        cursor: &mut PreparedRoundCursor<'_>,
        channel: &mut numerical::PreparedCursorLearnedChannel,
    ) -> Result<DriverProgress, DriverRefusal> {
        self.correlate(cursor)?;
        let result =
            if cursor.progress() == CursorProgress::ReadyNumerical && self.pending.is_none() {
                if self.closed {
                    Err(DriverRefusal::Closed)
                } else {
                    channel
                        .step(cursor)
                        .map(|_| DriverProgress::WaitingNumerical)
                        .map_err(DriverRefusal::Cursor)
                }
            } else {
                self.poll_source_or_cursor(cursor)
            };
        self.finish(result)
    }
    fn poll_source_or_cursor(
        &mut self,
        cursor: &mut PreparedRoundCursor<'_>,
    ) -> Result<DriverProgress, DriverRefusal> {
        if self.closed {
            return Err(DriverRefusal::Closed);
        }
        if self.pending.is_some() {
            return self.poll_pending(cursor);
        }
        match cursor.progress() {
            CursorProgress::Complete => Ok(DriverProgress::Complete),
            CursorProgress::ReadySource => {
                self.enqueue(cursor)?;
                Ok(DriverProgress::WaitingSource)
            }
            CursorProgress::ReadyNumerical => Err(DriverRefusal::Preparation),
            CursorProgress::AwaitingSource => Err(DriverRefusal::ForeignCursor),
            _ => {
                cursor.step().map_err(DriverRefusal::Cursor)?;
                Ok(DriverProgress::Progress)
            }
        }
    }
    fn enqueue(&mut self, cursor: &mut PreparedRoundCursor<'_>) -> Result<(), DriverRefusal> {
        if self.records.len() >= self.bounds.observations
            || self
                .arena
                .len()
                .checked_add(TRANSPORT * 2)
                .is_none_or(|end| end > self.bounds.evidence_bytes)
        {
            return Err(DriverRefusal::Pressure);
        }
        self.unqueued = Some(cursor.issue_source().map_err(DriverRefusal::Cursor)?);
        let request = cursor
            .source_request(self.unqueued.as_ref().ok_or(DriverRefusal::Sequence)?)
            .map_err(DriverRefusal::Cursor)?;
        // This inventory is prepared from the cursor's exact privately cached Source.
        let operation = self
            .operations
            .iter()
            .position(|entry| entry.name == request.operation)
            .ok_or(DriverRefusal::UnsupportedOperation)?;
        if request.input.len() > TRANSPORT {
            return Err(DriverRefusal::SourceTransport);
        }
        let start = self.arena.len();
        self.arena.extend_from_slice(request.input);
        let end = self.arena.len();
        let record = self.records.len();
        self.records.push(Record {
            operation,
            ordinal: request.ordinal,
            input: start..end,
            output: end..end,
            ack: None,
            submitted: false,
        });
        self.pending = Some(Pending {
            operation,
            record,
            lease: self.unqueued.take(),
            admitted: false,
            acked: false,
        });
        Ok(())
    }
    fn poll_pending(
        &mut self,
        cursor: &mut PreparedRoundCursor<'_>,
    ) -> Result<DriverProgress, DriverRefusal> {
        let pending = self.pending.as_mut().ok_or(DriverRefusal::Sequence)?;
        if let Some(lease) = &pending.lease {
            cursor
                .source_request(lease)
                .map_err(DriverRefusal::Cursor)?;
        }
        let record = &mut self.records[pending.record];
        let operation = &mut self.operations[pending.operation];
        if !pending.admitted {
            operation.submit(&self.arena[record.input.clone()])?;
            pending.admitted = true;
            return Ok(DriverProgress::WaitingSource);
        }
        if !pending.acked {
            let Some(output) = operation.poll()? else {
                return Ok(DriverProgress::WaitingSource);
            };
            // Actual oversized output remains unACKed in the operation's full Fore buffer.
            if output.len() > TRANSPORT {
                return Err(DriverRefusal::SourceTransport);
            }
            let start = self.arena.len();
            self.arena.extend_from_slice(output);
            record.output = start..self.arena.len();
            record.ack = Some(operation.acknowledge()?);
            pending.acked = true;
            return Ok(DriverProgress::WaitingSource);
        }
        let lease = pending.lease.take().ok_or(DriverRefusal::Sequence)?;
        match cursor.submit_source(lease, &self.arena[record.output.clone()]) {
            Ok(()) => {
                record.submitted = true;
                self.pending = None;
                Ok(DriverProgress::Progress)
            }
            Err(failure) => {
                pending.lease = Some(failure.lease);
                Err(DriverRefusal::Cursor(failure.refusal))
            }
        }
    }
    /// Terminal explicit cancellation; preserves pending leases, outputs and ACK facts.
    pub fn cancel(&mut self) -> Result<(), DriverRefusal> {
        self.closed = true;
        let mut result = Ok(());
        for operation in &mut self.operations {
            if let Err(error) = operation.cancel() {
                result = Err(error);
            }
        }
        result
    }
}
