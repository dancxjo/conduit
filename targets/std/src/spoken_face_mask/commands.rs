//! Commands over one exact acknowledged Face and its bounded reading cursors.
use super::*;
impl SpokenFaceSession {
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
        let mut interaction = None;
        let mut cancel_stream_identity = None;
        match command {
            ReaderCommand::Help => {
                if self.reading.take().is_some() {
                    interrupted = Some(self.finish_turn(SpokenTurnOutcome::Cancelled));
                }
                self.begin_message("Enter one command per line. Type help to repeat this guide. Type summary for the useful overview. Type read remaining for three current items, then more items to continue. Type read current items for the complete primary-item stream. Type read all for the complete view; next, previous, or repeat to move. Type next action to find a control. Type focus followed by an offered action ID when you know it. Type edit followed by the announced argument name and new value, then type activate to apply it. Type stop to interrupt speech, or quit to leave.".into());
            }
            ReaderCommand::Summary => {
                let wording = primary_face_clauses(&self.face)?.join(" ");
                if wording.len() > 4_096 {
                    return Err(SpokenFaceRefusal::VoiceBound);
                }
                if self.reading.take().is_some() {
                    interrupted = Some(self.finish_turn(SpokenTurnOutcome::Cancelled));
                }
                self.begin_message(wording);
            }
            ReaderCommand::ReadItemPage | ReaderCommand::MoreItems => {
                let mut cursor = if command == ReaderCommand::ReadItemPage {
                    SpokenItemCursor::new(&self.face)?
                } else {
                    self.item_cursor
                        .clone()
                        .ok_or(SpokenFaceRefusal::InvalidValue)?
                };
                let page = cursor.next_page(&self.face)?;
                if command == ReaderCommand::MoreItems && self.reading.is_some() {
                    return Err(SpokenFaceRefusal::SpeechPressure);
                }
                let mut clauses = page.clauses;
                if clauses.is_empty() {
                    clauses.push("No more current items.".into());
                }
                if page.more {
                    clauses.push("Type more items to continue.".into());
                }
                if self.reading.take().is_some() {
                    interrupted = Some(self.finish_turn(SpokenTurnOutcome::Cancelled));
                }
                self.begin_message(clauses.join(" "));
                self.item_cursor = Some(cursor);
            }
            ReaderCommand::ReadAll
            | ReaderCommand::ReadCurrentItems
            | ReaderCommand::Next
            | ReaderCommand::Previous
            | ReaderCommand::Repeat
            | ReaderCommand::NextSubject
            | ReaderCommand::PreviousSubject
            | ReaderCommand::NextAction
            | ReaderCommand::PreviousAction
            | ReaderCommand::NextRole(_)
            | ReaderCommand::PreviousRole(_)
            | ReaderCommand::FocusSubject(_)
            | ReaderCommand::FocusAction(_) => {
                let reading_command = match command {
                    ReaderCommand::ReadAll => FaceReadingCommand::ReadAll,
                    ReaderCommand::ReadCurrentItems => FaceReadingCommand::ReadRoleAtDisclosure(
                        PresentationRole::Item,
                        conduit_presentation::PresentationDisclosureLevel::Primary,
                    ),
                    ReaderCommand::Next => FaceReadingCommand::Next,
                    ReaderCommand::Previous => FaceReadingCommand::Previous,
                    ReaderCommand::Repeat => FaceReadingCommand::Repeat,
                    ReaderCommand::NextSubject => FaceReadingCommand::NextSubject,
                    ReaderCommand::PreviousSubject => FaceReadingCommand::PreviousSubject,
                    ReaderCommand::NextAction => FaceReadingCommand::NextAction,
                    ReaderCommand::PreviousAction => FaceReadingCommand::PreviousAction,
                    ReaderCommand::NextRole(role) => FaceReadingCommand::NextRole(role),
                    ReaderCommand::PreviousRole(role) => FaceReadingCommand::PreviousRole(role),
                    ReaderCommand::FocusSubject(identity) => {
                        FaceReadingCommand::FocusSubject(identity)
                    }
                    ReaderCommand::FocusAction(identity) => {
                        FaceReadingCommand::FocusAction(identity)
                    }
                    _ => unreachable!(),
                };
                let moving_backward = matches!(
                    reading_command,
                    FaceReadingCommand::Previous
                        | FaceReadingCommand::PreviousSubject
                        | FaceReadingCommand::PreviousAction
                        | FaceReadingCommand::PreviousRole(_)
                );
                let reading_current_items = matches!(
                    reading_command,
                    FaceReadingCommand::ReadRoleAtDisclosure(
                        PresentationRole::Item,
                        conduit_presentation::PresentationDisclosureLevel::Primary
                    )
                );
                let outcome = self
                    .cursor
                    .command(&self.face, reading_command)
                    .map_err(reading_refusal)?;
                // A refused focus must leave the existing turn and its cursor
                // intact. Once navigation succeeds, retire only the old speech
                // turn; stopping the cursor here would discard its new range.
                if self.reading.take().is_some() {
                    interrupted = Some(self.finish_turn_receipt(SpokenTurnOutcome::Cancelled));
                }
                if outcome.at_boundary {
                    self.begin_message(if reading_current_items {
                        "No current items.".into()
                    } else if moving_backward {
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
                self.cursor
                    .command(&self.face, FaceReadingCommand::Stop)
                    .map_err(reading_refusal)?;
                cancel_stream_identity = self
                    .pending
                    .as_ref()
                    .map(|packet| packet.segment.stream_identity.clone())
                    .or_else(|| {
                        self.pending_batch
                            .as_ref()
                            .map(|batch| batch.stream_identity.clone())
                    });
                self.cancel_requested = cancel_stream_identity.is_some();
                if cancel_stream_identity.is_none() {
                    interrupted = Some(self.finish_turn(SpokenTurnOutcome::Cancelled));
                }
            }
            ReaderCommand::Edit { argument, value } => {
                let (action_id, value_name) = self.validated_edit(&argument, &value)?;
                if self.reading.take().is_some() {
                    interrupted = Some(self.finish_turn(SpokenTurnOutcome::Cancelled));
                }
                self.drafts.retain(|(owner, _), _| owner == &action_id);
                self.drafts.insert((action_id, argument), value);
                self.begin_message(format!("{value_name} is ready. Activate to apply it."));
            }
            ReaderCommand::Activate => {
                let (accepted, action_name) = self.validated_activation(sequence)?;
                interaction = Some(accepted);
                if self.reading.take().is_some() {
                    interrupted = Some(self.finish_turn(SpokenTurnOutcome::Cancelled));
                }
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
}
