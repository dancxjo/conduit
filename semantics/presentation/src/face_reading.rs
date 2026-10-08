//! A bounded, deterministic reading cursor over one exact Face revision.
//!
//! This is navigation over the complete semantic Face, independent of a
//! graphical viewport or speech device. A Mask decides how to present each
//! clause and must separately validate its current Show before an action.

use alloc::string::String;

use crate::{
    plan_face_utterances, FaceUtteranceClause, FaceUtteranceClauseKind, FaceUtterancePlan,
    FaceUtterancePlanError, FaceUtteranceProvenance, Presentation, PresentationDisclosureLevel,
    PresentationRole,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceReadingCommand {
    ReadAll,
    /// Read only subjects matching an exact semantic role and disclosure.
    ReadRoleAtDisclosure(PresentationRole, PresentationDisclosureLevel),
    Next,
    Previous,
    Repeat,
    NextSubject,
    PreviousSubject,
    /// Jump among subjects carrying an exact structural or open semantic role.
    NextRole(PresentationRole),
    PreviousRole(PresentationRole),
    NextAction,
    PreviousAction,
    FocusSubject(String),
    FocusAction(String),
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaceReadingRefusal {
    InvalidFace(FaceUtterancePlanError),
    EmptyFace,
    StaleFace,
    UnknownSubject,
    UnknownAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceReadingOutcome {
    pub focused_clause: usize,
    pub reading: bool,
    pub interrupted: bool,
    pub at_boundary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceReadingRefresh {
    pub focused_clause: usize,
    /// Indexed facts cannot be presumed to retain their meaning across Face
    /// revisions. Stable subject and action identities can retain focus.
    pub retained_focus: bool,
    pub interrupted: bool,
}

/// A cursor and a finite pending selection, never a second copy of Face truth.
/// The retained plan is the existing provenance-bearing aural projection.
#[derive(Debug)]
enum PendingReading {
    Range {
        next: usize,
        end: usize,
    },
    Filtered {
        next: usize,
        role: PresentationRole,
        level: PresentationDisclosureLevel,
    },
}

#[derive(Debug)]
pub struct FaceReadingCursor {
    plan: FaceUtterancePlan,
    focus: usize,
    pending: Option<PendingReading>,
}

impl FaceReadingCursor {
    pub fn new(face: &Presentation) -> Result<Self, FaceReadingRefusal> {
        let plan = plan_face_utterances(face).map_err(FaceReadingRefusal::InvalidFace)?;
        if plan.clauses.is_empty() {
            return Err(FaceReadingRefusal::EmptyFace);
        }
        Ok(Self {
            plan,
            focus: 0,
            pending: None,
        })
    }

    pub fn source_face_identity(&self) -> &str {
        &self.plan.source_face_identity
    }

    pub fn source_face_revision(&self) -> u64 {
        self.plan.source_face_revision
    }

    pub fn clause_count(&self) -> usize {
        self.plan.clauses.len()
    }

    pub fn plan(&self) -> &FaceUtterancePlan {
        &self.plan
    }

    pub fn focused_index(&self) -> usize {
        self.focus
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn focused_clause(&self) -> &FaceUtteranceClause {
        &self.plan.clauses[self.focus]
    }

    pub fn command(
        &mut self,
        current_face: &Presentation,
        command: FaceReadingCommand,
    ) -> Result<FaceReadingOutcome, FaceReadingRefusal> {
        if command != FaceReadingCommand::Stop {
            self.check_current(current_face)?;
        }
        // Refused focus requests must leave the current reading intact.
        let exact_focus = match &command {
            FaceReadingCommand::FocusSubject(identity) => Some(
                self.plan
                    .clauses
                    .iter()
                    .position(|clause| {
                        matches!(&clause.provenance, FaceUtteranceProvenance::Subject(subject) if subject.identity() == identity)
                    })
                    .ok_or(FaceReadingRefusal::UnknownSubject)?,
            ),
            FaceReadingCommand::FocusAction(identity) => Some(
                self.plan
                    .clauses
                    .iter()
                    .position(|clause| {
                        matches!(&clause.provenance, FaceUtteranceProvenance::Action(action) if action.identity() == identity)
                    })
                    .ok_or(FaceReadingRefusal::UnknownAction)?,
            ),
            _ => None,
        };
        let interrupted = self.pending.take().is_some();
        let mut at_boundary = false;
        match command {
            FaceReadingCommand::ReadAll => {
                self.pending = Some(PendingReading::Range {
                    next: 0,
                    end: self.plan.clauses.len(),
                });
            }
            FaceReadingCommand::ReadRoleAtDisclosure(role, level) => {
                if let Some(next) = self.next_matching(current_face, 0, &role, level) {
                    self.pending = Some(PendingReading::Filtered { next, role, level });
                } else {
                    at_boundary = true;
                }
            }
            FaceReadingCommand::Next => {
                if self.focus + 1 == self.plan.clauses.len() {
                    at_boundary = true;
                } else {
                    self.focus += 1;
                }
                self.pending = Some(PendingReading::Range {
                    next: self.focus,
                    end: self.focus + 1,
                });
            }
            FaceReadingCommand::Previous => {
                if self.focus == 0 {
                    at_boundary = true;
                } else {
                    self.focus -= 1;
                }
                self.pending = Some(PendingReading::Range {
                    next: self.focus,
                    end: self.focus + 1,
                });
            }
            FaceReadingCommand::Repeat => {
                self.pending = Some(PendingReading::Range {
                    next: self.focus,
                    end: self.focus + 1,
                });
            }
            FaceReadingCommand::NextSubject => {
                at_boundary = self.move_by_kind(FaceUtteranceClauseKind::Subject, true);
            }
            FaceReadingCommand::PreviousSubject => {
                at_boundary = self.move_by_kind(FaceUtteranceClauseKind::Subject, false);
            }
            FaceReadingCommand::NextRole(role) => {
                at_boundary = self.move_by_role(current_face, &role, true);
            }
            FaceReadingCommand::PreviousRole(role) => {
                at_boundary = self.move_by_role(current_face, &role, false);
            }
            FaceReadingCommand::NextAction => {
                at_boundary = self.move_by_kind(FaceUtteranceClauseKind::Action, true);
            }
            FaceReadingCommand::PreviousAction => {
                at_boundary = self.move_by_kind(FaceUtteranceClauseKind::Action, false);
            }
            FaceReadingCommand::FocusSubject(_) | FaceReadingCommand::FocusAction(_) => {
                self.focus = exact_focus.expect("validated exact focus");
                self.pending = Some(PendingReading::Range {
                    next: self.focus,
                    end: self.focus + 1,
                });
            }
            FaceReadingCommand::Stop => {}
        }
        Ok(FaceReadingOutcome {
            focused_clause: self.focus,
            reading: self.pending.is_some(),
            interrupted,
            at_boundary,
        })
    }

    /// Yield one exact clause at a time. The caller can interrupt after any
    /// item; no viewport or prerecorded transcript limits the range.
    pub fn next_read_clause(
        &mut self,
        current_face: &Presentation,
    ) -> Result<Option<&FaceUtteranceClause>, FaceReadingRefusal> {
        self.check_current(current_face)?;
        let Some(pending) = self.pending.take() else {
            return Ok(None);
        };
        let next = match pending {
            PendingReading::Range { next, end } => {
                self.pending = (next + 1 < end).then_some(PendingReading::Range {
                    next: next + 1,
                    end,
                });
                next
            }
            PendingReading::Filtered { next, role, level } => {
                let Some(found) = self.next_matching(current_face, next, &role, level) else {
                    return Ok(None);
                };
                if found + 1 < self.plan.clauses.len() {
                    self.pending = Some(PendingReading::Filtered {
                        next: found + 1,
                        role,
                        level,
                    });
                }
                found
            }
        };
        self.focus = next;
        Ok(Some(&self.plan.clauses[next]))
    }

    /// Replace a Face only after its producer accepted the new revision.
    /// Any pending reading is interrupted. Indexed clauses reset to the start
    /// because their numerical provenance can refer to a different fact now.
    pub fn refresh(
        &mut self,
        face: &Presentation,
    ) -> Result<FaceReadingRefresh, FaceReadingRefusal> {
        Ok(self.replace(Self::new(face)?))
    }

    /// Install an already validated and prepared reading cursor. This avoids
    /// rebuilding a large Face when a Mask also prepares spoken wording.
    pub fn replace(&mut self, next: Self) -> FaceReadingRefresh {
        let old = &self.plan.clauses[self.focus].provenance;
        let stable = matches!(
            old,
            FaceUtteranceProvenance::Subject(_) | FaceUtteranceProvenance::Action(_)
        );
        let retained = stable
            .then(|| {
                next.plan
                    .clauses
                    .iter()
                    .position(|clause| &clause.provenance == old)
            })
            .flatten();
        let interrupted = self.pending.take().is_some();
        self.focus = retained.unwrap_or(0);
        self.plan = next.plan;
        FaceReadingRefresh {
            focused_clause: self.focus,
            retained_focus: retained.is_some(),
            interrupted,
        }
    }

    fn check_current(&self, face: &Presentation) -> Result<(), FaceReadingRefusal> {
        if self.plan.source_face_identity != face.identity.as_str()
            || self.plan.source_face_revision != face.revision
        {
            return Err(FaceReadingRefusal::StaleFace);
        }
        Ok(())
    }

    fn next_matching(
        &self,
        face: &Presentation,
        from: usize,
        role: &PresentationRole,
        level: PresentationDisclosureLevel,
    ) -> Option<usize> {
        (from..self.plan.clauses.len()).find(|index| {
            let FaceUtteranceProvenance::Subject(source) = &self.plan.clauses[*index].provenance
            else {
                return false;
            };
            face.subjects
                .iter()
                .any(|subject| subject.identity == *source.identity() && &subject.role == role)
                && face.disclosures.iter().any(|disclosure| {
                    disclosure.subject == *source.identity() && disclosure.level == level
                })
        })
    }

    /// Return true only when there is no next/previous anchor of this kind.
    fn move_by_kind(&mut self, kind: FaceUtteranceClauseKind, forward: bool) -> bool {
        self.move_where(forward, |clause| clause.kind == kind)
    }

    fn move_by_role(
        &mut self,
        face: &Presentation,
        role: &PresentationRole,
        forward: bool,
    ) -> bool {
        self.move_where(forward, |clause| match &clause.provenance {
            FaceUtteranceProvenance::Subject(subject) => face.subjects.iter().any(|candidate| {
                candidate.identity == *subject.identity() && &candidate.role == role
            }),
            _ => false,
        })
    }

    fn move_where(
        &mut self,
        forward: bool,
        matches: impl Fn(&FaceUtteranceClause) -> bool,
    ) -> bool {
        let found = if forward {
            (self.focus + 1..self.plan.clauses.len())
                .find(|index| matches(&self.plan.clauses[*index]))
        } else {
            (0..self.focus)
                .rev()
                .find(|index| matches(&self.plan.clauses[*index]))
        };
        match found {
            Some(index) => {
                self.focus = index;
                self.pending = Some(PendingReading::Range {
                    next: index,
                    end: index + 1,
                });
                false
            }
            None => true,
        }
    }
}
