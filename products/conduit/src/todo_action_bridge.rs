//! One Mask-neutral, finite ingress from exact Todo Face actions to a live
//! Body Fore queue. Queue acceptance is Host staging, never a state mutation.

use conduit_core::{ActivePlayId, TerminalDisposition};
use conduit_presentation::{
    FaceEvidenceAckRefusal, FaceInteraction, FaceInteractionDisposition, FaceInteractionEvidence,
    FaceInteractionId, FaceInteractionLedger, FaceInteractionRefusal, MaskShow, Presentation,
    MAX_INTERACTION_LEDGER_ADMISSIONS,
};
use conduit_std_host::body_execution::BodyRunReport;
use conduit_std_host::{BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery};
use conduit_todo_face::{todo_command_from_interaction, TodoFaceError};
use conduit_todo_plot::{TodoRefusal, TodoState};

/// The only required effect is the exact already-admitted std Fore queue.
/// A small fake may exercise the bridge without inventing a second Body.
pub trait TodoForeSubmission {
    fn submit(&self, canonical: &[u8]) -> Result<BodyLiveForeAdmission, String>;
}

impl TodoForeSubmission for BodyLiveForeQueue {
    fn submit(&self, canonical: &[u8]) -> Result<BodyLiveForeAdmission, String> {
        BodyLiveForeQueue::submit(self, canonical)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoActionBridgeRefusal {
    Face(TodoFaceError),
    Ledger(FaceInteractionRefusal),
    Encode(TodoRefusal),
    Queue(String),
    PlayNotBound,
    StalePlay,
    MissingLiveReport,
    InconsistentReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoQueueReceipt {
    pub interaction_id: FaceInteractionId,
    pub face_id: String,
    pub show_id: String,
    pub action_id: String,
    /// Accepted into Host staging; the Body may not have admitted it yet.
    pub queue_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoActionAdmission {
    Accepted(TodoQueueReceipt),
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TodoActionObservation {
    /// No matching delivered Fore output and child Signs were observed.
    Staged(TodoQueueReceipt),
    /// Actual Body output and a correlated child Sign were both retained.
    /// This is not a durable checkpoint or listener acknowledgement.
    Produced {
        staged: TodoQueueReceipt,
        parent_active_play_id: ActivePlayId,
        child_active_play_id: ActivePlayId,
        state: TodoState,
    },
}

/// Owns no Todo state. All Face truth is supplied by the owner on each call,
/// and one exact Body output may be decoded only when a report is observed.
pub struct TodoActionBridge<Q = BodyLiveForeQueue> {
    queue: Q,
    ledger: FaceInteractionLedger,
    accepted: Vec<TodoQueueReceipt>,
    parent_play: Option<ActivePlayId>,
}

impl<Q: TodoForeSubmission> TodoActionBridge<Q> {
    pub fn new(queue: Q, maximum_evidence: usize) -> Result<Self, FaceInteractionRefusal> {
        Ok(Self {
            queue,
            ledger: FaceInteractionLedger::new(1, maximum_evidence)?,
            accepted: Vec::with_capacity(MAX_INTERACTION_LEDGER_ADMISSIONS),
            parent_play: None,
        })
    }

    /// Bind the exact parent Play published by the std start callback before
    /// any Mask may stage an action. A bridge never follows a replacement Play.
    pub fn bind_parent_play(
        &mut self,
        active_play_id: ActivePlayId,
    ) -> Result<(), TodoActionBridgeRefusal> {
        match &self.parent_play {
            Some(current) if current != &active_play_id => Err(TodoActionBridgeRefusal::StalePlay),
            Some(_) => Ok(()),
            None => {
                self.parent_play = Some(active_play_id);
                Ok(())
            }
        }
    }

    pub fn submit(
        &mut self,
        state: &TodoState,
        face: &Presentation,
        show: &MaskShow,
        interaction: FaceInteraction,
    ) -> Result<TodoActionAdmission, TodoActionBridgeRefusal> {
        let parent_play = self
            .parent_play
            .as_ref()
            .ok_or(TodoActionBridgeRefusal::PlayNotBound)?;
        if face.basis.active_play_id.as_ref() != Some(parent_play) {
            return Err(TodoActionBridgeRefusal::StalePlay);
        }
        let command = todo_command_from_interaction(state, face, show, &interaction)
            .map_err(TodoActionBridgeRefusal::Face)?;
        let canonical = command
            .encode_info()
            .map_err(TodoActionBridgeRefusal::Encode)?;
        self.ledger
            .check_admit(&interaction)
            .map_err(TodoActionBridgeRefusal::Ledger)?;
        let admission = self
            .queue
            .submit(&canonical)
            .map_err(TodoActionBridgeRefusal::Queue)?;
        let BodyLiveForeAdmission::Accepted { sequence } = admission else {
            return Ok(TodoActionAdmission::Full);
        };
        let receipt = TodoQueueReceipt {
            interaction_id: interaction.identity.clone(),
            face_id: interaction.face_id.clone(),
            show_id: interaction.show_id.clone(),
            action_id: interaction.action_id.clone(),
            queue_sequence: sequence,
        };
        // Exclusive ledger ownership and check_admit above make both steps
        // infallible. The operation ID is constructed within the finite limit.
        self.ledger
            .admit(interaction)
            .expect("checked ledger admission changed without an intervening mutation");
        self.ledger
            .finish_front(FaceInteractionDisposition::Accepted {
                operation_request_id: format!("todo/fore/{sequence}"),
            })
            .expect("checked evidence capacity changed without an intervening mutation");
        self.accepted.push(receipt.clone());
        Ok(TodoActionAdmission::Accepted(receipt))
    }

    pub fn evidence(&self) -> &[FaceInteractionEvidence] {
        self.ledger.evidence()
    }

    /// A caller must retain these complete receipts before acknowledging them.
    pub fn acknowledge_persisted_evidence_prefix(
        &mut self,
        expected: &[FaceInteractionId],
    ) -> Result<(), FaceEvidenceAckRefusal> {
        self.ledger.acknowledge_persisted_evidence_prefix(expected)
    }

    pub fn accepted(&self) -> &[TodoQueueReceipt] {
        &self.accepted
    }

    /// Correlate the actual terminal Body report without treating queue
    /// acceptance, kernel ingress, or an output alone as committed Todo truth.
    pub fn observe_report(
        &self,
        report: &BodyRunReport,
    ) -> Result<Vec<TodoActionObservation>, TodoActionBridgeRefusal> {
        let status = report
            .live_fore_status
            .ok_or(TodoActionBridgeRefusal::MissingLiveReport)?;
        if self.parent_play.as_ref() != Some(&report.play.active_play_id) {
            return Err(TodoActionBridgeRefusal::StalePlay);
        }
        if usize::from(status.queue_accepted) < self.accepted.len() {
            return Err(TodoActionBridgeRefusal::InconsistentReport);
        }
        let child_signs = report
            .scan_child_signs
            .as_ref()
            .map_err(|_| TodoActionBridgeRefusal::InconsistentReport)?;
        let mut observations = Vec::with_capacity(self.accepted.len());
        for receipt in &self.accepted {
            let output = unique_sequence(&report.fore_deliveries, receipt.queue_sequence)?;
            let child = child_signs
                .iter()
                .filter(|child| {
                    u64::from(child.invocation) == receipt.queue_sequence
                        && child.parent_active_play_id == report.play.active_play_id
                })
                .collect::<Vec<_>>();
            if child.len() > 1 || output.is_some() != !child.is_empty() {
                return Err(TodoActionBridgeRefusal::InconsistentReport);
            }
            observations.push(match (output, child.first()) {
                (Some(output), Some(child)) => TodoActionObservation::Produced {
                    staged: receipt.clone(),
                    parent_active_play_id: report.play.active_play_id.clone(),
                    child_active_play_id: child.child_active_play_id.clone(),
                    state: TodoState::decode_info(&output.bytes)
                        .map_err(TodoActionBridgeRefusal::Encode)?,
                },
                (None, None) => TodoActionObservation::Staged(receipt.clone()),
                _ => return Err(TodoActionBridgeRefusal::InconsistentReport),
            });
        }
        // Completed means all accepted commands produced an output. Other
        // terminal dispositions may legitimately leave a staged suffix.
        if report.terminal == TerminalDisposition::Completed
            && observations
                .iter()
                .any(|item| matches!(item, TodoActionObservation::Staged(_)))
        {
            return Err(TodoActionBridgeRefusal::InconsistentReport);
        }
        Ok(observations)
    }
}

fn unique_sequence(
    deliveries: &[ExternalForeDelivery],
    sequence: u64,
) -> Result<Option<&ExternalForeDelivery>, TodoActionBridgeRefusal> {
    let mut matched = deliveries.iter().filter(|item| item.sequence == sequence);
    let first = matched.next();
    if matched.next().is_some() {
        return Err(TodoActionBridgeRefusal::InconsistentReport);
    }
    Ok(first)
}

#[cfg(test)]
#[path = "todo_action_bridge/tests.rs"]
mod tests;
