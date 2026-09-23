use std::{path::Path, process::Command};

use conduit_body::{BodyLifecycleEvent, WakeLifecycleEvent};

use super::host_local_model::PreparedOrifinaJourney;
use crate::commands::body_journey_track::{self, TrackIdentities, TrackSource};

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
    body_journey_track::write(
        TrackSource {
            commit: git_head()?,
            track_id: "hosted-generative",
            embodiment: "hosted-open-weight-model-body",
            presenter_id: "std/local-open-weight-model@1",
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
        },
        Path::new("target/journeys/three-bodies/hosted-generative"),
    )
    .map_err(std::io::Error::other)?;
    Ok(())
}

fn git_head() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    if !output.status.success() {
        return Err("cannot resolve exact source commit".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
