//! Live-browser Patchbay front-door topology and realization capstone.

#![recursion_limit = "256"]

use conduit_body::{AdmissionSigns, AmbientAdmissionProof, CandidateObservation, DiscoveryProofId};
use conduit_core::{
    process_owned_line_offer_with_limits, BootId, ConnectionBase, HostId, LineAvailability, LineId,
    LinkBindingId, LinkLimits, OfferGeneration, SignId,
};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BROWSER_ADMISSION_PROTOCOL, MAX_BROWSER_ADMISSION_FRAME_BYTES,
};
use patchbay_model::{
    compare_entrances, EntranceUpdateDisposition, LocalFrontDoor, PatchbayEntranceState,
    RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
};
use serde_json::json;

const RECEIPT_PATH_ENV: &str = "CONDUIT_PATCHBAY_TOPOLOGY_RECEIPT_PATH";
const BROWSER_LINE_ID: &str = "patchbay-front-door/browser-line";
const BROWSER_BINDING_ID: &str = "patchbay-front-door/browser-binding";

fn main() -> Result<(), String> {
    let mut session = LocalFrontDoor::with_identity(
        HostId::from("patchbay-front-door/here"),
        BootId::from("patchbay-front-door/here/boot-1"),
    )?;
    let initial = session.project()?;
    let mut native_state = PatchbayEntranceState::enter(&initial.presentation)
        .map_err(|error| format!("native entrance: {error:?}"))?;
    let mut browser_state = native_state.clone();
    let original_selection = native_state.selected_subject.clone();
    let (first_plan, first_play) = session.plan_and_play()?;

    let listener = BrowserAdmissionListener::bind_loopback().map_err(debug("bind Body Line"))?;
    let body_url = listener.url().map_err(debug("Body Line URL"))?;
    println!("body_url={body_url}");
    let mut socket = listener.accept().map_err(debug("accept browser"))?;
    let (advertisement, verifying_key, friendly_label, freshness_sequence, encoded_bytes) =
        match socket
            .receive_with_size()
            .map_err(debug("browser advertisement"))?
        {
            (
                BrowserAdmissionIngress::Advertise {
                    advertisement,
                    verifying_key,
                    friendly_label,
                    freshness_sequence,
                    ..
                },
                encoded_bytes,
            ) => (
                advertisement,
                verifying_key,
                friendly_label,
                freshness_sequence,
                encoded_bytes,
            ),
            _ => return Err("browser did not begin with an advertisement".into()),
        };
    let candidate_id = session.observe_candidate(CandidateObservation {
        advertisement: advertisement.clone(),
        friendly_label,
        observed_binding_id: LinkBindingId::from(BROWSER_BINDING_ID),
        observation_sign_id: SignId::from("patchbay-front-door/browser-observed"),
        proof_id: DiscoveryProofId::bind("patchbay-front-door/browser-discovery")
            .map_err(debug("discovery proof"))?,
        freshness_sequence,
        encoded_bytes,
    })?;
    session.observe_line(process_owned_line_offer_with_limits(
        BROWSER_LINE_ID,
        BROWSER_BINDING_ID,
        ConnectionBase::WebSocket,
        &body_url,
        &advertisement,
        session.advertisement(),
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
            maximum_buffered_bytes: (MAX_BROWSER_ADMISSION_FRAME_BYTES * 2) as u32,
            maximum_frame_bytes: MAX_BROWSER_ADMISSION_FRAME_BYTES as u32,
        },
    ))?;
    let candidate = session.project()?;
    let candidate_subject = format!("candidate/{}", candidate_id.as_str());
    for state in [&mut native_state, &mut browser_state] {
        state
            .update(&candidate.presentation)
            .map_err(|error| format!("candidate update: {error:?}"))?;
        state
            .select(&candidate.presentation, &candidate_subject)
            .map_err(|error| format!("candidate selection: {error:?}"))?;
    }

    let challenge = session.begin_ambient_admission(
        &candidate_id,
        verifying_key
            .try_into()
            .map_err(|_| "browser verifying key has the wrong length")?,
        [29; 32],
        1_000,
        2_000,
        SignId::from("patchbay-front-door/browser-admission-requested"),
    )?;
    socket
        .send(&BrowserAdmissionEgress::Challenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge,
        })
        .map_err(debug("send challenge"))?;
    let proof = ambient_proof(socket.receive().map_err(debug("receive proof"))?)?;
    let credential = session.complete_ambient_admission(
        &proof,
        1_001,
        AdmissionSigns {
            part_admitted: SignId::from("patchbay-front-door/browser-part-admitted"),
            host_attached: SignId::from("patchbay-front-door/browser-host-attached"),
            candidate_admitted: SignId::from("patchbay-front-door/browser-candidate-admitted"),
        },
    )?;
    socket
        .send(&BrowserAdmissionEgress::Admitted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential: credential.clone(),
        })
        .map_err(debug("send admission"))?;
    let attached = session.project()?;
    for state in [&mut native_state, &mut browser_state] {
        if state
            .update(&attached.presentation)
            .map_err(|error| format!("attached update: {error:?}"))?
            != EntranceUpdateDisposition::SelectionBecameStale
        {
            return Err("admitted candidate selection did not become explicitly stale".into());
        }
        state
            .select(&attached.presentation, &format!("line/{BROWSER_LINE_ID}"))
            .map_err(|error| format!("attached Line selection: {error:?}"))?;
    }
    if session.current_plan_id() != Some(&first_plan)
        || session.current_play_id() != Some(&first_play)
    {
        return Err("browser admission mutated the active Plan or Play".into());
    }
    println!(
        "ready_for_offline body={} part={} plan={} play={}",
        session.body().body_id.as_str(),
        credential.part_id.as_str(),
        first_plan.as_str(),
        first_play.as_str()
    );

    if socket.receive().is_ok() {
        return Err("closed browser unexpectedly sent another admission frame".into());
    }
    session.observe_line_availability(
        &LineId::from(BROWSER_LINE_ID),
        LineAvailability::Unavailable,
        SignId::from("patchbay-front-door/browser-line-unavailable"),
    )?;
    let line_lost = session.project()?;
    for state in [&mut native_state, &mut browser_state] {
        if state
            .update(&line_lost.presentation)
            .map_err(|error| format!("Line update: {error:?}"))?
            != EntranceUpdateDisposition::SelectionPreserved
        {
            return Err("selected Line identity was not preserved after availability loss".into());
        }
        state
            .select(
                &line_lost.presentation,
                &format!("part/{}", credential.part_id.as_str()),
            )
            .map_err(|error| format!("attached Part selection: {error:?}"))?;
    }
    session.observe_part_offline(
        &credential.part_id,
        &advertisement.boot_id,
        SignId::from("patchbay-front-door/browser-offline"),
    )?;
    let offline = session.project()?;
    for state in [&mut native_state, &mut browser_state] {
        if state
            .update(&offline.presentation)
            .map_err(|error| format!("offline update: {error:?}"))?
            != EntranceUpdateDisposition::SelectionPreserved
        {
            return Err("durable offline Part selection was not preserved".into());
        }
    }
    session.observe_local_restart(
        BootId::from("patchbay-front-door/here/boot-2"),
        OfferGeneration(2),
    )?;
    let restarted = session.project()?;
    for state in [&mut native_state, &mut browser_state] {
        state
            .update(&restarted.presentation)
            .map_err(|error| format!("restart update: {error:?}"))?;
    }
    if session.current_plan_id() != Some(&first_plan) {
        return Err("restart mutated the immutable Plan".into());
    }
    let (replacement_plan, replacement_play) = session.plan_and_play()?;
    if replacement_plan == first_plan {
        return Err("explicit replan did not create a distinct Plan".into());
    }
    let final_projection = session.project()?;
    for state in [&mut native_state, &mut browser_state] {
        state
            .update(&final_projection.presentation)
            .map_err(|error| format!("replacement update: {error:?}"))?;
    }
    let equivalence = compare_entrances(
        &final_projection.presentation,
        &native_state,
        &browser_state,
    )
    .map_err(|error| format!("renderer equivalence: {error:?}"))?;
    let native = renderer(
        final_projection.presentation.clone(),
        RendererAdapterKind::NativeWayland,
        "native",
    )?;
    let browser = renderer(
        final_projection.presentation.clone(),
        RendererAdapterKind::HtmlDomSvg,
        "browser",
    )?;
    let receipt = json!({
        "schema": "conduit.patchbay/live-front-door-topology@1",
        "proof_class": "live-browser",
        "body_id": session.body().body_id.as_str(),
        "wake_id": session.wake().wake_id.as_str(),
        "original_selection": original_selection,
        "selected_part": native_state.selected_subject,
        "candidate_id": candidate_id.as_str(),
        "part_id": credential.part_id.as_str(),
        "browser_host_id": advertisement.host_id.as_str(),
        "browser_boot_id": advertisement.boot_id.as_str(),
        "first_plan_id": first_plan.as_str(),
        "first_play_id": first_play.as_str(),
        "replacement_plan_id": replacement_plan.as_str(),
        "replacement_play_id": replacement_play.as_str(),
        "plan_unchanged_by_join_offline_and_restart": true,
        "replacement_plan_distinct": true,
        "candidate_selection_became_stale": true,
        "offline_part_selection_preserved": true,
        "line_selection_preserved_on_loss": true,
        "line_id": BROWSER_LINE_ID,
        "line_binding_id": BROWSER_BINDING_ID,
        "line_base": "WebSocket",
        "line_availability_transition": ["Ready", "Unavailable"],
        "final_presentation_id": final_projection.presentation.identity.as_str(),
        "final_presentation_revision": final_projection.presentation.revision,
        "final_basis_sign_ids": final_projection.presentation.basis.sign_ids,
        "final_subjects": equivalence.subjects,
        "final_relationships": equivalence.relationships,
        "final_properties": equivalence.properties,
        "actions": equivalence.actions,
        "layer": equivalence.layer,
        "native_manifestation_id": native.manifestation.manifestation_id.as_str(),
        "browser_manifestation_id": browser.manifestation.manifestation_id.as_str(),
        "renderer_semantics_equivalent": equivalence.equivalent,
        "declared_bounds": {
            "body_parts": conduit_body::MAX_BODY_PARTS,
            "membership_events": conduit_body::MAX_MEMBERSHIP_EVENTS,
            "candidates": conduit_body::MAX_CANDIDATES,
            "lines": patchbay_model::MAX_FRONT_DOOR_LINES,
            "browser_admission_frame_bytes": MAX_BROWSER_ADMISSION_FRAME_BYTES,
            "presentation_subjects": conduit_presentation::MAX_PRESENTATION_SUBJECTS,
            "presentation_properties": conduit_presentation::MAX_PRESENTATION_PROPERTIES,
        }
    });
    retain(&receipt)?;
    println!("{receipt}");
    Ok(())
}

fn ambient_proof(frame: BrowserAdmissionIngress) -> Result<AmbientAdmissionProof, String> {
    match frame {
        BrowserAdmissionIngress::AmbientProof {
            admission_id,
            body_id,
            host_id,
            boot_id,
            nonce,
            signature,
            ..
        } => Ok(AmbientAdmissionProof {
            admission_id,
            body_id,
            host_id,
            boot_id,
            nonce: nonce.try_into().map_err(|_| "invalid ambient nonce")?,
            signature: signature
                .try_into()
                .map_err(|_| "invalid ambient signature")?,
        }),
        _ => Err("browser did not answer with ambient proof".into()),
    }
}

fn renderer(
    presentation: conduit_presentation::Presentation,
    kind: RendererAdapterKind,
    name: &str,
) -> Result<RendererExecution, String> {
    RendererExecution::prepare(
        presentation,
        kind,
        RendererAdapterIdentity {
            host_id: HostId::from(format!("patchbay-front-door/{name}-renderer")),
            boot_id: BootId::from(format!("patchbay-front-door/{name}-renderer/boot-1")),
            target_subject: format!("patchbay-front-door/{name}-target"),
        },
        SignId::from(format!("patchbay-front-door/{name}-prepared")),
    )
    .map_err(|error| error.to_string())
}

fn retain(receipt: &serde_json::Value) -> Result<(), String> {
    let Some(path) = std::env::var_os(RECEIPT_PATH_ENV) else {
        return Ok(());
    };
    let path = std::path::PathBuf::from(path);
    let parent = path
        .parent()
        .ok_or_else(|| format!("{RECEIPT_PATH_ENV} has no parent directory"))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("create receipt directory: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(receipt).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("write receipt: {error}"))?;
    std::fs::rename(&temporary, &path).map_err(|error| format!("retain receipt: {error}"))
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
