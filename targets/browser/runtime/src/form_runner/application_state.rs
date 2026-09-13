//! Retained shared application models prepared from exact Body Plan truth.

use conduit_body::BodyPlan;
use conduit_core::{ActivePlayId, ConfigurationValue, PlannedGear};
use conduit_tour_model::{TourApplicationPort, TourRunProof, TourWorkspaceRequest};

fn run_canonical_tour() -> Result<TourRunProof, String> {
    let (mut session, effect) = super::TourSession::prepare(
        "browser/resident-tour",
        "boot/resident-tour",
        conduit_tour_model::CANONICAL_SOURCE,
        1,
    )?;
    let super::TourHostEffect::Manifestation(effect) = effect else {
        return Err("canonical Tour run did not produce its planned manifestation".into());
    };
    if effect.text.as_deref() != Some(conduit_tour_model::CANONICAL_RESULT) {
        return Err("canonical Tour run produced the wrong manifestation".into());
    }
    let proof = TourRunProof {
        specimen_id: conduit_tour_model::CANONICAL_SPECIMEN_ID.into(),
        source_document_id: conduit_core::SourceDocumentId(effect.source_document_id.clone()),
        checked_form_id: conduit_core::CheckedFormId(effect.checked_form_id.clone()),
        expanded_form_id: conduit_core::ExpandedFormId(effect.expanded_form_id.clone()),
        plan_id: conduit_core::PlanId(effect.plan_id.clone()),
        active_play_id: conduit_core::ActivePlayId(effect.active_play_id.clone()),
        result: conduit_tour_model::CANONICAL_RESULT.into(),
    };
    if !matches!(session.advance()?, super::TourProgress::Receipt(_)) {
        return Err("canonical Tour run did not reach its terminal receipt".into());
    }
    Ok(proof)
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
        if placement.kind_id.as_str() != conduit_semantic_catalog::RETAINED_APPLICATION_KIND {
            return Ok(None);
        }
        let application = crate::installed_browser::application::application_id(placement)?;
        match application {
            "tour" => Ok(Some(Self::Tour(TourApplicationPort::canonical()))),
            "patchbay" => {
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
                if output.request == Some(TourWorkspaceRequest::Run) {
                    port.complete_run(run_canonical_tour()?)
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
