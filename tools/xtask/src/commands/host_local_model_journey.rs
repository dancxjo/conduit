use std::{path::Path, process::Command};

use conduit_body::{BodyLifecycleEvent, WakeLifecycleEvent};

use super::host_local_model::PreparedOrifinaJourney;
use crate::commands::body_journey_track::{self, TrackIdentities, TrackSource};

#[path = "host_local_model_documentary.rs"]
mod documentary;

pub(super) fn write(
    journey: &PreparedOrifinaJourney,
    receipt: &conduit_std_host::local_model_proof::LocalModelLiveProofReceipt,
) -> Result<(), Box<dyn std::error::Error>> {
    let evidence = &journey.receipt.biography;
    let body_sign = |select: fn(&BodyLifecycleEvent) -> bool| {
        evidence
            .body
            .events
            .iter()
            .find(|event| select(event))
            .map(|event| event.sign_id().as_str().to_owned())
            .ok_or("Orifina biography lacks a required Body sign")
    };
    let born = body_sign(|event| matches!(event, BodyLifecycleEvent::Born { .. }))?;
    let workload = body_sign(|event| matches!(event, BodyLifecycleEvent::FormAdmitted { .. }))?;
    let lulled = body_sign(|event| matches!(event, BodyLifecycleEvent::LullRetained { .. }))?;
    let fulfilled = body_sign(|event| matches!(event, BodyLifecycleEvent::Fulfilled { .. }))?;
    let wake = evidence
        .wakes
        .first()
        .and_then(|wake| wake.sign_ids.first())
        .ok_or("Orifina biography lacks its Wake sign")?
        .as_str()
        .to_owned();
    let failed = evidence
        .wakes
        .iter()
        .find(|wake| {
            wake.events
                .iter()
                .any(|event| matches!(event, WakeLifecycleEvent::Failed { .. }))
        })
        .and_then(|wake| wake.sign_ids.last())
        .ok_or("Orifina biography lacks its fault sign")?
        .as_str()
        .to_owned();
    let repaired = evidence
        .wakes
        .last()
        .and_then(|wake| wake.sign_ids.last())
        .ok_or("Orifina biography lacks its repair sign")?
        .as_str()
        .to_owned();
    let manifestation = receipt
        .presenter_requests
        .iter()
        .find(|candidate| candidate.request_identity == journey.requests[0].request_identity)
        .ok_or("Orifina live proof lacks its Presenter manifestation")?;
    let presentation = journey.requests[0].semantic_data.presentation.clone();
    let retained = manifestation.manifestation.clone();
    let initial = conduit_std_host::spoken_mask_journey::execute_retained_manifestation_mask(
        "spoken-initial",
        "orifina-spoken-initial",
        presentation.clone(),
        retained.clone(),
    )?;
    let alternate = conduit_std_host::spoken_mask_journey::execute_retained_manifestation_mask(
        "spoken-generative",
        "orifina-spoken-alternate",
        presentation.clone(),
        retained.clone(),
    )?;
    let replacement_alternate =
        conduit_std_host::spoken_mask_journey::execute_retained_manifestation_mask(
            "spoken-generative-replanned",
            "orifina-spoken-replanned",
            presentation.clone(),
            retained.clone(),
        )?;
    let restored = conduit_std_host::spoken_mask_journey::execute_retained_manifestation_mask(
        "spoken-restored",
        "orifina-spoken-restored",
        presentation.clone(),
        retained,
    )?;
    let initial_routes = conduit_std_host::spoken_mask_journey::admit_spoken_mask_routes(
        &[initial.clone(), alternate.clone()],
        "orifina-spoken-initial-plan",
    )?;
    let replacement_routes = conduit_std_host::spoken_mask_journey::admit_spoken_mask_routes(
        &[replacement_alternate.clone(), restored.clone()],
        "orifina-spoken-replacement-plan",
    )?;
    let observation =
        |action: conduit_presentation::MaskJourneyAction,
         event: &str,
         execution: Option<&conduit_std_host::spoken_mask_journey::SpokenMaskExecution>,
         plan: &conduit_core::PlanId,
         route: Option<&str>| {
            conduit_std_host::spoken_mask_journey::SpokenMaskJourneyObservation {
                action_id: action.id().into(),
                concrete_event: event.into(),
                presentation_id: presentation.identity.as_str().into(),
                selected_mask_form_id: execution
                    .map(|item| item.mask.form_identity.checked_form_id.as_str().into()),
                plan_id: plan.as_str().into(),
                selected_route_id: route.map(str::to_owned),
                show_id: execution.map(|item| item.shown.show.show_id.as_str().into()),
                receipt_ids: vec![action.id().into()],
            }
        };
    use conduit_presentation::MaskJourneyAction as A;
    let initial_plan = &initial_routes.body_plan.plan_id;
    let replacement_plan = &replacement_routes.body_plan.plan_id;
    let initial_route = initial_routes.routes.routes()[0].route_id.as_str();
    let alternate_route = initial_routes.routes.routes()[1].route_id.as_str();
    let replacement_alternate_route = replacement_routes.routes.routes()[0].route_id.as_str();
    let restored_route = replacement_routes.routes.routes()[1].route_id.as_str();
    let prepared_actions = vec![
        observation(A::InspectInitialShow, "initial spoken Mask completed Presenter, synthesis, artifact, and acknowledged Show Host Calls", Some(&initial), initial_plan, Some(initial_route)),
        observation(A::WearAlternateMask, "alternate spoken Mask was worn while the selected sealed route remained current", Some(&initial), initial_plan, Some(initial_route)),
        observation(A::PreferAlternateMask, "preference selected the alternate sealed route under the same immutable Body Plan", Some(&alternate), initial_plan, Some(alternate_route)),
        observation(A::WithdrawSelectedRoute, "selected route was withdrawn without inventing a Show", None, initial_plan, None),
        observation(A::InspectUnavailableShow, "no current Show; retained WAV is stale artifact evidence only", None, initial_plan, None),
        observation(A::AddPresentationHost, "replacement presentation Host became available but the old Plan selected no route on it", None, initial_plan, None),
        observation(A::AdmitReplacementPlan, "replacement Body Plan sealed the replacement Host routes", None, replacement_plan, None),
        observation(A::InspectReplannedShow, "replacement spoken route completed Presenter, synthesis, artifact, and acknowledged Show Host Calls", Some(&replacement_alternate), replacement_plan, Some(replacement_alternate_route)),
        observation(A::DoffAlternateMask, "alternate Mask was doffed and its Show ceased being current", None, replacement_plan, None),
        observation(A::InspectRestoredShow, "initial-role spoken Mask was restored through the replacement Body Plan", Some(&restored), replacement_plan, Some(restored_route)),
    ];
    struct HostedSpokenJourney(
        std::collections::VecDeque<
            conduit_std_host::spoken_mask_journey::SpokenMaskJourneyObservation,
        >,
    );
    impl conduit_presentation::MaskJourneyEmbodiment for HostedSpokenJourney {
        type Outcome = conduit_std_host::spoken_mask_journey::SpokenMaskJourneyObservation;
        type Error = String;

        fn perform(&mut self, action: A) -> Result<Self::Outcome, Self::Error> {
            let outcome = self
                .0
                .pop_front()
                .ok_or("hosted spoken Mask outcome missing")?;
            if outcome.action_id != action.id() {
                return Err(format!(
                    "hosted spoken Mask outcome {} does not match {}",
                    outcome.action_id,
                    action.id()
                ));
            }
            Ok(outcome)
        }
    }
    let mut journey_run = HostedSpokenJourney(prepared_actions.into());
    let mut mask_actions = Vec::new();
    conduit_presentation::actualize_mask_journey(&mut journey_run, |_, outcome| {
        mask_actions.push(outcome.clone());
    })
    .map_err(|error| {
        format!(
            "hosted spoken Mask journey failed at {}: {}",
            error.action.id(),
            error.source
        )
    })?;
    let spoken_evidence = conduit_std_host::spoken_mask_journey::SpokenMaskJourneyEvidence {
        schema: conduit_std_host::spoken_mask_journey::SpokenMaskJourneyEvidence::SCHEMA.into(),
        observations: mask_actions.clone(),
        human_hearing_observed: false,
        inward_face_interaction_observed: false,
    };
    spoken_evidence.validate()?;
    let facts = [
        serde_json::json!({"initial_body": null, "host_id": journey.receipt.host_ids[0], "boot_id": journey.receipt.boot_ids[0]}),
        serde_json::json!({"provider": manifestation.manifestation.provider_identity, "model": manifestation.manifestation.model_identity}),
        serde_json::json!({"body_id": journey.receipt.body_id, "born_sign_id": born}),
        serde_json::json!({"wake_sign_id": wake, "plan_id": journey.receipt.plan_ids[0]}),
        serde_json::json!({"plan_id": journey.receipt.plan_ids[0], "play_id": journey.receipt.play_ids[0]}),
        serde_json::json!({"presentation_id": manifestation.source_presentation_identity, "manifestation": manifestation.manifestation}),
        serde_json::json!({"workload_revision": journey.receipt.workload_revision, "workload_sign_id": workload}),
        serde_json::json!({"host_id": journey.receipt.host_ids[1], "boot_id": journey.receipt.boot_ids[1], "membership": evidence.membership}),
        serde_json::json!({"reason": journey.receipt.fault_reason, "fault_sign_id": failed}),
        serde_json::json!({"repaired": journey.receipt.repaired, "plan_id": journey.receipt.plan_ids.last(), "play_id": journey.receipt.play_ids.last()}),
        serde_json::json!({"active_play_id": journey.receipt.play_ids.last(), "presenter_requests": receipt.presenter_requests.len()}),
        serde_json::json!({"lull_sign_id": lulled}),
        serde_json::json!({"fulfilled_sign_id": fulfilled, "fulfilled": journey.receipt.fulfilled}),
    ];
    let (embodiment, presenter_id) = if receipt.proof_class == "ollama-http-fixture" {
        (
            "hosted-ollama-http-fixture-body",
            "std/ollama-http-fixture@1",
        )
    } else {
        (
            "hosted-open-weight-model-body",
            "std/local-open-weight-model@1",
        )
    };
    body_journey_track::write(
        TrackSource {
            commit: git_head()?,
            track_id: "hosted-generative",
            embodiment,
            presenter_id,
            identities: TrackIdentities {
                body: journey.receipt.body_id.clone(),
                host: journey.receipt.host_ids[0].clone(),
                boot: journey.receipt.boot_ids[0].clone(),
                peer_host: journey.receipt.host_ids[1].clone(),
                peer_boot: journey.receipt.boot_ids[1].clone(),
                plan: journey
                    .receipt
                    .plan_ids
                    .last()
                    .cloned()
                    .ok_or("missing Orifina Plan")?,
                distributed_plan: None,
                play: journey
                    .receipt
                    .play_ids
                    .last()
                    .cloned()
                    .ok_or("missing Orifina Play")?,
                presentation: manifestation.source_presentation_identity.clone(),
                manifestation: manifestation.manifestation.manifestation_identity.clone(),
                line: None,
                signs: [
                    None,
                    None,
                    Some(born),
                    Some(wake),
                    Some(repaired.clone()),
                    None,
                    Some(workload),
                    Some("sign/orifina-companion/joined".into()),
                    Some(failed),
                    Some(repaired.clone()),
                    Some(repaired),
                    Some(lulled),
                    Some(fulfilled),
                ],
            },
            facts,
            mask_actions,
        },
        Path::new("target/journeys/three-bodies/hosted-generative"),
    )
    .map_err(std::io::Error::other)?;
    documentary::retain(
        Path::new("target/journeys/three-bodies/hosted-generative"),
        journey,
        receipt,
    )?;
    Ok(())
}

fn git_head() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    if !output.status.success() {
        return Err("cannot resolve exact source commit".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
