//! Bounded, revision-exact evolution of a [`CurrentExperience`](crate::CurrentExperience).

use alloc::{boxed::Box, vec::Vec};

use crate::{
    CurrentExperience, ExperienceItem, ExperienceRefusal, ExperienceRelation, ExperienceSourceRef,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExperienceRevision(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExperienceEvolutionLimits {
    pub maximum_pending_updates: usize,
    pub maximum_retained_revisions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExperienceUpdateOperation {
    Admit(Box<ExperienceItem>),
    Relate(ExperienceRelation),
    RemoveSource(ExperienceSourceRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceUpdate {
    pub expected_revision: ExperienceRevision,
    pub operation: ExperienceUpdateOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceRevisionSnapshot {
    pub revision: ExperienceRevision,
    pub experience: CurrentExperience,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperienceUpdateRefusal {
    InvalidLimits,
    PendingUpdateCapacity,
    StaleRevision,
    FutureRevision,
    RevisionOverflow,
    NoSemanticChange,
    Experience(ExperienceRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperienceUpdateError {
    pub refusal: ExperienceUpdateRefusal,
    pub update: Box<ExperienceUpdate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolvingExperience {
    limits: ExperienceEvolutionLimits,
    current: ExperienceRevisionSnapshot,
    pending: Vec<ExperienceUpdate>,
    retained: Vec<ExperienceRevisionSnapshot>,
}

impl EvolvingExperience {
    pub fn new(
        experience: CurrentExperience,
        limits: ExperienceEvolutionLimits,
    ) -> Result<Self, ExperienceUpdateRefusal> {
        if limits.maximum_pending_updates == 0 || limits.maximum_retained_revisions == 0 {
            return Err(ExperienceUpdateRefusal::InvalidLimits);
        }
        let current = ExperienceRevisionSnapshot {
            revision: ExperienceRevision(0),
            experience,
        };
        Ok(Self {
            pending: Vec::with_capacity(limits.maximum_pending_updates),
            retained: Vec::with_capacity(limits.maximum_retained_revisions),
            limits,
            current,
        })
    }

    pub fn current(&self) -> &ExperienceRevisionSnapshot {
        &self.current
    }

    pub fn pending(&self) -> &[ExperienceUpdate] {
        &self.pending
    }

    /// Completed prior outputs, from oldest retained to newest retained.
    pub fn retained(&self) -> &[ExperienceRevisionSnapshot] {
        &self.retained
    }

    pub fn try_enqueue(&mut self, update: ExperienceUpdate) -> Result<(), ExperienceUpdateError> {
        let next = self
            .current
            .revision
            .0
            .checked_add(self.pending.len() as u64)
            .ok_or_else(|| {
                update_error(ExperienceUpdateRefusal::RevisionOverflow, update.clone())
            })?;
        if update.expected_revision.0 < next {
            return Err(update_error(ExperienceUpdateRefusal::StaleRevision, update));
        }
        if update.expected_revision.0 > next {
            return Err(update_error(
                ExperienceUpdateRefusal::FutureRevision,
                update,
            ));
        }
        if self.pending.len() == self.limits.maximum_pending_updates {
            return Err(update_error(
                ExperienceUpdateRefusal::PendingUpdateCapacity,
                update,
            ));
        }
        self.pending.push(update);
        Ok(())
    }

    pub fn try_apply_next(&mut self) -> Result<Option<ExperienceRevision>, ExperienceUpdateError> {
        if self.pending.is_empty() {
            return Ok(None);
        }
        let update = self.pending.remove(0);
        if update.expected_revision != self.current.revision {
            return Err(update_error(ExperienceUpdateRefusal::StaleRevision, update));
        }

        let mut next_experience = self.current.experience.clone();
        match &update.operation {
            ExperienceUpdateOperation::Admit(item) => next_experience
                .try_admit((**item).clone())
                .map_err(|error| {
                update_error(
                    ExperienceUpdateRefusal::Experience(error.refusal),
                    update.clone(),
                )
            })?,
            ExperienceUpdateOperation::Relate(relation) => next_experience
                .relate(relation.clone())
                .map_err(|refusal| {
                    update_error(ExperienceUpdateRefusal::Experience(refusal), update.clone())
                })?,
            ExperienceUpdateOperation::RemoveSource(source) => {
                next_experience.remove_source(source);
                if next_experience == self.current.experience {
                    return Err(update_error(
                        ExperienceUpdateRefusal::NoSemanticChange,
                        update,
                    ));
                }
            }
        }

        let revision =
            ExperienceRevision(self.current.revision.0.checked_add(1).ok_or_else(|| {
                update_error(ExperienceUpdateRefusal::RevisionOverflow, update.clone())
            })?);
        let previous = core::mem::replace(
            &mut self.current,
            ExperienceRevisionSnapshot {
                revision,
                experience: next_experience,
            },
        );
        if self.retained.len() == self.limits.maximum_retained_revisions {
            self.retained.remove(0);
        }
        self.retained.push(previous);
        Ok(Some(revision))
    }
}

fn update_error(
    refusal: ExperienceUpdateRefusal,
    update: ExperienceUpdate,
) -> ExperienceUpdateError {
    ExperienceUpdateError {
        refusal,
        update: Box::new(update),
    }
}
