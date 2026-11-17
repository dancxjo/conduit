//! Two bounded, opaque kernel-committed contribution routes. Neither labels nor
//! transport delivery authorize a contribution, and terminal Plays are historical.
use super::Owner;
use conduit_face::{
    BodyFace, Face, FaceContext, FaceContribution, FaceContributionRole, FaceFocus, FaceFragment,
    FaceNames,
};
use conduit_std_host::body_execution::{
    BodyFaceContributionObserver, CommittedFaceContribution, FaceContributionRefusal,
    ReturnedFaceContribution,
};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

pub(super) struct RuntimeFaceSender(SyncSender<CommittedFaceContribution>);
pub(super) struct RuntimeFaceReceiver(Receiver<CommittedFaceContribution>);
/// One receiver slot and two retained route frames. Immutable frame backing
/// retains its original execution lease; channel/cache metadata belongs to the
/// installed owner's separately bounded service, not the producer's workspace.
pub(super) fn channel() -> (RuntimeFaceSender, RuntimeFaceReceiver) {
    let (sender, receiver) = mpsc::sync_channel(1);
    (RuntimeFaceSender(sender), RuntimeFaceReceiver(receiver))
}
impl BodyFaceContributionObserver for RuntimeFaceSender {
    fn contribute(
        &mut self,
        contribution: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        self.0.try_send(contribution).map_err(|error| match error {
            TrySendError::Full(contribution) => ReturnedFaceContribution {
                contribution,
                refusal: FaceContributionRefusal::Pressure,
            },
            TrySendError::Disconnected(contribution) => ReturnedFaceContribution {
                contribution,
                refusal: FaceContributionRefusal::Revoked,
            },
        })
    }
}
impl RuntimeFaceReceiver {
    pub(super) fn progress(&self, owner: &mut Owner) -> Result<(), String> {
        let retired = owner.runtime_face.each_ref().map(|slot| {
            slot.as_ref()
                .is_some_and(|frame| !owner.runtime_face_is_current(frame))
        });
        // Inspect one bounded slot before changing the retained view. A foreign
        // token or exhausted revision must not partly replace a valid route.
        let incoming = if let Ok(frame) = self.0.try_recv() {
            if !owner.runtime_face_scope_matches_current(&frame) {
                return Err(
                    "committed Face differs from current Body/Host/Boot/Plan/Play/workset".into(),
                );
            }
            if frame.scope().is_retired() {
                None
            } else {
                if owner.runtime_face.iter().enumerate().any(|(index, slot)| {
                    !retired[index]
                        && slot.as_ref().is_some_and(|previous| {
                            previous.fragment().basis != frame.fragment().basis
                        })
                }) {
                    return Err(
                        "committed Face routes require the same exact parent contribution basis"
                            .into(),
                    );
                }
                let index = owner
                    .runtime_face
                    .iter()
                    .enumerate()
                    .position(|(index, slot)| {
                        !retired[index]
                            && slot
                                .as_ref()
                                .is_some_and(|previous| same_route(previous, &frame))
                    })
                    .or_else(|| {
                        owner
                            .runtime_face
                            .iter()
                            .enumerate()
                            .position(|(index, slot)| slot.is_none() || retired[index])
                    })
                    .ok_or("committed Face route capacity exhausted")?;
                if !retired[index]
                    && owner.runtime_face[index].as_ref().is_some_and(|previous| {
                        same_route(previous, &frame) && previous.request() >= frame.request()
                    })
                {
                    return Err(
                        "committed Face request did not advance within current route".into(),
                    );
                }
                Some((index, frame))
            }
        } else {
            None
        };
        if retired.iter().any(|retired| *retired) || incoming.is_some() {
            owner
                .owner_navigation
                .advance_revision(owner.session.evidence().last_sequence())?;
            for (index, retired) in retired.into_iter().enumerate() {
                if retired {
                    owner.runtime_face[index] = None;
                }
            }
            if let Some((index, frame)) = incoming {
                owner.runtime_face[index] = Some(frame);
            }
        }
        Ok(())
    }
}
fn same_route(previous: &CommittedFaceContribution, frame: &CommittedFaceContribution) -> bool {
    let previous = previous.scope();
    let current = frame.scope();
    previous.body_play() == current.body_play()
        && previous.plot() == current.plot()
        && previous.host() == current.host()
        && previous.boot() == current.boot()
        && previous.partition_plan() == current.partition_plan()
        && previous.fragment() == current.fragment()
        && previous.node() == current.node()
        && previous.placement() == current.placement()
        && previous.call() == current.call()
}
impl Owner {
    fn runtime_face_is_current(&self, frame: &CommittedFaceContribution) -> bool {
        !frame.scope().is_retired() && self.runtime_face_scope_matches_current(frame)
    }
    fn runtime_face_scope_matches_current(&self, frame: &CommittedFaceContribution) -> bool {
        if self.workset_recovery_required || self.execution_persistence_pending {
            return false;
        }
        let scope = frame.scope();
        let host = self.host.advertisement();
        let Some(realization) = self.session.realization() else {
            return false;
        };
        let Some(play) = &realization.play else {
            return false;
        };
        if play != scope.body_play()
            || self.session.evidence().body_id != play.body_id
            || realization.wake.body_id != play.body_id
            || realization.wake.wake_id != play.wake_id
            || realization.plan.plan_id != play.plan_id
            || realization.plan.validate_for(&realization.wake).is_err()
            || !play.validate_for(&realization.plan)
            || scope.host() != &host.host_id
            || scope.boot() != &host.boot_id
            || frame.fragment().basis.checked_plot_id != scope.plot().checked_plot_id
            || frame.fragment().basis.plan_id != play.plan_id
            || frame.fragment().basis.active_play_id != play.active_play_id
            || frame
                .fragment()
                .basis
                .required_interaction_context
                .is_some()
        {
            return false;
        }
        realization.plan.plots.iter().any(|entry| {
            entry.plan.plan_id == *scope.partition_plan()
                && entry.plot.source_document_id == scope.plot().source_document_id
                && entry.plot.checked_plot_id == scope.plot().checked_plot_id
                && self
                    .session
                    .evidence()
                    .body
                    .workset
                    .plots()
                    .contains(&entry.plot)
                && entry.plan.fragments.iter().any(|fragment| {
                    fragment.fragment_id == *scope.fragment()
                        && fragment.expanded_plot_id == scope.plot().expanded_plot_id
                        && fragment.host_id == *scope.host()
                        && fragment.boot_id == *scope.boot()
                        && fragment.offer_generation == host.offer_generation
                        && fragment.placements.iter().any(|placement| {
                            placement.placement_id == *scope.placement()
                                && placement.host_id == *scope.host()
                                && placement.boot_id == *scope.boot()
                                && placement.host_calls.get(scope.call().0 as usize).is_some()
                        })
                })
        })
    }
    pub(super) fn project_runtime_face(&self) -> Result<Option<Face>, String> {
        let frames: Vec<_> = self
            .runtime_face
            .iter()
            .flatten()
            .filter(|frame| self.runtime_face_is_current(frame))
            .collect();
        if frames.is_empty() {
            return Ok(None);
        }
        let revision = self
            .owner_navigation
            .face_revision(self.session.evidence().last_sequence())?;
        // This clone is bounded projection workspace owned by the snapshot
        // service. It neither creates a committed token nor changes its basis.
        let projected = BodyFace::project_with_names(
            &self.session.evidence().body,
            self.session
                .realization()
                .map(|realization| &realization.wake),
            revision,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![FaceContribution::from_face(
                FaceContributionRole::Inspection,
                compose_fragments(
                    frames[0].fragment(),
                    frames.get(1).map(|frame| frame.fragment()),
                )?,
            )],
            FaceNames {
                body_name: Some(&self.session.evidence().friendly_name),
                resident_plots: &[],
            },
        )
        .map_err(|e| format!("current committed Face projection refused: {e:?}"))?;
        projected
            .face
            .validate()
            .map_err(|e| format!("current committed Face invalid: {e:?}"))?;
        Ok(Some(projected.face))
    }
}

/// Both private tokens keep their original leases. Only this bounded snapshot
/// clone is composed: one parent Plot basis means one Inspection contribution,
/// and therefore one canonical ActivePlay provenance subject in the Body Face.
fn compose_fragments(
    first: &FaceFragment,
    second: Option<&FaceFragment>,
) -> Result<FaceFragment, String> {
    first
        .validate_bounds()
        .map_err(|e| format!("runtime Face fragment bounds: {e:?}"))?;
    let Some(second) = second else {
        return Ok(first.clone());
    };
    if first.basis != second.basis {
        return Err("runtime Face fragments have different parent contribution bases".into());
    }
    second
        .validate_bounds()
        .map_err(|e| format!("runtime Face fragment bounds: {e:?}"))?;
    for (left, right, maximum) in [
        (
            first.subjects.len(),
            second.subjects.len(),
            conduit_face::MAX_FACE_SUBJECTS,
        ),
        (
            first.relationships.len(),
            second.relationships.len(),
            conduit_face::MAX_FACE_RELATIONSHIPS,
        ),
        (
            first.composition.len(),
            second.composition.len(),
            conduit_face::MAX_FACE_COMPOSITION_RELATIONS,
        ),
        (
            first.properties.len(),
            second.properties.len(),
            conduit_face::MAX_FACE_PROPERTIES,
        ),
        (
            first.text.len(),
            second.text.len(),
            conduit_face::MAX_FACE_TEXT_ITEMS,
        ),
        (
            first.actions.len(),
            second.actions.len(),
            conduit_face::MAX_FACE_ACTIONS,
        ),
        (
            first.disclosures.len(),
            second.disclosures.len(),
            conduit_face::MAX_FACE_DISCLOSURES,
        ),
        (
            first.temporal_references.len(),
            second.temporal_references.len(),
            conduit_face::MAX_TEMPORAL_REFERENCES,
        ),
        (
            first.temporal_facts.len(),
            second.temporal_facts.len(),
            conduit_face::MAX_FACE_TEMPORAL_FACTS,
        ),
    ] {
        if left.checked_add(right).is_none_or(|sum| sum > maximum) {
            return Err("combined runtime Face exceeds its existing fragment bound".into());
        }
    }
    let mut fragment = first.clone();
    fragment.subjects.extend(second.subjects.iter().cloned());
    fragment
        .relationships
        .extend(second.relationships.iter().cloned());
    fragment
        .composition
        .extend(second.composition.iter().cloned());
    fragment
        .properties
        .extend(second.properties.iter().cloned());
    fragment.text.extend(second.text.iter().cloned());
    fragment.actions.extend(second.actions.iter().cloned());
    fragment
        .disclosures
        .extend(second.disclosures.iter().cloned());
    fragment
        .temporal_references
        .extend(second.temporal_references.iter().cloned());
    fragment
        .temporal_facts
        .extend(second.temporal_facts.iter().cloned());
    fragment
        .validate_bounds()
        .map_err(|e| format!("combined runtime Face invalid: {e:?}"))?;
    Ok(fragment)
}

#[cfg(test)]
#[path = "runtime_face/tests.rs"]
mod tests;
