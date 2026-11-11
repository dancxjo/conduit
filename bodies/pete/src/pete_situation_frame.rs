//! Typed preparation from Pete's established observation families. The ordinary
//! Situation Gear performs runtime freshness selection using explicit time.
use crate::{
    pete_situation_experiences, PeteExperienceRefusal as R, PeteSituation, SituationFactState,
};
use conduit_human::*;
use conduit_plot::rust_binding::BoundedSequence;

pub fn pete_situation_frame(
    situation: &PeteSituation,
    policy: ExperienceTemporalPolicy,
    revision: u64,
    provenance: &str,
    retain: bool,
) -> Result<ExperienceSituationFrame, R> {
    let mut items = pete_situation_experiences(situation, policy)?.items;
    for (name, missing, kind, domain, origin) in [
        (
            "battery",
            situation.battery.state == SituationFactState::Missing,
            conduit_robotics::ROBOTICS_BATTERY_INFO_ID,
            ExperienceDomain::BodyState,
            ExperienceOrigin::Observation,
        ),
        (
            "utterance",
            situation.utterance.state == SituationFactState::Missing,
            crate::PETE_UTTERANCE_INFO_KIND,
            ExperienceDomain::HumanUtterance,
            ExperienceOrigin::HumanReported,
        ),
        (
            "body",
            situation.body.state == SituationFactState::Missing,
            crate::PETE_BODY_STATE_INFO_KIND,
            ExperienceDomain::BodyState,
            ExperienceOrigin::Observation,
        ),
    ] {
        if missing {
            items.push(ExperienceItem {
                id: format!("pete/{name}/missing"),
                domain,
                content_kind: kind.into(),
                encoded_content: vec![],
                origin,
                temporal_role: ExperienceTemporalRole::Current,
                availability: ExperienceAvailability::NotObserved,
                certainty: ExperienceCertainty::Uncertain,
                observed_at: None,
                recorded_at: None,
                sources: vec![],
            });
        }
    }
    let request =
        crate::pete_experience_context::project_pete_experience_items(&items, revision, provenance)
            .map_err(|_| R::ContentBound)?;
    let observations = request
        .evidence()
        .get()
        .as_slice()
        .iter()
        .cloned()
        .zip(&items)
        .map(|(evidence, item)| {
            let time = match &item.observed_at {
                Some(time) => ExperienceSituationTime::at(
                    time.clone().try_into().map_err(|_| R::InvalidFact)?,
                )
                .map_err(|_| R::InvalidFact)?,
                None => ExperienceSituationTime::Absent,
            };
            ExperienceSituationObservation::new(evidence, time).map_err(|_| R::InvalidFact)
        })
        .collect::<Result<Vec<_>, R>>()?;
    ExperienceSituationFrame::new(
        ExperienceSituationObservations::new(
            BoundedSequence::try_from_iter(observations).map_err(|_| R::ContentBound)?,
        )
        .map_err(|_| R::ContentBound)?,
        policy,
        situation
            .reference
            .clone()
            .try_into()
            .map_err(|_| R::InvalidFact)?,
        retain,
        request.selection().clone(),
    )
    .map_err(|_| R::InvalidFact)
}

#[cfg(test)]
#[path = "pete_situation_frame_tests.rs"]
mod tests;
