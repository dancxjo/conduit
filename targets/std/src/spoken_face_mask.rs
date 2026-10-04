//! A mechanical, interactive spoken reading of one exact Face and Show.
//!
//! The reader has no application state. It traverses the same provenanced
//! Face clauses used by other linear Masks, and emits typed interactions only
//! through the Show that offered the action. A selected speech Host effect must
//! acknowledge each bounded segment before the reader advances.

use std::collections::BTreeMap;

use conduit_presentation::{
    FaceInteraction, FaceInteractionArgument, FaceInteractionRefusal, FaceReadingCommand,
    FaceReadingCursor, FaceUtteranceClause, FaceUtterancePlan, FaceUtteranceProvenance, MaskShow,
    Presentation, PresentationActionAvailability, PresentationPropertyValue,
    PresentationRelationshipKind, PresentationRole, UTF8_TEXT_VALUE_KIND,
};
use conduit_tongues::{SpeakableSegment, SpeechCommitReason, MAXIMUM_SPEAKABLE_SEGMENT_BYTES};
use sha2::{Digest, Sha256};

mod speech;
pub use speech::*;
mod batch;
pub use batch::*;
mod voice;
use voice::voice_clauses;
mod reader_contract;
use reader_contract::{check_show, reading_refusal, Reading};
pub use reader_contract::{ReaderCommand, ReaderResult, SpokenFaceRefusal, SpokenTextReadout};
#[cfg(test)]
mod tests;

/// One bounded speech turn at a time. Only one segment may be in flight, so
/// producer pressure cannot turn an unacknowledged clip into a completed Show.
pub struct SpokenFaceSession {
    face: Presentation,
    show: MaskShow,
    cursor: FaceReadingCursor,
    voiced: Vec<String>,
    drafts: BTreeMap<(String, String), Vec<u8>>,
    reading: Option<Reading>,
    pending: Option<SpokenSegment>,
    pending_batch: Option<SpokenBatch>,
    completed_segments: u32,
    produced_pcm_bytes: u64,
    provider_sha256: Option<String>,
    correlation: Sha256,
    cancel_requested: bool,
    turn: u64,
    sequence: u32,
    batch: u32,
}

impl SpokenFaceSession {
    pub fn new(face: Presentation, show: MaskShow) -> Result<Self, SpokenFaceRefusal> {
        check_show(&face, &show)?;
        let cursor = FaceReadingCursor::new(&face).map_err(reading_refusal)?;
        let voiced = voice_clauses(&face, cursor.plan())?;
        Ok(Self {
            face,
            show,
            cursor,
            voiced,
            drafts: BTreeMap::new(),
            reading: None,
            pending: None,
            pending_batch: None,
            completed_segments: 0,
            produced_pcm_bytes: 0,
            provider_sha256: None,
            correlation: Sha256::new(),
            cancel_requested: false,
            turn: 0,
            sequence: 0,
            batch: 0,
        })
    }

    pub fn face(&self) -> &Presentation {
        &self.face
    }
    pub fn show(&self) -> &MaskShow {
        &self.show
    }
    pub fn focused_clause(&self) -> &FaceUtteranceClause {
        self.cursor.focused_clause()
    }
    pub fn focused_index(&self) -> usize {
        self.cursor.focused_index()
    }
    pub fn clause_count(&self) -> usize {
        self.cursor.clause_count()
    }
    #[cfg(test)]
    fn plan(&self) -> &FaceUtterancePlan {
        self.cursor.plan()
    }

    /// Consume the pending reading as text. This is a distinct output path
    /// from `next_segment`: it never acknowledges synthesis or playback.
    pub fn take_text_readout(&mut self) -> Result<Option<SpokenTextReadout>, SpokenFaceRefusal> {
        if self.pending.is_some() || self.pending_batch.is_some() {
            return Err(SpokenFaceRefusal::SpeechPressure);
        }
        let Some(reading) = self.reading.take() else {
            return Ok(None);
        };
        let clauses = match reading {
            Reading::Clauses { current, offset } => {
                let mut clauses = Vec::new();
                if let Some(index) = current {
                    clauses.push(self.voiced[index][offset..].to_owned());
                }
                while self
                    .cursor
                    .next_read_clause(&self.face)
                    .map_err(reading_refusal)?
                    .is_some()
                {
                    clauses.push(self.voiced[self.cursor.focused_index()].clone());
                }
                clauses
            }
            Reading::Message { text, offset } => vec![text[offset..].to_owned()],
        };
        Ok(Some(SpokenTextReadout {
            face_id: self.face.identity.as_str().into(),
            face_revision: self.face.revision,
            show_id: self.show.show_id.as_str().into(),
            clauses,
        }))
    }

    /// Replace the exact Face/Show after the producer has accepted a change.
    /// The old local focus survives only if its provenance still exists.
    pub fn refresh(&mut self, face: Presentation, show: MaskShow) -> Result<(), SpokenFaceRefusal> {
        if self.pending.is_some() || self.pending_batch.is_some() {
            return Err(SpokenFaceRefusal::SpeechPressure);
        }
        check_show(&face, &show)?;
        let next = FaceReadingCursor::new(&face).map_err(reading_refusal)?;
        let voiced = voice_clauses(&face, next.plan())?;
        self.cursor.replace(next);
        self.face = face;
        self.show = show;
        self.voiced = voiced;
        self.drafts.clear();
        self.cursor
            .command(&self.face, FaceReadingCommand::Repeat)
            .map_err(reading_refusal)?;
        self.begin_cursor_clauses();
        Ok(())
    }

    /// The caller must supply its *current* producer Face and currently
    /// acknowledged Show. A stale session cannot continue to act or speak.
    pub fn command(
        &mut self,
        current_face: &Presentation,
        current_show: &MaskShow,
        command: ReaderCommand,
        sequence: u64,
    ) -> Result<ReaderResult, SpokenFaceRefusal> {
        if command != ReaderCommand::Stop {
            self.check_current(current_face, current_show)?;
        }
        if (self.pending.is_some() || self.pending_batch.is_some())
            && command != ReaderCommand::Stop
        {
            return Err(SpokenFaceRefusal::SpeechPressure);
        }
        let mut interrupted = None;
        if self.reading.is_some() && command != ReaderCommand::Stop {
            self.reading = None;
            interrupted = Some(self.finish_turn(SpokenTurnOutcome::Cancelled));
        }
        let mut interaction = None;
        let mut cancel_stream_identity = None;
        match command {
            ReaderCommand::Help => self.begin_message("Enter one command per line. Type help to repeat this guide. Type read all for the current view; next, previous, or repeat to move. Type next action to find a control. Type focus followed by an offered action ID when you know it. Type edit followed by the announced argument name and new value, then type activate to apply it. Type stop to interrupt speech, or quit to leave.".into()),
            ReaderCommand::ReadAll | ReaderCommand::Next | ReaderCommand::Previous | ReaderCommand::Repeat | ReaderCommand::NextSubject | ReaderCommand::PreviousSubject | ReaderCommand::NextAction | ReaderCommand::PreviousAction | ReaderCommand::NextRole(_) | ReaderCommand::PreviousRole(_) | ReaderCommand::FocusSubject(_) | ReaderCommand::FocusAction(_) => {
                let reading_command = match command {
                    ReaderCommand::ReadAll => FaceReadingCommand::ReadAll,
                    ReaderCommand::Next => FaceReadingCommand::Next,
                    ReaderCommand::Previous => FaceReadingCommand::Previous,
                    ReaderCommand::Repeat => FaceReadingCommand::Repeat,
                    ReaderCommand::NextSubject => FaceReadingCommand::NextSubject,
                    ReaderCommand::PreviousSubject => FaceReadingCommand::PreviousSubject,
                    ReaderCommand::NextAction => FaceReadingCommand::NextAction,
                    ReaderCommand::PreviousAction => FaceReadingCommand::PreviousAction,
                    ReaderCommand::NextRole(role) => FaceReadingCommand::NextRole(role),
                    ReaderCommand::PreviousRole(role) => FaceReadingCommand::PreviousRole(role),
                    ReaderCommand::FocusSubject(identity) => FaceReadingCommand::FocusSubject(identity),
                    ReaderCommand::FocusAction(identity) => FaceReadingCommand::FocusAction(identity),
                    _ => unreachable!(),
                };
                let moving_backward = matches!(reading_command,
                    FaceReadingCommand::Previous | FaceReadingCommand::PreviousSubject |
                    FaceReadingCommand::PreviousAction | FaceReadingCommand::PreviousRole(_));
                let outcome = self.cursor.command(&self.face, reading_command).map_err(reading_refusal)?;
                if outcome.at_boundary {
                    self.begin_message(if moving_backward {
                        "No previous matching item. Focus unchanged.".into()
                    } else {
                        "No next matching item. Focus unchanged.".into()
                    });
                } else {
                    self.begin_cursor_clauses();
                }
            }
            ReaderCommand::Stop => {
                self.reading = None;
                self.cursor.command(&self.face, FaceReadingCommand::Stop).map_err(reading_refusal)?;
                cancel_stream_identity = self
                    .pending
                    .as_ref()
                    .map(|packet| packet.segment.stream_identity.clone())
                    .or_else(|| self.pending_batch.as_ref().map(|batch| batch.stream_identity.clone()));
                self.cancel_requested = cancel_stream_identity.is_some();
                if cancel_stream_identity.is_none() {
                    interrupted = Some(self.finish_turn(SpokenTurnOutcome::Cancelled));
                }
            }
            ReaderCommand::Edit { argument, value } => {
                let action = self.focused_action().ok_or(SpokenFaceRefusal::NoActionInFocus)?;
                let declaration = action.arguments.iter().find(|item| item.name == argument)
                    .ok_or(SpokenFaceRefusal::UnknownArgument)?;
                if declaration.contract.value_kind.as_str() != UTF8_TEXT_VALUE_KIND
                    && declaration.contract.value_kind.as_str() != "value/bool" {
                    return Err(SpokenFaceRefusal::UnsupportedValueKind);
                }
                declaration.contract.validate(&value).map_err(|_| SpokenFaceRefusal::InvalidValue)?;
                let action_id = action.identity.clone();
                let value_name = declaration.value_name.clone();
                self.drafts.retain(|(owner, _), _| owner == &action_id);
                self.drafts.insert((action_id, argument), value);
                self.begin_message(format!("{value_name} is ready. Activate to apply it."));
            }
            ReaderCommand::Activate => {
                let action = self.focused_action().ok_or(SpokenFaceRefusal::NoActionInFocus)?;
                if !action.availability.is_available() {
                    return Err(SpokenFaceRefusal::Interaction(match action.availability {
                        PresentationActionAvailability::Refused { .. } => FaceInteractionRefusal::RefusedAction,
                        _ => FaceInteractionRefusal::UnavailableAction,
                    }));
                }
                let arguments = action.arguments.iter().map(|declaration| {
                    self.drafts.get(&(action.identity.clone(), declaration.name.clone())).map(|value| FaceInteractionArgument {
                        name: declaration.name.clone(), value_kind: declaration.contract.value_kind.as_str().into(), value: value.clone(),
                    }).ok_or(SpokenFaceRefusal::Interaction(FaceInteractionRefusal::MissingArgument))
                }).collect::<Result<Vec<_>, _>>()?;
                interaction = Some(FaceInteraction::new(&self.face, &self.show, &action.identity, &action.target, arguments, sequence)
                    .map_err(SpokenFaceRefusal::Interaction)?);
                let action_name = action.name.clone();
                self.drafts.clear();
                self.begin_message(format!("{action_name} requested. Waiting for the result."));
            }
        }
        Ok(ReaderResult {
            interaction,
            reading: self.reading.is_some(),
            focused_clause: self.cursor.focused_index(),
            cancel_stream_identity,
            interrupted,
        })
    }

    /// Demand-driven segmentation. The previous packet must have a terminal
    /// audio outcome before another one is offered. Read-all needs no giant
    /// assembled speech string or whole-view PCM allocation.
    pub fn next_segment(&mut self) -> Result<Option<SpokenSegment>, SpokenFaceRefusal> {
        self.next_segment_up_to(MAXIMUM_SPEAKABLE_SEGMENT_BYTES)
    }

    /// Use a smaller UTF-8 text budget when the selected speech Back has a
    /// tighter, provider-dependent audio-time budget. The default remains the
    /// portable maximum; neither limit predicts actual PCM duration.
    pub fn next_segment_up_to(
        &mut self,
        maximum_text_bytes: usize,
    ) -> Result<Option<SpokenSegment>, SpokenFaceRefusal> {
        if !(4..=MAXIMUM_SPEAKABLE_SEGMENT_BYTES).contains(&maximum_text_bytes) {
            return Err(SpokenFaceRefusal::InvalidValue);
        }
        if self.pending.is_some() || self.pending_batch.is_some() {
            return Err(SpokenFaceRefusal::SpeechPressure);
        }
        if matches!(self.reading, Some(Reading::Clauses { current: None, .. })) {
            if self
                .cursor
                .next_read_clause(&self.face)
                .map_err(reading_refusal)?
                .is_some()
            {
                if let Some(Reading::Clauses { current, .. }) = &mut self.reading {
                    *current = Some(self.cursor.focused_index());
                }
            } else {
                self.reading = None;
                return Ok(None);
            }
        }
        let Some(reading) = &mut self.reading else {
            return Ok(None);
        };
        let (text, clause_index, provenance, offset) = match reading {
            Reading::Clauses { current, offset } => {
                let index = current.expect("clause was obtained from the current Face cursor");
                let clause = &self.cursor.plan().clauses[index];
                (
                    &self.voiced[index],
                    Some(index),
                    Some(clause.provenance.clone()),
                    offset,
                )
            }
            Reading::Message { text, offset } => (&*text, None, None, offset),
        };
        let remaining = &text[*offset..];
        let cut = split_at_char_boundary(remaining, maximum_text_bytes);
        let part = remaining[..cut].to_string();
        *offset += cut;
        let finished_piece = *offset == text.len();
        match reading {
            Reading::Clauses { current, offset } if finished_piece => {
                *current = None;
                *offset = 0;
                if !self.cursor.has_pending() {
                    self.reading = None;
                }
            }
            Reading::Message { .. } if finished_piece => {
                self.reading = None;
            }
            _ => {}
        }
        if self.sequence == 32 {
            self.batch += 1;
            self.sequence = 0;
        }
        let stream_identity = format!(
            "face/{}/{}/turn/{}/batch/{}",
            self.face.identity.as_str(),
            self.face.revision,
            self.turn,
            self.batch
        );
        let reason = if self.reading.is_none() || self.sequence == 31 {
            SpeechCommitReason::FinalFlush
        } else {
            SpeechCommitReason::TonguesBoundary
        };
        let segment = SpeakableSegment::new(stream_identity, self.sequence, part.clone(), reason)
            .map_err(|_| SpokenFaceRefusal::InvalidValue)?;
        self.sequence += 1;
        let packet = SpokenSegment {
            face_id: self.face.identity.as_str().into(),
            face_revision: self.face.revision,
            show_id: self.show.show_id.as_str().into(),
            clause_index,
            clause_provenance: provenance,
            text_sha256: format!("{:x}", Sha256::digest(part.as_bytes())),
            segment,
        };
        self.pending = Some(packet.clone());
        Ok(Some(packet))
    }

    pub fn acknowledge(
        &mut self,
        delivery: SpokenDelivery,
    ) -> Result<Option<SpokenTurnReceipt>, SpokenFaceRefusal> {
        let packet = self
            .pending
            .take()
            .ok_or(SpokenFaceRefusal::SpeechReceipt)?;
        match delivery {
            SpokenDelivery::Completed(receipt) => {
                if receipt.stream_identity != packet.segment.stream_identity
                    || receipt.sequence != packet.segment.sequence
                    || receipt.source_show_id != packet.show_id
                    || receipt.speech_plan_id.is_empty()
                    || receipt.speech_play_id.is_empty()
                    || receipt.text_sha256 != packet.text_sha256
                    || receipt.pcm_bytes == 0
                    || receipt.pcm_bytes > conduit_tongues::MAXIMUM_STREAM_PCM_BYTES
                    || !sha256_hex(&receipt.provider_sha256)
                    || !sha256_hex(&receipt.pcm_sha256)
                {
                    self.pending = Some(packet);
                    return Err(SpokenFaceRefusal::SpeechReceipt);
                }
                if self
                    .provider_sha256
                    .as_ref()
                    .is_some_and(|provider| provider != &receipt.provider_sha256)
                {
                    self.pending = Some(packet);
                    return Err(SpokenFaceRefusal::SpeechReceipt);
                }
                self.provider_sha256 = Some(receipt.provider_sha256.clone());
                self.completed_segments += 1;
                self.produced_pcm_bytes += u64::from(receipt.pcm_bytes);
                self.correlation.update(receipt.stream_identity.as_bytes());
                self.correlation.update(receipt.sequence.to_le_bytes());
                self.correlation.update(receipt.source_show_id.as_bytes());
                self.correlation.update(receipt.speech_plan_id.as_bytes());
                self.correlation.update(receipt.speech_play_id.as_bytes());
                self.correlation.update(receipt.text_sha256.as_bytes());
                self.correlation.update(receipt.pcm_sha256.as_bytes());
                self.correlation.update(receipt.pcm_bytes.to_le_bytes());
                if self.reading.is_none() {
                    let outcome = if self.cancel_requested {
                        SpokenTurnOutcome::Cancelled
                    } else {
                        SpokenTurnOutcome::Completed
                    };
                    return Ok(Some(self.finish_turn(outcome)));
                }
                Ok(None)
            }
            SpokenDelivery::Cancelled => {
                self.reading = None;
                Ok(Some(self.finish_turn(SpokenTurnOutcome::Cancelled)))
            }
            SpokenDelivery::Failed(reason) => {
                self.reading = None;
                Ok(Some(self.finish_turn(SpokenTurnOutcome::Failed(reason))))
            }
        }
    }

    fn check_current(&self, face: &Presentation, show: &MaskShow) -> Result<(), SpokenFaceRefusal> {
        if face != &self.face {
            return Err(SpokenFaceRefusal::StaleFace);
        }
        if show != &self.show {
            return Err(SpokenFaceRefusal::StaleShow);
        }
        check_show(face, show)
    }
    fn focused_action(&self) -> Option<&conduit_presentation::PresentationAction> {
        let id = match &self.cursor.focused_clause().provenance {
            FaceUtteranceProvenance::Action(value) => value.identity(),
            FaceUtteranceProvenance::ActionArgument(value) => value.action_identity(),
            _ => return None,
        };
        self.face
            .actions
            .iter()
            .find(|action| action.identity == *id)
    }
    fn begin_cursor_clauses(&mut self) {
        self.begin_turn();
        self.reading = Some(Reading::Clauses {
            current: None,
            offset: 0,
        });
    }
    fn begin_message(&mut self, text: String) {
        self.cursor
            .command(&self.face, FaceReadingCommand::Stop)
            .expect("stopping a Face reading is always valid");
        self.begin_turn();
        self.reading = Some(Reading::Message { text, offset: 0 });
    }
    fn begin_turn(&mut self) {
        self.turn += 1;
        self.sequence = 0;
        self.batch = 0;
        self.reset_correlation();
    }
    fn reset_correlation(&mut self) {
        self.completed_segments = 0;
        self.produced_pcm_bytes = 0;
        self.provider_sha256 = None;
        self.correlation = Sha256::new();
        self.cancel_requested = false;
    }
    fn finish_turn(&mut self, outcome: SpokenTurnOutcome) -> SpokenTurnReceipt {
        self.cursor
            .command(&self.face, FaceReadingCommand::Stop)
            .expect("stopping a Face reading is always valid");
        SpokenTurnReceipt {
            face_id: self.face.identity.as_str().into(),
            face_revision: self.face.revision,
            show_id: self.show.show_id.as_str().into(),
            completed_segments: std::mem::take(&mut self.completed_segments),
            produced_pcm_bytes: std::mem::take(&mut self.produced_pcm_bytes),
            provider_sha256: self.provider_sha256.take(),
            correlation_sha256: format!("{:x}", std::mem::take(&mut self.correlation).finalize()),
            outcome,
        }
    }
}

fn sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
