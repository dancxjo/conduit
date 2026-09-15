//! Retained shared application models prepared from exact Body Plan truth.

use conduit_body::BodyPlan;
use conduit_core::{ActivePlayId, ConfigurationValue, PlannedGear};
use conduit_tour_model::{TourApplicationPort, TourRunProof, TourWorkspaceRequest};

fn proof_from_manifestation(
    effect: &super::TourEffect,
    specimen_id: &str,
    expected: &str,
) -> Option<TourRunProof> {
    (effect.text.as_deref() == Some(expected)).then(|| TourRunProof {
        specimen_id: specimen_id.into(),
        source_document_id: conduit_core::SourceDocumentId(effect.source_document_id.clone()),
        checked_form_id: conduit_core::CheckedFormId(effect.checked_form_id.clone()),
        expanded_form_id: conduit_core::ExpandedFormId(effect.expanded_form_id.clone()),
        plan_id: conduit_core::PlanId(effect.plan_id.clone()),
        active_play_id: conduit_core::ActivePlayId(effect.active_play_id.clone()),
        result: expected.into(),
        terminal: conduit_tour_model::TourRunTerminal::Stopped,
    })
}

fn run_resident_tour(chapter: u8, stage_index: u8) -> Result<TourRunProof, String> {
    let stage = conduit_tour_model::TOUR_CHAPTERS
        .get(usize::from(chapter))
        .and_then(|chapter| chapter.stages.get(usize::from(stage_index)))
        .ok_or("resident Tour requested an unknown exact stage")?;
    if chapter != 0 || stage.mode != conduit_tour_model::TourStageMode::Run {
        return Err("resident browser Host does not yet implement this exact Tour stage".into());
    }
    let expected = stage
        .expected_text
        .ok_or("resident Tour stage has no exact expected text")?;
    let expected_manifestations = stage
        .expected_manifestations
        .ok_or("resident Tour stage has no exact manifestation count")?;
    let source = conduit_tour_model::tour_stage_source(chapter, stage_index)
        .map_err(|error| error.to_string())?;
    let (mut session, effect) =
        super::TourSession::prepare("browser/resident-tour", "boot/resident-tour", &source, 1)?;
    let mut manifestations = 0_u8;
    let mut proof = match effect {
        super::TourHostEffect::Manifestation(effect) => {
            manifestations += 1;
            proof_from_manifestation(&effect, stage.identity, expected)
        }
        _ => return Err("resident Tour stage requested an unsupported Host effect".into()),
    };
    loop {
        match session.advance()? {
            super::TourProgress::Effect(effect) => match *effect {
                super::TourHostEffect::Manifestation(effect) => {
                    manifestations = manifestations
                        .checked_add(1)
                        .ok_or("resident Tour manifestation count exhausted")?;
                    proof = proof
                        .or_else(|| proof_from_manifestation(&effect, stage.identity, expected));
                }
                _ => return Err("resident Tour stage requested an unsupported Host effect".into()),
            },
            super::TourProgress::Receipt(_) => {
                if manifestations != expected_manifestations {
                    return Err(
                        "resident Tour stage completed with the wrong manifestation count".into(),
                    );
                }
                let mut proof = proof.ok_or_else(|| {
                    "resident Tour stage completed without its expected manifestation".to_string()
                })?;
                proof.terminal = conduit_tour_model::TourRunTerminal::Completed;
                return Ok(proof);
            }
            super::TourProgress::Waiting { .. }
                if proof.is_some() && manifestations == expected_manifestations =>
            {
                let receipt = session.cancel()?;
                if receipt.disposition != "cancelled" {
                    return Err("resident Tour open exercise did not stop exactly".into());
                }
                return Ok(proof.expect("proof checked above"));
            }
            _ => return Err("resident Tour stage did not reach an exact terminal receipt".into()),
        }
    }
}

pub(super) enum PreparedApplication {
    Tour(TourApplicationPort),
    Patchbay(patchbay_application::PatchbayApplicationPort),
}

impl PreparedApplication {
    pub(super) fn prepare(
        placement: &PlannedGear,
        plan: &BodyPlan,
        _active_play_id: &ActivePlayId,
        source: &str,
        foreground_checked_form_id: &str,
    ) -> Result<Option<Self>, String> {
        #[cfg(not(feature = "creche-surface"))]
        let _ = (plan, source, foreground_checked_form_id);
        if placement.kind_id.as_str() != conduit_semantic_catalog::RETAINED_APPLICATION_KIND {
            return Ok(None);
        }
        let application = crate::installed_browser::application::application_id(placement)?;
        match application {
            "tour" => Ok(Some(Self::Tour(TourApplicationPort::canonical()))),
            "patchbay" => {
                #[cfg(not(feature = "creche-surface"))]
                return Err("resident Patchbay preparation requires the Crèche surface".into());

                #[cfg(feature = "creche-surface")]
                {
                    let target = plan.forms.iter().find(|part| {
                    part.form.checked_form_id.as_str() == foreground_checked_form_id
                        && part.plan.fragments.iter().all(|fragment| fragment.placements.iter().all(|gear| {
                            gear.configuration.iter().all(|entry| !matches!(
                                (&*entry.key, &entry.value),
                                (conduit_semantic_catalog::APPLICATION_ID_CONFIGURATION, ConfigurationValue::Text(value)) if value == "patchbay"
                            ))
                        }))
                }).or_else(|| plan.forms.iter().find(|part| part.form.checked_form_id.as_str() != foreground_checked_form_id))
                    .ok_or("Patchbay has no resident Form subject")?;
                    let expanded = crate::creche::expanded_inventory_form(source, &target.form)?;
                    let port = patchbay_application::PatchbayApplicationPort::open(
                        &expanded,
                        target.plan.plan_id.clone(),
                        plan.plan_id.clone(),
                    )
                    .map_err(|error| format!("prepare resident Patchbay: {error:?}"))?;
                    Ok(Some(Self::Patchbay(port)))
                }
            }
            _ => Err(format!("unknown retained application {application:?}")),
        }
    }

    pub(super) fn execute(&mut self, input: &[u8]) -> Result<Vec<u8>, String> {
        let input = if input == [0] { &[][..] } else { input };
        match self {
            Self::Tour(port) => {
                let output = port
                    .apply(input)
                    .map_err(|error| format!("resident Tour event: {error:?}"))?;
                if let Some(TourWorkspaceRequest::Run { chapter, stage }) = output.request {
                    port.complete_run(run_resident_tour(chapter, stage)?)
                        .map_err(|error| format!("complete resident Tour run: {error:?}"))?;
                    return port
                        .apply(&[])
                        .map(|output| output.view)
                        .map_err(|error| format!("project resident Tour result: {error:?}"));
                }
                Ok(output.view)
            }
            Self::Patchbay(port) => port
                .apply(input)
                .map(|output| output.view)
                .map_err(|error| format!("resident Patchbay event: {error:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_resident_tour_runs_every_chapter_one_exercise_exactly() {
        let expected = [
            ("canonical-form:meet-one-gear", "HELLO"),
            ("canonical-form:edit-one-gear", "MAKE THIS LOUD"),
            ("canonical-form:branch-a-cord", "SOS"),
        ];
        for (stage, (identity, result)) in expected.into_iter().enumerate() {
            let proof = run_resident_tour(0, stage as u8).unwrap();
            assert_eq!(proof.specimen_id, identity);
            assert_eq!(proof.result, result);
            assert!(!proof.source_document_id.as_str().is_empty());
            assert!(!proof.checked_form_id.as_str().is_empty());
            assert!(!proof.expanded_form_id.as_str().is_empty());
            assert!(!proof.plan_id.as_str().is_empty());
            assert!(!proof.active_play_id.as_str().is_empty());
        }
        assert_eq!(
            run_resident_tour(0, 0).unwrap().terminal,
            conduit_tour_model::TourRunTerminal::Completed
        );
        assert_eq!(
            run_resident_tour(0, 1).unwrap().terminal,
            conduit_tour_model::TourRunTerminal::Stopped
        );
    }

    #[test]
    fn browser_resident_tour_refuses_an_unimplemented_exact_stage() {
        assert_eq!(
            run_resident_tour(1, 0),
            Err("resident browser Host does not yet implement this exact Tour stage".into())
        );
    }
}
