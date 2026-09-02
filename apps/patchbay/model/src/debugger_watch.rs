//! Exact, finite debugger Watches layered beside canonical Patchbay truth.

use crate::{
    debugger_presentation::{event_name, value_presentation},
    DebuggerExecutionIdentity, DebuggerGapPresentation, DebuggerPresentation,
    DebuggerValuePresentation,
};
use conduit_kernel::debug_observation::{
    DebugExecutionIdentity, DebugObservationGap, DebugObservationRecord, DebugSubject,
};
use serde::{Deserialize, Serialize};

pub const DEBUGGER_WATCH_SCHEMA: &str = "conduit.patchbay.debugger-watch-set/v1";
pub const MAX_DEBUGGER_WATCHES: usize = 8;
pub const MAX_WATCH_HISTORY_RECORDS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebuggerWatchSubjectRole {
    Gear,
    Port,
    Cord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebuggerWatchBinding {
    pub runtime_subject: DebugSubject,
    pub visible_subject: String,
    pub role: DebuggerWatchSubjectRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebuggerWatchLifecycle {
    Current,
    Missing,
    StaleExecution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerWatchRate {
    /// A sequence-domain density, never fabricated wall-clock frequency.
    pub updates: u64,
    pub sequence_span: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerWatchHistoryEntry {
    pub sequence: u64,
    pub event: String,
    pub value: Option<DebuggerValuePresentation>,
    pub fault_code: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerWatch {
    pub subject: String,
    pub role: DebuggerWatchSubjectRole,
    pub execution: DebuggerExecutionIdentity,
    pub lifecycle: DebuggerWatchLifecycle,
    pub latest: Option<DebuggerWatchHistoryEntry>,
    pub update_count: u64,
    pub rate: Option<DebuggerWatchRate>,
    pub history: Vec<DebuggerWatchHistoryEntry>,
    pub evicted_history: u64,
    pub telemetry_gap: Option<DebuggerGapPresentation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerWatchSet {
    pub schema: String,
    pub execution: DebuggerExecutionIdentity,
    pub revision: u64,
    pub focused_subject: Option<String>,
    pub eligible_subjects: Vec<(String, DebuggerWatchSubjectRole)>,
    pub watches: Vec<DebuggerWatch>,
    #[serde(skip)]
    bindings: Vec<DebuggerWatchBinding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebuggerWatchError {
    InvalidBounds,
    DuplicateBinding,
    IneligibleSubject,
    DuplicateWatch,
    WatchLimit,
    UnknownWatch,
    StaleExecution,
    NonmonotonicSequence,
}

impl DebuggerWatchSet {
    pub fn new(
        execution: DebugExecutionIdentity,
        bindings: Vec<DebuggerWatchBinding>,
    ) -> Result<Self, DebuggerWatchError> {
        if bindings.is_empty() || bindings.len() > crate::MAX_DEBUGGER_SUBJECTS {
            return Err(DebuggerWatchError::InvalidBounds);
        }
        for (index, binding) in bindings.iter().enumerate() {
            if binding.visible_subject.is_empty()
                || binding.visible_subject.len() > crate::MAX_DEBUGGER_SUMMARY_BYTES
                || bindings[..index].iter().any(|prior| {
                    prior.runtime_subject == binding.runtime_subject
                        || prior.visible_subject == binding.visible_subject
                })
            {
                return Err(DebuggerWatchError::DuplicateBinding);
            }
        }
        let eligible_subjects = bindings
            .iter()
            .map(|binding| (binding.visible_subject.clone(), binding.role))
            .collect();
        Ok(Self {
            schema: DEBUGGER_WATCH_SCHEMA.to_owned(),
            execution: execution.into(),
            revision: 0,
            focused_subject: None,
            eligible_subjects,
            watches: Vec::with_capacity(MAX_DEBUGGER_WATCHES),
            bindings,
        })
    }

    pub fn add(&mut self, subject: &str) -> Result<(), DebuggerWatchError> {
        let (_, role) = self
            .eligible_subjects
            .iter()
            .find(|(candidate, _)| candidate == subject)
            .ok_or(DebuggerWatchError::IneligibleSubject)?;
        if self.watches.iter().any(|watch| watch.subject == subject) {
            return Err(DebuggerWatchError::DuplicateWatch);
        }
        if self.watches.len() == MAX_DEBUGGER_WATCHES {
            return Err(DebuggerWatchError::WatchLimit);
        }
        self.watches.push(DebuggerWatch {
            subject: subject.to_owned(),
            role: *role,
            execution: self.execution.clone(),
            lifecycle: DebuggerWatchLifecycle::Current,
            latest: None,
            update_count: 0,
            rate: None,
            history: Vec::with_capacity(MAX_WATCH_HISTORY_RECORDS),
            evicted_history: 0,
            telemetry_gap: None,
        });
        self.focused_subject = Some(subject.to_owned());
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn remove(&mut self, subject: &str) -> Result<DebuggerWatch, DebuggerWatchError> {
        let index = self
            .watches
            .iter()
            .position(|watch| watch.subject == subject)
            .ok_or(DebuggerWatchError::UnknownWatch)?;
        let removed = self.watches.remove(index);
        if self.focused_subject.as_deref() == Some(subject) {
            self.focused_subject = self.watches.last().map(|watch| watch.subject.clone());
        }
        self.revision = self.revision.saturating_add(1);
        Ok(removed)
    }

    /// Seeds a newly added Watch from the already coalesced live projection.
    /// Subsequent full history still comes only from exact P0 records.
    pub fn capture_current(
        &mut self,
        subject: &str,
        debugger: &DebuggerPresentation,
    ) -> Result<bool, DebuggerWatchError> {
        if debugger.execution != self.execution {
            return Err(DebuggerWatchError::StaleExecution);
        }
        let Some(activity) = debugger
            .activities
            .iter()
            .find(|activity| activity.subject == subject)
        else {
            return Ok(false);
        };
        let watch = self
            .watches
            .iter_mut()
            .find(|watch| watch.subject == subject)
            .ok_or(DebuggerWatchError::UnknownWatch)?;
        let entry = DebuggerWatchHistoryEntry {
            sequence: activity.latest_sequence,
            event: activity.latest_kind.clone(),
            value: activity.latest_value.clone(),
            fault_code: activity.retained_fault_code,
        };
        watch.latest = Some(entry.clone());
        watch.history.push(entry);
        watch.update_count = 1;
        watch.telemetry_gap = debugger.gap.clone();
        self.revision = self.revision.saturating_add(1);
        Ok(true)
    }

    pub fn focus(&mut self, subject: &str) -> Result<(), DebuggerWatchError> {
        if !self.watches.iter().any(|watch| watch.subject == subject) {
            return Err(DebuggerWatchError::UnknownWatch);
        }
        self.focused_subject = Some(subject.to_owned());
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn clear_history(&mut self, subject: &str) -> Result<(), DebuggerWatchError> {
        let watch = self
            .watches
            .iter_mut()
            .find(|watch| watch.subject == subject)
            .ok_or(DebuggerWatchError::UnknownWatch)?;
        watch.latest = None;
        watch.update_count = 0;
        watch.rate = None;
        watch.history.clear();
        watch.evicted_history = 0;
        watch.telemetry_gap = None;
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn observe(&mut self, record: &DebugObservationRecord) -> Result<bool, DebuggerWatchError> {
        if (DebugExecutionIdentity {
            body: self.execution.body,
            plan: self.execution.plan,
            play: self.execution.play,
        }) != record.execution
        {
            return Err(DebuggerWatchError::StaleExecution);
        }
        let mut subjects = [Some(record.subject), record.related_subject];
        let mut changed = false;
        for runtime_subject in subjects.iter_mut().filter_map(Option::take) {
            let Some(binding) = self
                .bindings
                .iter()
                .find(|binding| binding.runtime_subject == runtime_subject)
            else {
                continue;
            };
            let Some(watch) = self
                .watches
                .iter_mut()
                .find(|watch| watch.subject == binding.visible_subject)
            else {
                continue;
            };
            if watch
                .latest
                .as_ref()
                .is_some_and(|latest| latest.sequence >= record.sequence)
            {
                return Err(DebuggerWatchError::NonmonotonicSequence);
            }
            let entry = DebuggerWatchHistoryEntry {
                sequence: record.sequence,
                event: event_name(record.kind).to_owned(),
                value: value_presentation(record),
                fault_code: record.fault_code,
            };
            if watch.history.len() == MAX_WATCH_HISTORY_RECORDS {
                watch.history.remove(0);
                watch.evicted_history = watch.evicted_history.saturating_add(1);
            }
            watch.history.push(entry.clone());
            watch.latest = Some(entry);
            watch.update_count = watch.update_count.saturating_add(1);
            watch.lifecycle = DebuggerWatchLifecycle::Current;
            let first = watch.history.first().map(|entry| entry.sequence);
            let last = watch.history.last().map(|entry| entry.sequence);
            watch.rate = first.zip(last).and_then(|(first, last)| {
                (last > first).then_some(DebuggerWatchRate {
                    updates: u64::try_from(watch.history.len()).unwrap_or(u64::MAX),
                    sequence_span: last - first,
                })
            });
            changed = true;
        }
        if changed {
            self.revision = self.revision.saturating_add(1);
        }
        Ok(changed)
    }

    pub fn note_gap(&mut self, gap: DebugObservationGap) {
        let gap = DebuggerGapPresentation {
            dropped_records: gap.dropped_records,
            first_retained_sequence: gap.first_retained_sequence,
        };
        for watch in &mut self.watches {
            watch.telemetry_gap = Some(gap.clone());
        }
        self.revision = self.revision.saturating_add(1);
    }

    pub fn replace_execution(&mut self, execution: DebugExecutionIdentity) {
        self.execution = execution.into();
        self.focused_subject = None;
        for watch in &mut self.watches {
            watch.lifecycle = DebuggerWatchLifecycle::StaleExecution;
        }
        self.revision = self.revision.saturating_add(1);
    }

    pub fn subject_disappeared(&mut self, subject: &str) -> Result<(), DebuggerWatchError> {
        let watch = self
            .watches
            .iter_mut()
            .find(|watch| watch.subject == subject)
            .ok_or(DebuggerWatchError::UnknownWatch)?;
        watch.lifecycle = DebuggerWatchLifecycle::Missing;
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }
}
