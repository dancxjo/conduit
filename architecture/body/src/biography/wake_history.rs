use super::*;
use crate::{BodyState, Wake, WakeLifecycleEvent};

impl BodyBiographyEvidence {
    /// Retain an exact extension of one Wake and its Body lifecycle atomically.
    /// Workload changes must already have their own biography records. Execution
    /// termination is not a Lull: callers must supply that distinct transition.
    pub fn append_wake(
        &mut self,
        body: Body,
        wake: Wake,
        first_sequence: u64,
    ) -> Result<(), BodyBiographyError> {
        self.validate()?;
        if body.body_id != self.body_id || wake.body_id != self.body_id {
            return Err(BodyBiographyError::WrongBody);
        }
        body.validate()
            .map_err(|_| BodyBiographyError::InvalidBody)?;
        wake.validate()
            .map_err(|_| BodyBiographyError::InvalidEvidence)?;
        if !body.events.starts_with(&self.body.events) {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let position = self
            .wakes
            .iter()
            .position(|old| old.wake_id == wake.wake_id);
        let prior_len = if let Some(index) = position {
            let old = &self.wakes[index];
            if !wake.events.starts_with(&old.events)
                || wake.plans.len() < old.plans.len()
                || old.plans.iter().zip(&wake.plans).any(|(prior, next)| {
                    prior.plan_id != next.plan_id
                        || prior.hold != next.hold
                        || prior
                            .active_play_id
                            .as_ref()
                            .is_some_and(|id| next.active_play_id.as_ref() != Some(id))
                })
                || (wake.events == old.events && &wake != old)
            {
                return Err(BodyBiographyError::InvalidEvidence);
            }
            old.events.len()
        } else {
            if self.wakes.len() >= MAX_BODY_BIOGRAPHY_WAKES {
                return Err(BodyBiographyError::CapacityExhausted);
            }
            0
        };
        let mut additions = Vec::new();
        for (index, event) in wake.events.iter().enumerate().skip(prior_len) {
            additions.push((
                event.sign_id().clone(),
                BodyBiographyRecordKind::WakeEvent {
                    wake_id: wake.wake_id.clone(),
                    event_index: u16::try_from(index)
                        .map_err(|_| BodyBiographyError::CapacityExhausted)?,
                },
            ));
        }
        for event in body.events.iter().skip(self.body.events.len()) {
            match event {
                BodyLifecycleEvent::Woke { wake_id, .. } if wake_id == &wake.wake_id => {}
                BodyLifecycleEvent::LullRetained { wake_id, sign_id }
                    if wake_id == &wake.wake_id =>
                {
                    additions.push((
                        sign_id.clone(),
                        BodyBiographyRecordKind::LullRetained {
                            wake_id: wake_id.clone(),
                        },
                    ));
                }
                _ => return Err(BodyBiographyError::InvalidEvidence),
            }
        }
        if additions.is_empty() {
            return Err(BodyBiographyError::DuplicateEvidence);
        }
        self.can_append(additions.len())?;
        if first_sequence <= self.last_sequence() {
            return Err(BodyBiographyError::InvalidSequence);
        }
        let mut candidate = self.clone();
        candidate.body = body;
        if let Some(index) = position {
            candidate.wakes[index] = wake;
        } else {
            candidate.wakes.push(wake);
        }
        for (offset, (sign_id, kind)) in additions.into_iter().enumerate() {
            candidate.records.push(BodyBiographyRecord {
                sequence: first_sequence
                    .checked_add(offset as u64)
                    .ok_or(BodyBiographyError::InvalidSequence)?,
                sign_id,
                kind,
            });
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub(super) fn validate_wake_history(&self) -> Result<(), BodyBiographyError> {
        let invalid = |_| BodyBiographyError::InvalidEvidence;
        if self.wakes.len() > MAX_BODY_BIOGRAPHY_WAKES {
            return Err(BodyBiographyError::CapacityExhausted);
        }
        for (index, wake) in self.wakes.iter().enumerate() {
            wake.validate().map_err(invalid)?;
            if wake.body_id != self.body_id
                || self.wakes[..index]
                    .iter()
                    .any(|old| old.wake_id == wake.wake_id)
            {
                return Err(BodyBiographyError::InvalidEvidence);
            }
        }
        let BodyLifecycleEvent::Born {
            initial_workset,
            sign_id,
            ..
        } = &self.body.events[0]
        else {
            return Err(BodyBiographyError::InvalidEvidence);
        };
        let mut body = Body::born_with_forms(
            initial_workset.clone(),
            self.body.birth_sequence,
            sign_id.clone(),
        )
        .map_err(invalid)?;
        let mut consumed = vec![0usize; self.wakes.len()];
        let mut begun = 0usize;
        for record in self.records.iter().skip(1) {
            match &record.kind {
                BodyBiographyRecordKind::FormAdmitted {
                    source_document_id,
                    checked_form_id,
                    ..
                } => {
                    body = body
                        .admit_form(
                            crate::ResidentForm::new(
                                source_document_id.clone(),
                                checked_form_id.clone(),
                            ),
                            record.sign_id.clone(),
                        )
                        .map_err(invalid)?;
                }
                BodyBiographyRecordKind::FormRemoved {
                    source_document_id,
                    checked_form_id,
                    ..
                } => {
                    body = body
                        .remove_form(
                            &crate::ResidentForm::new(
                                source_document_id.clone(),
                                checked_form_id.clone(),
                            ),
                            record.sign_id.clone(),
                        )
                        .map_err(invalid)?;
                }
                BodyBiographyRecordKind::WakeEvent {
                    wake_id,
                    event_index,
                } => {
                    let index = self
                        .wakes
                        .iter()
                        .position(|wake| &wake.wake_id == wake_id)
                        .ok_or(BodyBiographyError::InvalidEvidence)?;
                    let wake = &self.wakes[index];
                    let event = wake
                        .events
                        .get(usize::from(*event_index))
                        .ok_or(BodyBiographyError::InvalidEvidence)?;
                    if consumed[index] != usize::from(*event_index)
                        || event.sign_id() != &record.sign_id
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                    if *event_index == 0 {
                        if index != begun {
                            return Err(BodyBiographyError::InvalidEvidence);
                        }
                        let (next, initial) = body
                            .wake(wake.wake_sequence, record.sign_id.clone())
                            .map_err(invalid)?;
                        let (workset, revision) = wake
                            .events
                            .iter()
                            .find_map(|event| match event {
                                WakeLifecycleEvent::WorkloadChanged {
                                    prior_workset,
                                    prior_workload_revision,
                                    ..
                                } => Some((prior_workset, *prior_workload_revision)),
                                _ => None,
                            })
                            .unwrap_or((&wake.workset, wake.workload_revision));
                        if initial.wake_id != wake.wake_id
                            || &initial.workset != workset
                            || initial.workload_revision != revision
                        {
                            return Err(BodyBiographyError::InvalidEvidence);
                        }
                        body = next;
                        begun += 1;
                    }
                    if !matches!(&body.state, BodyState::Awake { wake_id: id } if id == wake_id) {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                    if let WakeLifecycleEvent::WorkloadChanged {
                        replacement_workset,
                        replacement_workload_revision,
                        ..
                    } = event
                    {
                        if replacement_workset != &body.workset
                            || *replacement_workload_revision != body.workload_revision
                        {
                            return Err(BodyBiographyError::InvalidEvidence);
                        }
                    }
                    consumed[index] += 1;
                }
                BodyBiographyRecordKind::LullRetained { wake_id } => {
                    let index = self
                        .wakes
                        .iter()
                        .position(|wake| &wake.wake_id == wake_id)
                        .ok_or(BodyBiographyError::InvalidEvidence)?;
                    let wake = &self.wakes[index];
                    if consumed[index] != wake.events.len() {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                    body = body
                        .retain_after_lull(wake, record.sign_id.clone())
                        .map_err(invalid)?;
                }
                _ => {}
            }
        }
        if body != self.body
            || begun != self.wakes.len()
            || consumed
                .iter()
                .zip(&self.wakes)
                .any(|(count, wake)| *count != wake.events.len())
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        Ok(())
    }
}
