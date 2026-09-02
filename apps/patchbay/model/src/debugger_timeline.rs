//! Finite observation playback shared by live-following and replay projection.

use crate::{
    debugger_presentation::{event_name, value_presentation},
    DebuggerExecutionIdentity, DebuggerGapPresentation, DebuggerValuePresentation,
    DebuggerWatchHistoryEntry, DebuggerWatchSet, MAX_DEBUGGER_SUMMARY_BYTES,
};
use conduit_kernel::debug_observation::{
    DebugExecutionIdentity, DebugObservationGap, DebugObservationRecord, DebugSubject,
};
use serde::{Deserialize, Serialize};

pub const DEBUGGER_TIMELINE_SCHEMA: &str = "conduit.patchbay.debugger-timeline/v1";
pub const MAX_DEBUGGER_TIMELINE_EVENTS: usize = 128;
pub const MAX_DEBUGGER_TIMELINE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebuggerTimelineBinding {
    pub execution: DebugExecutionIdentity,
    pub runtime_subject: DebugSubject,
    pub visible_subject: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebuggerTimelineMode {
    Live,
    Replay,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerTimelineEvent {
    pub execution: DebuggerExecutionIdentity,
    pub sequence: u64,
    pub host_sequence: u64,
    pub host: u16,
    pub form: u16,
    pub subject: String,
    pub related_subject: Option<String>,
    pub event: String,
    pub value: Option<DebuggerValuePresentation>,
    pub fault_code: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerTimelineSubjectState {
    pub subject: String,
    pub event: String,
    pub sequence: u64,
    pub value: Option<DebuggerValuePresentation>,
    pub fault_code: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerTimelineWatchState {
    pub subject: String,
    pub historical: bool,
    pub latest: Option<DebuggerWatchHistoryEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerTimelineProjection {
    pub mode: DebuggerTimelineMode,
    pub cursor: Option<usize>,
    pub cursor_sequence: Option<u64>,
    pub execution: Option<DebuggerExecutionIdentity>,
    pub exact_reconstruction: bool,
    pub states: Vec<DebuggerTimelineSubjectState>,
    pub watch_states: Vec<DebuggerTimelineWatchState>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerTimeline {
    pub schema: String,
    pub revision: u64,
    pub mode: DebuggerTimelineMode,
    pub cursor: Option<usize>,
    pub selected_event: Option<usize>,
    pub subject_filter: Option<String>,
    pub events: Vec<DebuggerTimelineEvent>,
    pub retained_bytes: usize,
    pub evicted_events: u64,
    pub gap: Option<DebuggerGapPresentation>,
    #[serde(skip)]
    bindings: Vec<DebuggerTimelineBinding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebuggerTimelineError {
    InvalidBounds,
    DuplicateBinding,
    UnknownSubject,
    UnsupportedEvent,
    NonmonotonicSequence,
    InvalidCursor,
}

impl DebuggerTimeline {
    pub fn new(bindings: Vec<DebuggerTimelineBinding>) -> Result<Self, DebuggerTimelineError> {
        if bindings.is_empty() || bindings.len() > crate::MAX_DEBUGGER_SUBJECTS {
            return Err(DebuggerTimelineError::InvalidBounds);
        }
        for (index, binding) in bindings.iter().enumerate() {
            if binding.visible_subject.is_empty()
                || binding.visible_subject.len() > MAX_DEBUGGER_SUMMARY_BYTES
                || bindings[..index].iter().any(|prior| {
                    prior.execution == binding.execution
                        && prior.runtime_subject == binding.runtime_subject
                })
            {
                return Err(DebuggerTimelineError::DuplicateBinding);
            }
        }
        Ok(Self {
            schema: DEBUGGER_TIMELINE_SCHEMA.to_owned(),
            revision: 0,
            mode: DebuggerTimelineMode::Live,
            cursor: None,
            selected_event: None,
            subject_filter: None,
            events: Vec::with_capacity(MAX_DEBUGGER_TIMELINE_EVENTS),
            retained_bytes: 0,
            evicted_events: 0,
            gap: None,
            bindings,
        })
    }

    pub fn observe(
        &mut self,
        record: &DebugObservationRecord,
    ) -> Result<(), DebuggerTimelineError> {
        if matches!(
            record.kind,
            conduit_kernel::debug_observation::DebugEventKind::Unsupported(_)
        ) {
            return Err(DebuggerTimelineError::UnsupportedEvent);
        }
        if self
            .events
            .iter()
            .rev()
            .find(|event| event.execution == record.execution.into())
            .is_some_and(|last| last.sequence >= record.sequence)
        {
            return Err(DebuggerTimelineError::NonmonotonicSequence);
        }
        let subject = self.visible(record.execution, record.subject)?.to_owned();
        let related_subject = record
            .related_subject
            .map(|related| self.visible(record.execution, related).map(str::to_owned))
            .transpose()?;
        let event = DebuggerTimelineEvent {
            execution: record.execution.into(),
            sequence: record.sequence,
            host_sequence: record.host_sequence,
            host: record.host,
            form: record.form,
            subject,
            related_subject,
            event: event_name(record.kind).to_owned(),
            value: value_presentation(record),
            fault_code: record.fault_code,
        };
        self.retained_bytes = self.retained_bytes.saturating_add(event.retained_bytes());
        self.events.push(event);
        while self.events.len() > MAX_DEBUGGER_TIMELINE_EVENTS
            || self.retained_bytes > MAX_DEBUGGER_TIMELINE_BYTES
        {
            let removed = self.events.remove(0);
            self.retained_bytes = self.retained_bytes.saturating_sub(removed.retained_bytes());
            self.evicted_events = self.evicted_events.saturating_add(1);
            self.cursor = self.cursor.map(|cursor| cursor.saturating_sub(1));
            self.selected_event = self.selected_event.map(|cursor| cursor.saturating_sub(1));
        }
        if self.mode == DebuggerTimelineMode::Live {
            self.cursor = self.events.len().checked_sub(1);
        }
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn pause(&mut self) {
        self.mode = DebuggerTimelineMode::Replay;
        self.cursor = self.cursor.or_else(|| self.events.len().checked_sub(1));
        self.revision = self.revision.saturating_add(1);
    }

    pub fn jump_live(&mut self) {
        self.mode = DebuggerTimelineMode::Live;
        self.cursor = self.events.len().checked_sub(1);
        self.revision = self.revision.saturating_add(1);
    }

    pub fn move_cursor(&mut self, cursor: usize) -> Result<(), DebuggerTimelineError> {
        if cursor >= self.events.len() {
            return Err(DebuggerTimelineError::InvalidCursor);
        }
        self.mode = DebuggerTimelineMode::Replay;
        self.cursor = Some(cursor);
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn previous_event(&mut self) -> Result<(), DebuggerTimelineError> {
        let cursor = self.cursor.ok_or(DebuggerTimelineError::InvalidCursor)?;
        self.move_cursor(cursor.saturating_sub(1))
    }

    pub fn next_event(&mut self) -> Result<(), DebuggerTimelineError> {
        let cursor = self.cursor.ok_or(DebuggerTimelineError::InvalidCursor)?;
        self.move_cursor((cursor + 1).min(self.events.len().saturating_sub(1)))
    }

    pub fn select_event(&mut self, index: usize) -> Result<&str, DebuggerTimelineError> {
        let subject = self
            .events
            .get(index)
            .ok_or(DebuggerTimelineError::InvalidCursor)?
            .subject
            .clone();
        self.move_cursor(index)?;
        self.selected_event = Some(index);
        self.subject_filter = Some(subject);
        self.revision = self.revision.saturating_add(1);
        Ok(self.subject_filter.as_deref().expect("just assigned"))
    }

    pub fn filter_subject(&mut self, subject: Option<&str>) -> Result<(), DebuggerTimelineError> {
        if subject.is_some_and(|subject| {
            !self.events.iter().any(|event| {
                event.subject == subject || event.related_subject.as_deref() == Some(subject)
            })
        }) {
            return Err(DebuggerTimelineError::UnknownSubject);
        }
        self.subject_filter = subject.map(str::to_owned);
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn note_gap(&mut self, gap: DebugObservationGap) {
        self.gap = Some(DebuggerGapPresentation {
            dropped_records: gap.dropped_records,
            first_retained_sequence: gap.first_retained_sequence,
        });
        self.revision = self.revision.saturating_add(1);
    }

    pub fn visible_events(&self) -> impl Iterator<Item = (usize, &DebuggerTimelineEvent)> {
        self.events.iter().enumerate().filter(|(_, event)| {
            self.subject_filter.as_ref().is_none_or(|subject| {
                &event.subject == subject || event.related_subject.as_ref() == Some(subject)
            })
        })
    }

    pub fn project(&self, watches: Option<&DebuggerWatchSet>) -> DebuggerTimelineProjection {
        let cursor = self.cursor.filter(|cursor| *cursor < self.events.len());
        let selected = cursor.and_then(|cursor| self.events.get(cursor));
        let execution = selected.map(|event| event.execution.clone());
        let mut states: Vec<DebuggerTimelineSubjectState> = Vec::new();
        if let (Some(cursor), Some(execution)) = (cursor, execution.as_ref()) {
            for event in self.events[..=cursor]
                .iter()
                .filter(|event| &event.execution == execution)
            {
                let state = DebuggerTimelineSubjectState {
                    subject: event.subject.clone(),
                    event: event.event.clone(),
                    sequence: event.sequence,
                    value: event.value.clone(),
                    fault_code: event.fault_code,
                };
                if let Some(existing) = states
                    .iter_mut()
                    .find(|state| state.subject == event.subject)
                {
                    *existing = state;
                } else {
                    states.push(state);
                }
            }
        }
        let watch_states = watches.map_or_else(Vec::new, |watches| {
            watches
                .watches
                .iter()
                .map(|watch| DebuggerTimelineWatchState {
                    subject: watch.subject.clone(),
                    historical: self.mode == DebuggerTimelineMode::Replay,
                    latest: cursor.and_then(|cursor| {
                        let execution = execution.as_ref()?;
                        self.events[..=cursor]
                            .iter()
                            .rev()
                            .find(|event| {
                                &event.execution == execution && event.subject == watch.subject
                            })
                            .map(DebuggerTimelineEvent::watch_entry)
                    }),
                })
                .collect()
        });
        DebuggerTimelineProjection {
            mode: self.mode,
            cursor,
            cursor_sequence: selected.map(|event| event.sequence),
            execution,
            exact_reconstruction: self.gap.is_none() && self.evicted_events == 0,
            states,
            watch_states,
        }
    }

    fn visible(
        &self,
        execution: DebugExecutionIdentity,
        subject: DebugSubject,
    ) -> Result<&str, DebuggerTimelineError> {
        self.bindings
            .iter()
            .find(|binding| binding.execution == execution && binding.runtime_subject == subject)
            .map(|binding| binding.visible_subject.as_str())
            .ok_or(DebuggerTimelineError::UnknownSubject)
    }
}

impl DebuggerTimelineEvent {
    pub fn retained_bytes(&self) -> usize {
        128 + self.subject.len()
            + self.related_subject.as_ref().map_or(0, String::len)
            + self.event.len()
            + self.value.as_ref().map_or(0, |value| value.summary.len())
    }

    fn watch_entry(&self) -> DebuggerWatchHistoryEntry {
        DebuggerWatchHistoryEntry {
            sequence: self.sequence,
            event: self.event.clone(),
            value: self.value.clone(),
            fault_code: self.fault_code,
        }
    }
}
