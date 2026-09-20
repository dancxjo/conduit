//! Retained shared application models prepared from exact Body Plan truth.

use conduit_body::BodyPlan;
use conduit_core::{ActivePlayId, PlannedGear};
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
        comparison: None,
        multi_host: None,
    })
}

fn run_resident_compare(
    stage: &conduit_tour_model::TourStage,
    source: &str,
) -> Result<TourRunProof, String> {
    let expected = stage
        .expected_text
        .ok_or("resident Tour comparison has no exact result")?;
    let (mut proof, direct_segments, direct_unit_millis) =
        run_direct_comparison(stage, source, expected)?;
    complete_recursive_comparison(&mut proof, source, &direct_segments, direct_unit_millis)?;
    Ok(proof)
}

#[inline(never)]
fn run_direct_comparison(
    stage: &conduit_tour_model::TourStage,
    source: &str,
    expected: &str,
) -> Result<(TourRunProof, Vec<super::protocol::IndicatorSegment>, u16), String> {
    let (mut direct_session, direct) =
        super::TourSession::prepare("browser/resident-tour", "boot/resident-tour", source, 1)?;
    let super::TourHostEffect::Manifestation(direct) = direct else {
        return Err("resident Tour comparison requested an unsupported Host effect".into());
    };
    if direct.realization != "direct" {
        return Err("resident Tour direct realization was not selected".into());
    }
    if !matches!(
        direct_session.advance()?,
        super::TourProgress::Waiting { .. }
    ) || direct_session.cancel()?.disposition != "cancelled"
    {
        return Err("resident Tour direct realization did not stop exactly".into());
    }
    let direct_segments = direct.segments.clone();
    let direct_unit_millis = direct.unit_millis;
    let proof = TourRunProof {
        specimen_id: stage.identity.into(),
        source_document_id: conduit_core::SourceDocumentId(direct.source_document_id.clone()),
        checked_form_id: conduit_core::CheckedFormId(direct.checked_form_id.clone()),
        expanded_form_id: conduit_core::ExpandedFormId(direct.expanded_form_id.clone()),
        plan_id: conduit_core::PlanId(direct.plan_id.clone()),
        active_play_id: conduit_core::ActivePlayId(direct.active_play_id.clone()),
        result: expected.into(),
        terminal: conduit_tour_model::TourRunTerminal::Completed,
        comparison: None,
        multi_host: None,
    };
    Ok((proof, direct_segments, direct_unit_millis))
}

#[inline(never)]
fn complete_recursive_comparison(
    proof: &mut TourRunProof,
    source: &str,
    direct_segments: &[super::protocol::IndicatorSegment],
    direct_unit_millis: u16,
) -> Result<(), String> {
    let (mut recursive_session, recursive) = super::TourSession::prepare_recursive(
        "browser/resident-tour",
        "boot/resident-tour",
        source,
        2,
    )?;
    let super::TourHostEffect::Manifestation(recursive) = recursive else {
        return Err("resident Tour comparison requested an unsupported Host effect".into());
    };
    if proof.source_document_id.as_str() != recursive.source_document_id
        || proof.checked_form_id.as_str() != recursive.checked_form_id
        || proof.expanded_form_id.as_str() == recursive.expanded_form_id
        || proof.plan_id.as_str() == recursive.plan_id
        || direct_segments != recursive.segments
        || direct_unit_millis != recursive.unit_millis
        || recursive.realization != "recursive"
        || recursive.realization_backs.is_empty()
    {
        return Err("resident Tour realizations did not preserve the same Form behavior".into());
    }
    if !matches!(
        recursive_session.advance()?,
        super::TourProgress::Waiting { .. }
    ) || recursive_session.cancel()?.disposition != "cancelled"
    {
        return Err("resident Tour recursive realization did not stop exactly".into());
    }
    proof.comparison = Some(conduit_tour_model::TourComparisonProof {
        expanded_form_id: conduit_core::ExpandedFormId(recursive.expanded_form_id.clone()),
        plan_id: conduit_core::PlanId(recursive.plan_id.clone()),
    });
    Ok(())
}

fn run_resident_tour(chapter: u8, stage_index: u8) -> Result<TourRunProof, String> {
    let stage = conduit_tour_model::TOUR_CHAPTERS
        .get(usize::from(chapter))
        .and_then(|chapter| chapter.stages.get(usize::from(stage_index)))
        .ok_or("resident Tour requested an unknown exact stage")?;
    let source = conduit_tour_model::tour_stage_source(chapter, stage_index)
        .map_err(|error| error.to_string())?;
    match stage.mode {
        conduit_tour_model::TourStageMode::Compare => run_resident_compare(stage, &source),
        conduit_tour_model::TourStageMode::Run if chapter == 0 => {
            run_resident_stage(stage, &source)
        }
        conduit_tour_model::TourStageMode::Run if chapter == 2 => {
            run_resident_timer(stage, &source)
        }
        conduit_tour_model::TourStageMode::TwoHost
        | conduit_tour_model::TourStageMode::TwoHostPlan => run_resident_multi(stage, &source),
        _ => Err("resident browser Host does not yet implement this exact Tour stage".into()),
    }
}

#[inline(never)]
fn run_resident_multi(
    stage: &conduit_tour_model::TourStage,
    source: &str,
) -> Result<TourRunProof, String> {
    let expected = stage
        .expected_text
        .ok_or("resident Tour two-Host stage has no exact expected text")?;
    let multi = super::multihost::run_resident(source)?;
    let mut proof = proof_from_manifestation(&multi.manifestation, stage.identity, expected)
        .ok_or("resident Tour sink Host produced the wrong exact value")?;
    proof.terminal = conduit_tour_model::TourRunTerminal::Completed;
    proof.multi_host = Some(conduit_tour_model::TourMultiHostProof {
        source_fragment_id: multi.source_fragment_id,
        sink_fragment_id: multi.sink_fragment_id,
        source_active_play_id: multi.source_active_play_id,
        sink_active_play_id: multi.sink_active_play_id,
        line_id: multi.line_id,
        transferred_values: multi.transferred_values,
    });
    Ok(proof)
}

#[inline(never)]
fn run_resident_timer(
    stage: &conduit_tour_model::TourStage,
    source: &str,
) -> Result<TourRunProof, String> {
    let expected = stage
        .expected_text
        .ok_or("resident Tour timer stage has no exact expected text")?;
    let expected_timers = stage
        .expected_timer_completions
        .ok_or("resident Tour timer stage has no exact tick count")?;
    let (mut session, initial) =
        super::TourSession::prepare("browser/resident-tour", "boot/resident-tour", source, 1)?;
    let super::TourHostEffect::Timer(timer) = initial else {
        return Err("resident Tour timer stage did not request its admitted timer".into());
    };
    if timer.duration_millis != 120 {
        return Err("resident Tour timer stage requested the wrong duration".into());
    }
    let super::TourProgress::Effect(initial_count) = session.advance()? else {
        return Err("resident Tour timer stage did not expose its initial count".into());
    };
    let super::TourHostEffect::Manifestation(initial_count) = *initial_count else {
        return Err("resident Tour timer tick requested an unsupported Host effect".into());
    };
    if initial_count.text.as_deref() != Some("0") {
        return Err("resident Tour timer stage exposed the wrong initial count".into());
    }
    let super::TourProgress::Effect(second_timer) = session.advance()? else {
        return Err("resident Tour timer stage did not retain its continuing timer".into());
    };
    let super::TourHostEffect::Timer(second_timer) = *second_timer else {
        return Err("resident Tour timer stage requested an unsupported continuing effect".into());
    };
    if second_timer.duration_millis != 120 {
        return Err("resident Tour timer stage changed its admitted duration".into());
    }
    let super::TourProgress::Effect(ticked_count) = session.advance()? else {
        return Err("resident Tour timer tick did not produce a manifestation".into());
    };
    let super::TourHostEffect::Manifestation(manifestation) = *ticked_count else {
        return Err("resident Tour timer tick requested an unsupported manifestation".into());
    };
    let proof = proof_from_manifestation(&manifestation, stage.identity, expected)
        .ok_or("resident Tour timer tick produced the wrong result")?;
    let super::TourProgress::Effect(next) = session.advance()? else {
        return Err("resident Tour timer stage did not retain its next admitted tick".into());
    };
    let super::TourHostEffect::Timer(next) = *next else {
        return Err("resident Tour timer stage requested an unsupported continuing effect".into());
    };
    if next.duration_millis != 120 {
        return Err("resident Tour timer stage changed its admitted duration".into());
    }
    let receipt = session.cancel()?;
    if receipt.disposition != "cancelled"
        || receipt.timer_completions != u32::from(expected_timers)
        || receipt.manifestation_completions
            != u32::from(stage.expected_manifestations.unwrap_or_default())
    {
        return Err("resident Tour timer stage did not stop at its exact finite bound".into());
    }
    Ok(proof)
}

#[inline(never)]
fn run_resident_stage(
    stage: &conduit_tour_model::TourStage,
    source: &str,
) -> Result<TourRunProof, String> {
    let expected = stage
        .expected_text
        .ok_or("resident Tour stage has no exact expected text")?;
    let expected_manifestations = stage
        .expected_manifestations
        .ok_or("resident Tour stage has no exact manifestation count")?;
    let (mut session, effect) =
        super::TourSession::prepare("browser/resident-tour", "boot/resident-tour", source, 1)?;
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
    Tour(Box<TourApplicationPort>),
    #[cfg(feature = "creche-surface")]
    Patchbay(patchbay_application::PatchbayApplicationPort),
}

impl PreparedApplication {
    pub(super) fn prepare(
        placement: &PlannedGear,
        plan: &BodyPlan,
        active_play_id: &ActivePlayId,
        source: &str,
        foreground_checked_form_id: &str,
    ) -> Result<Option<Self>, String> {
        #[cfg(not(feature = "creche-surface"))]
        let _ = (plan, active_play_id, source, foreground_checked_form_id);
        if placement.kind_id.as_str() != conduit_semantic_catalog::RETAINED_APPLICATION_KIND {
            return Ok(None);
        }
        let application = crate::installed_browser::application::application_id(placement)?;
        match application {
            "tutorial" | "tour" => Ok(Some(Self::Tour(Box::new(TourApplicationPort::canonical())))),
            "patchbay" => {
                #[cfg(not(feature = "creche-surface"))]
                return Err("resident Patchbay preparation requires the Crèche surface".into());

                #[cfg(feature = "creche-surface")]
                {
                    let mut active = Vec::with_capacity(plan.forms.len());
                    for part in &plan.forms {
                        let expanded = crate::creche::expanded_inventory_form(source, &part.form)?;
                        let title = crate::creche::inventory_form_title(source, &part.form)?;
                        active.push(
                            patchbay_application::PatchbayActiveForm::project(
                                &expanded,
                                title,
                                part.form.checked_form_id.as_str(),
                                part.plan.plan_id.clone(),
                                patchbay_application::PatchbayActiveFormState::Playing,
                                part.form.checked_form_id.as_str() == foreground_checked_form_id,
                            )
                            .map_err(|error| {
                                format!("project resident Patchbay Form: {error:?}")
                            })?,
                        );
                    }
                    let port = patchbay_application::PatchbayApplicationPort::open_active(
                        active,
                        plan.plan_id.clone(),
                        Some(active_play_id.clone()),
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
            #[cfg(feature = "creche-surface")]
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
    fn browser_resident_tour_runs_later_exact_exercises() {
        let proof = run_resident_tour(1, 0).unwrap();
        assert_eq!(proof.specimen_id, "canonical-form:same-morse-caller");
        assert_eq!(proof.result, "Direct and recursive realizations agree");
        let comparison = proof.comparison.unwrap();
        assert_ne!(proof.expanded_form_id, comparison.expanded_form_id);
        assert_ne!(proof.plan_id, comparison.plan_id);

        let timed = run_resident_tour(2, 0).unwrap();
        assert_eq!(timed.specimen_id, "canonical-form:count-over-time");
        assert_eq!(timed.result, "1");
        assert_eq!(timed.terminal, conduit_tour_model::TourRunTerminal::Stopped);

        for stage in 0..=1 {
            let across = run_resident_tour(3, stage).unwrap();
            assert_eq!(across.specimen_id, "canonical-form:hello-across");
            assert_eq!(across.result, "hello across one cord");
            let multi = across.multi_host.unwrap();
            assert_ne!(multi.source_fragment_id, multi.sink_fragment_id);
            assert_ne!(multi.source_active_play_id, multi.sink_active_play_id);
            assert!(!multi.line_id.is_empty());
            assert_eq!(multi.transferred_values, 1);
        }

        assert_eq!(
            run_resident_tour(4, 0),
            Err("resident Tour requested an unknown exact stage".into())
        );
    }
}
