//! Finite presentation of scheduler-owned debugger execution control.

use crate::DebuggerExecutionIdentity;
use serde::{Deserialize, Serialize};

pub const DEBUGGER_CONTROL_SCHEMA: &str = "conduit.patchbay.debugger-control/v1";
pub const MAX_DEBUGGER_BREAKPOINT_SUBJECTS: usize = 16;
pub const MAX_DEBUGGER_CONTROL_REASON_BYTES: usize = 160;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DebuggerExecutionControlState {
    Running,
    Suspended,
    Stale,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerExecutionControl {
    pub schema: String,
    pub revision: u64,
    pub execution: DebuggerExecutionIdentity,
    pub state: DebuggerExecutionControlState,
    pub eligible_subjects: Vec<String>,
    pub breakpoint_subject: Option<String>,
    pub suspended_subject: Option<String>,
    pub reason: Option<String>,
}

impl DebuggerExecutionControl {
    pub fn new(execution: DebuggerExecutionIdentity, eligible_subjects: Vec<String>) -> Self {
        Self {
            schema: DEBUGGER_CONTROL_SCHEMA.into(),
            revision: 0,
            execution,
            state: DebuggerExecutionControlState::Running,
            eligible_subjects,
            breakpoint_subject: None,
            suspended_subject: None,
            reason: None,
        }
    }

    pub fn suspended(&mut self, subject: &str) {
        self.revision = self.revision.saturating_add(1);
        self.state = DebuggerExecutionControlState::Suspended;
        self.breakpoint_subject = Some(subject.to_owned());
        self.suspended_subject = Some(subject.to_owned());
        self.reason = None;
    }

    pub fn resumed(&mut self) {
        self.revision = self.revision.saturating_add(1);
        self.state = DebuggerExecutionControlState::Running;
        self.breakpoint_subject = None;
        self.suspended_subject = None;
        self.reason = None;
    }

    pub fn replace_execution(&mut self, execution: DebuggerExecutionIdentity) {
        if execution == self.execution {
            return;
        }
        self.revision = self.revision.saturating_add(1);
        self.state = DebuggerExecutionControlState::Stale;
        self.breakpoint_subject = None;
        self.suspended_subject = None;
        self.reason =
            Some("exact Body/Plan/Play execution was replaced; breakpoint was not remapped".into());
    }
}
