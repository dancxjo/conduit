//! Bounded renderer-neutral presentation of optional runtime observations.
//!
//! This state annotates exact canonical subjects. It never owns or mutates the
//! Patchbay graph, execution, or Plan truth.

use conduit_kernel::debug_observation::{
    DebugEventKind, DebugExecutionIdentity, DebugObservationGap, DebugObservationRecord,
    DebugSubject,
};
use serde::{Deserialize, Serialize};

pub const DEBUGGER_PRESENTATION_SCHEMA: &str = "conduit.patchbay.debugger-presentation/v1";
pub const MAX_DEBUGGER_SUBJECTS: usize = 512;
pub const MAX_DEBUGGER_SUMMARY_BYTES: usize = 96;
pub const RECENT_ACTIVITY_TICKS: u64 = 3;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerExecutionIdentity {
    pub body: [u8; 32],
    pub plan: [u8; 32],
    pub play: [u8; 32],
}

impl From<DebugExecutionIdentity> for DebuggerExecutionIdentity {
    fn from(value: DebugExecutionIdentity) -> Self {
        Self {
            body: value.body,
            plan: value.plan,
            play: value.play,
        }
    }
}

impl DebuggerExecutionIdentity {
    fn kernel(&self) -> DebugExecutionIdentity {
        DebugExecutionIdentity {
            body: self.body,
            plan: self.plan,
            play: self.play,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebuggerSubjectBinding {
    pub runtime_subject: DebugSubject,
    pub visible_subject: String,
    /// Present only when the admitted Plan authoritatively realizes this
    /// subject through a visible Line.
    pub line_subject: Option<String>,
    pub host: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebuggerActivityPhase {
    Active,
    Recent,
    Inactive,
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebuggerValueKind {
    Scalar,
    Text,
    Bytes,
    Opaque,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerValuePresentation {
    pub kind: DebuggerValueKind,
    pub summary: String,
    pub type_identity: Option<u16>,
    pub total_bytes: u32,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerSubjectActivity {
    pub subject: String,
    pub line_subject: Option<String>,
    pub host: u16,
    pub phase: DebuggerActivityPhase,
    pub latest_kind: String,
    pub latest_sequence: u64,
    pub observed_count: u64,
    pub coalesced_count: u64,
    pub last_activity_tick: u64,
    pub latest_value: Option<DebuggerValuePresentation>,
    pub retained_fault_code: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerPresentation {
    pub schema: String,
    pub execution: DebuggerExecutionIdentity,
    pub revision: u64,
    pub tick: u64,
    pub reduced_motion: bool,
    pub gap: Option<DebuggerGapPresentation>,
    pub activities: Vec<DebuggerSubjectActivity>,
    #[serde(skip)]
    bindings: Vec<DebuggerSubjectBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerGapPresentation {
    pub dropped_records: u64,
    pub first_retained_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebuggerPresentationError {
    InvalidBounds,
    DuplicateBinding,
    StaleExecution,
    UnknownSubject,
    HostMismatch,
    NonmonotonicSequence,
    UnsupportedEvent,
}

impl DebuggerPresentation {
    pub fn new(
        execution: DebugExecutionIdentity,
        bindings: Vec<DebuggerSubjectBinding>,
        reduced_motion: bool,
    ) -> Result<Self, DebuggerPresentationError> {
        if bindings.is_empty() || bindings.len() > MAX_DEBUGGER_SUBJECTS {
            return Err(DebuggerPresentationError::InvalidBounds);
        }
        for (index, binding) in bindings.iter().enumerate() {
            if binding.visible_subject.is_empty()
                || binding.visible_subject.len() > MAX_DEBUGGER_SUMMARY_BYTES
                || binding
                    .line_subject
                    .as_ref()
                    .is_some_and(|line| line.is_empty() || line.len() > MAX_DEBUGGER_SUMMARY_BYTES)
                || bindings[..index]
                    .iter()
                    .any(|prior| prior.runtime_subject == binding.runtime_subject)
            {
                return Err(DebuggerPresentationError::DuplicateBinding);
            }
        }
        Ok(Self {
            schema: DEBUGGER_PRESENTATION_SCHEMA.to_owned(),
            execution: execution.into(),
            revision: 0,
            tick: 0,
            reduced_motion,
            gap: None,
            activities: Vec::new(),
            bindings,
        })
    }

    pub fn observe(
        &mut self,
        record: &DebugObservationRecord,
    ) -> Result<(), DebuggerPresentationError> {
        if record.execution != self.execution.kernel() {
            return Err(DebuggerPresentationError::StaleExecution);
        }
        if matches!(record.kind, DebugEventKind::Unsupported(_)) {
            return Err(DebuggerPresentationError::UnsupportedEvent);
        }
        if self
            .activities
            .iter()
            .any(|activity| activity.latest_sequence >= record.sequence)
        {
            return Err(DebuggerPresentationError::NonmonotonicSequence);
        }
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.runtime_subject == record.subject)
            .cloned()
            .ok_or(DebuggerPresentationError::UnknownSubject)?;
        if binding.host != record.host {
            return Err(DebuggerPresentationError::HostMismatch);
        }
        let related = record.related_subject.and_then(|subject| {
            self.bindings
                .iter()
                .find(|binding| binding.runtime_subject == subject)
                .cloned()
        });
        if related
            .as_ref()
            .is_some_and(|binding| binding.host != record.host)
        {
            return Err(DebuggerPresentationError::HostMismatch);
        }
        let phase = if record.kind == DebugEventKind::Fault {
            DebuggerActivityPhase::Faulted
        } else {
            DebuggerActivityPhase::Active
        };
        let value = value_presentation(record);
        self.update_activity(&binding, record, phase, value.clone());
        if let Some(related) = related {
            self.update_activity(&related, record, phase, value);
        }
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    fn update_activity(
        &mut self,
        binding: &DebuggerSubjectBinding,
        record: &DebugObservationRecord,
        phase: DebuggerActivityPhase,
        value: Option<DebuggerValuePresentation>,
    ) {
        if let Some(activity) = self
            .activities
            .iter_mut()
            .find(|activity| activity.subject == binding.visible_subject)
        {
            activity.phase = phase;
            activity.latest_kind = event_name(record.kind).to_owned();
            activity.latest_sequence = record.sequence;
            activity.observed_count = activity.observed_count.saturating_add(1);
            activity.coalesced_count = activity.coalesced_count.saturating_add(1);
            activity.last_activity_tick = self.tick;
            activity.latest_value = value;
            if record.kind == DebugEventKind::Fault {
                activity.retained_fault_code = record.fault_code;
            }
        } else {
            self.activities.push(DebuggerSubjectActivity {
                subject: binding.visible_subject.clone(),
                line_subject: binding.line_subject.clone(),
                host: binding.host,
                phase,
                latest_kind: event_name(record.kind).to_owned(),
                latest_sequence: record.sequence,
                observed_count: 1,
                coalesced_count: 0,
                last_activity_tick: self.tick,
                latest_value: value,
                retained_fault_code: record.fault_code,
            });
        }
    }

    pub fn note_gap(&mut self, gap: DebugObservationGap) {
        self.gap = Some(DebuggerGapPresentation {
            dropped_records: gap.dropped_records,
            first_retained_sequence: gap.first_retained_sequence,
        });
        self.revision = self.revision.saturating_add(1);
    }

    pub fn advance(&mut self, tick: u64) -> Result<(), DebuggerPresentationError> {
        if tick < self.tick {
            return Err(DebuggerPresentationError::NonmonotonicSequence);
        }
        self.tick = tick;
        for activity in &mut self.activities {
            if activity.phase == DebuggerActivityPhase::Faulted {
                continue;
            }
            let age = tick.saturating_sub(activity.last_activity_tick);
            activity.phase = if age == 0 {
                DebuggerActivityPhase::Active
            } else if age <= RECENT_ACTIVITY_TICKS {
                DebuggerActivityPhase::Recent
            } else {
                DebuggerActivityPhase::Inactive
            };
        }
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn clear_fault(&mut self, subject: &str) -> Result<(), DebuggerPresentationError> {
        let activity = self
            .activities
            .iter_mut()
            .find(|activity| activity.subject == subject)
            .ok_or(DebuggerPresentationError::UnknownSubject)?;
        activity.retained_fault_code = None;
        activity.phase = DebuggerActivityPhase::Inactive;
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    /// Detaching returns the exact finite debugger state and leaves topology
    /// and execution ownership with their existing owners.
    pub fn detach(self) -> Vec<DebuggerSubjectActivity> {
        self.activities
    }
}

fn event_name(kind: DebugEventKind) -> &'static str {
    match kind {
        DebugEventKind::GearStarted => "gear-started",
        DebugEventKind::GearCompleted => "gear-completed",
        DebugEventKind::ValueSent => "value-sent",
        DebugEventKind::ValueReceived => "value-received",
        DebugEventKind::Fault => "fault",
        DebugEventKind::Unsupported(_) => "unsupported",
    }
}

fn value_presentation(record: &DebugObservationRecord) -> Option<DebuggerValuePresentation> {
    if !matches!(
        record.kind,
        DebugEventKind::ValueSent | DebugEventKind::ValueReceived
    ) {
        return None;
    }
    let preview = record.preview();
    let (kind, summary) = match core::str::from_utf8(preview) {
        Ok(text) if text.parse::<f64>().is_ok() || matches!(text, "true" | "false") => {
            (DebuggerValueKind::Scalar, text.to_owned())
        }
        Ok(text) if !text.chars().any(char::is_control) => {
            (DebuggerValueKind::Text, format!("\"{text}\""))
        }
        _ if preview.is_empty() => (
            DebuggerValueKind::Opaque,
            format!("{} B", record.value_bytes),
        ),
        _ => (
            DebuggerValueKind::Bytes,
            format!("{} B", record.value_bytes),
        ),
    };
    Some(DebuggerValuePresentation {
        kind,
        summary: bounded_summary(&summary),
        type_identity: record.type_identity,
        total_bytes: record.value_bytes,
        truncated: record.preview_truncated,
    })
}

fn bounded_summary(summary: &str) -> String {
    let end = summary
        .char_indices()
        .take_while(|(index, character)| index + character.len_utf8() <= MAX_DEBUGGER_SUMMARY_BYTES)
        .map(|(index, character)| index + character.len_utf8())
        .last()
        .unwrap_or(0);
    summary[..end].to_owned()
}
