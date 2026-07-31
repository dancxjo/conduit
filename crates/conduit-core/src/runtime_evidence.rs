//! Plan-visible bounded runtime evidence recording policy.

use crate::{
    EventClass, EventProviderCapabilities, EventStreamContract, Id, RetentionPolicy,
    validate_stream_contract,
};

pub const RUNTIME_EVIDENCE_POLICY_VERSION: u32 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEvidenceMode {
    Disabled,
    Record,
}

impl RuntimeEvidenceMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Record => "record",
        }
    }
}

/// Exact plan policy for projecting executor observations into immutable
/// `ExecutionEvent` records on one Resonance stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeEvidencePolicy<'a> {
    pub schema_version: u32,
    pub mode: RuntimeEvidenceMode,
    pub stream: Option<Id<'a>>,
    pub maximum_events: u32,
    pub maximum_bytes: u64,
    /// Capacity unavailable to optional telemetry, retained for one terminal
    /// record and other required observations.
    pub required_reserve_events: u32,
    pub required_reserve_bytes: u64,
    /// Record telemetry ordinals satisfying `ordinal % period == offset`.
    pub telemetry_period: u32,
    pub telemetry_offset: u32,
    /// Accounted bytes charged for one explicit sampled-telemetry summary.
    pub gap_summary_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeEvidenceReason {
    UnsupportedVersion,
    InvalidPolicy,
    StreamRequired,
    StreamForbidden,
    StreamIncapable,
    ArithmeticOverflow,
    RequiredCapacityExceeded,
    SummaryCapacityExceeded,
    DuplicateTerminal,
    MissingTerminal,
}

impl RuntimeEvidenceReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-RTE-001",
            Self::InvalidPolicy => "CND-RTE-002",
            Self::StreamRequired | Self::StreamForbidden => "CND-RTE-003",
            Self::StreamIncapable => "CND-RTE-004",
            Self::ArithmeticOverflow => "CND-RTE-005",
            Self::RequiredCapacityExceeded => "CND-RTE-006",
            Self::SummaryCapacityExceeded => "CND-RTE-007",
            Self::DuplicateTerminal | Self::MissingTerminal => "CND-RTE-008",
        }
    }
}

pub fn validate_runtime_evidence_policy<'a>(
    policy: RuntimeEvidencePolicy<'a>,
    stream: Option<(EventStreamContract<'_>, EventProviderCapabilities)>,
) -> Result<(), RuntimeEvidenceReason> {
    if policy.schema_version != RUNTIME_EVIDENCE_POLICY_VERSION {
        return Err(RuntimeEvidenceReason::UnsupportedVersion);
    }
    match policy.mode {
        RuntimeEvidenceMode::Disabled => {
            if policy.stream.is_some() || stream.is_some() {
                return Err(RuntimeEvidenceReason::StreamForbidden);
            }
            if policy.maximum_events != 0
                || policy.maximum_bytes != 0
                || policy.required_reserve_events != 0
                || policy.required_reserve_bytes != 0
                || policy.telemetry_period != 0
                || policy.telemetry_offset != 0
                || policy.gap_summary_bytes != 0
            {
                return Err(RuntimeEvidenceReason::InvalidPolicy);
            }
            Ok(())
        }
        RuntimeEvidenceMode::Record => {
            let Some(stream_id) = policy.stream else {
                return Err(RuntimeEvidenceReason::StreamRequired);
            };
            let Some((contract, provider)) = stream else {
                return Err(RuntimeEvidenceReason::StreamRequired);
            };
            if contract.id != stream_id
                || contract.event_class != EventClass::NormativeEvidence
                || !contract.terminal_evidence_required
                || contract.subscriber_coupling.flow().pressure.permits_loss()
                || validate_stream_contract(contract, provider).is_err()
                || !retention_covers(
                    contract.retention,
                    policy.maximum_events,
                    policy.maximum_bytes,
                )
            {
                return Err(RuntimeEvidenceReason::StreamIncapable);
            }
            if policy.maximum_events < 2
                || policy.maximum_bytes == 0
                || policy.required_reserve_events == 0
                || policy.required_reserve_events >= policy.maximum_events
                || policy.required_reserve_bytes == 0
                || policy.required_reserve_bytes >= policy.maximum_bytes
                || policy.telemetry_period == 0
                || policy.telemetry_offset >= policy.telemetry_period
                || policy.gap_summary_bytes == 0
                || policy.gap_summary_bytes > policy.maximum_bytes
            {
                return Err(RuntimeEvidenceReason::InvalidPolicy);
            }
            Ok(())
        }
    }
}

fn retention_covers(retention: RetentionPolicy, events: u32, bytes: u64) -> bool {
    let events = u64::from(events);
    match retention {
        RetentionPolicy::Ephemeral => events <= 1 && bytes <= 1,
        RetentionPolicy::Ring {
            maximum_events,
            maximum_bytes,
        }
        | RetentionPolicy::CheckpointAssociated {
            maximum_events,
            maximum_bytes,
            ..
        } => u64::from(maximum_events) >= events && maximum_bytes >= bytes,
        RetentionPolicy::DurableAppend {
            maximum_events,
            maximum_bytes,
            ..
        } => maximum_events >= events && maximum_bytes >= bytes,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TelemetryAdmission {
    Disabled,
    Record,
    RecordAfterSummary { skipped: u64 },
    Sampled,
}

/// Allocation-free accounting state shared by hosted and embedded recorders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeEvidenceBudget<'a> {
    policy: RuntimeEvidencePolicy<'a>,
    recorded_events: u32,
    recorded_bytes: u64,
    telemetry_seen: u64,
    telemetry_skipped: u64,
    terminal_recorded: bool,
}

impl<'a> RuntimeEvidenceBudget<'a> {
    pub const fn new(policy: RuntimeEvidencePolicy<'a>) -> Self {
        Self {
            policy,
            recorded_events: 0,
            recorded_bytes: 0,
            telemetry_seen: 0,
            telemetry_skipped: 0,
            terminal_recorded: false,
        }
    }

    pub fn record_required(
        &mut self,
        accounted_bytes: u64,
        terminal: bool,
    ) -> Result<(), RuntimeEvidenceReason> {
        if self.policy.mode == RuntimeEvidenceMode::Disabled {
            return Ok(());
        }
        if accounted_bytes == 0 {
            return Err(RuntimeEvidenceReason::InvalidPolicy);
        }
        if terminal && self.terminal_recorded {
            return Err(RuntimeEvidenceReason::DuplicateTerminal);
        }
        let next_events = self
            .recorded_events
            .checked_add(1)
            .ok_or(RuntimeEvidenceReason::ArithmeticOverflow)?;
        let next_bytes = self
            .recorded_bytes
            .checked_add(accounted_bytes)
            .ok_or(RuntimeEvidenceReason::ArithmeticOverflow)?;
        let fits = if terminal {
            next_events <= self.policy.maximum_events && next_bytes <= self.policy.maximum_bytes
        } else {
            next_events
                .checked_add(self.policy.required_reserve_events)
                .is_some_and(|value| value <= self.policy.maximum_events)
                && next_bytes
                    .checked_add(self.policy.required_reserve_bytes)
                    .is_some_and(|value| value <= self.policy.maximum_bytes)
        };
        if !fits {
            return Err(RuntimeEvidenceReason::RequiredCapacityExceeded);
        }
        self.recorded_events = next_events;
        self.recorded_bytes = next_bytes;
        self.terminal_recorded |= terminal;
        Ok(())
    }

    pub fn admit_telemetry(
        &mut self,
        accounted_bytes: u64,
    ) -> Result<TelemetryAdmission, RuntimeEvidenceReason> {
        if self.policy.mode == RuntimeEvidenceMode::Disabled {
            return Ok(TelemetryAdmission::Disabled);
        }
        if accounted_bytes == 0 {
            return Err(RuntimeEvidenceReason::InvalidPolicy);
        }
        let ordinal = self.telemetry_seen;
        self.telemetry_seen = ordinal
            .checked_add(1)
            .ok_or(RuntimeEvidenceReason::ArithmeticOverflow)?;
        let selected = ordinal % u64::from(self.policy.telemetry_period)
            == u64::from(self.policy.telemetry_offset);
        if !selected {
            self.telemetry_skipped = self
                .telemetry_skipped
                .checked_add(1)
                .ok_or(RuntimeEvidenceReason::ArithmeticOverflow)?;
            return Ok(TelemetryAdmission::Sampled);
        }

        let summary_events = u32::from(self.telemetry_skipped != 0);
        let summary_bytes = if self.telemetry_skipped == 0 {
            0
        } else {
            self.policy.gap_summary_bytes
        };
        let next_events = self
            .recorded_events
            .checked_add(1)
            .and_then(|value| value.checked_add(summary_events))
            .ok_or(RuntimeEvidenceReason::ArithmeticOverflow)?;
        let next_bytes = self
            .recorded_bytes
            .checked_add(accounted_bytes)
            .and_then(|value| value.checked_add(summary_bytes))
            .ok_or(RuntimeEvidenceReason::ArithmeticOverflow)?;
        let fits = next_events
            .checked_add(self.policy.required_reserve_events)
            .is_some_and(|value| value <= self.policy.maximum_events)
            && next_bytes
                .checked_add(self.policy.required_reserve_bytes)
                .is_some_and(|value| value <= self.policy.maximum_bytes);
        if !fits {
            self.telemetry_skipped = self
                .telemetry_skipped
                .checked_add(1)
                .ok_or(RuntimeEvidenceReason::ArithmeticOverflow)?;
            return Ok(TelemetryAdmission::Sampled);
        }

        let decision = if self.telemetry_skipped == 0 {
            TelemetryAdmission::Record
        } else {
            TelemetryAdmission::RecordAfterSummary {
                skipped: self.telemetry_skipped,
            }
        };
        self.recorded_events = next_events;
        self.recorded_bytes = next_bytes;
        self.telemetry_skipped = 0;
        Ok(decision)
    }

    /// Charge one final explicit sampling summary before the terminal record.
    pub fn flush_sampling_summary(&mut self) -> Result<Option<u64>, RuntimeEvidenceReason> {
        if self.policy.mode == RuntimeEvidenceMode::Disabled || self.telemetry_skipped == 0 {
            return Ok(None);
        }
        let skipped = self.telemetry_skipped;
        self.record_required(self.policy.gap_summary_bytes, false)
            .map_err(|_| RuntimeEvidenceReason::SummaryCapacityExceeded)?;
        self.telemetry_skipped = 0;
        Ok(Some(skipped))
    }

    pub fn finish(self) -> Result<(), RuntimeEvidenceReason> {
        if self.policy.mode == RuntimeEvidenceMode::Record && !self.terminal_recorded {
            return Err(RuntimeEvidenceReason::MissingTerminal);
        }
        if self.telemetry_skipped != 0 {
            return Err(RuntimeEvidenceReason::SummaryCapacityExceeded);
        }
        Ok(())
    }

    pub const fn recorded_events(&self) -> u32 {
        self.recorded_events
    }

    pub const fn recorded_bytes(&self) -> u64 {
        self.recorded_bytes
    }
}
