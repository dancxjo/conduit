//! One bounded native Body proof for mixed browser Parts and stable replanning truth.

use std::collections::BTreeMap;

use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, AmbientAdmissionProof,
    AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory, CandidateObservation,
    DiscoveryProofId, MembershipProofId, PartId, SpawnAdmissionProof, SpawnInvitationSecret,
};
use conduit_core::{
    BootId, CheckedFormId, ConnectionBase, HostAdvertisement, HostId, LinkBindingId, LinkLimits,
    OfferGeneration, SignId, SourceDocumentId,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{
    plan_expanded_canonical_with_options, PlacementChoice, PlacementChoices, PlanningOptions,
};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BrowserAdmissionSocket, BROWSER_ADMISSION_PROTOCOL,
};
use patchbay_model::{PartPresentationState, PartsView};
use serde_json::json;

#[path = "browser_parts_capstone/physical_pico.rs"]
mod physical_pico;
#[path = "browser_parts_capstone/receipt.rs"]
mod receipt;

const SOURCE: &str = include_str!("../../../../examples/webchat.conduit");

struct AdmittedBrowser {
    socket: BrowserAdmissionSocket,
    advertisement: HostAdvertisement,
    part_id: PartId,
}

fn main() -> Result<(), String> {
    let body = Body::born(
        SourceDocumentId::from("source/browser-parts-capstone"),
        CheckedFormId::from("checked/browser-parts-capstone"),
        1,
        SignId::from("browser-parts-capstone/body-born"),
    )
    .map_err(debug("Body birth"))?;
    let mut membership = BodyMembership::new(body.body_id.clone()).map_err(debug("membership"))?;
    let here = PartId::bind(&body.body_id, "part/here", 1).map_err(debug("Here identity"))?;
    membership
        .admit(
            &body.body_id,
            membership.revision,
            here.clone(),
            MembershipProofId::bind("proof/here").map_err(debug("Here proof"))?,
            SignId::from("browser-parts-capstone/here-admitted"),
        )
        .map_err(debug("admit Here"))?;
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &here,
            AuthenticatedHostObservation {
                host_id: HostId::from("body/std-here"),
                boot_id: BootId::from("body/std-here-boot"),
                offer_generation: OfferGeneration(1),
                proof_id: MembershipProofId::bind("proof/here-present")
                    .map_err(debug("Here presence proof"))?,
                sequence: 1,
            },
            SignId::from("browser-parts-capstone/here-present"),
        )
        .map_err(debug("observe Here"))?;
    let mut candidates =
        CandidateInventory::new(body.body_id.clone()).map_err(debug("candidate inventory"))?;
    let mut manager = AdmissionManager::new(body.body_id.clone()).map_err(debug("manager"))?;
    let pico = std::env::var("CONDUIT_B9_PICO_LINK_PORT")
        .ok()
        .map(|path| {
            physical_pico::admit(&path, &body, &mut candidates, &mut membership, &mut manager)
        })
        .transpose()?;
    let secret_bytes = [37; 32];
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes(secret_bytes).map_err(debug("secret"))?,
            [41; 32],
            1_000,
            20_000,
        )
        .map_err(debug("spawn invitation"))?;
    let listener = BrowserAdmissionListener::bind_loopback().map_err(debug("bind"))?;
    let body_url = listener.url().map_err(debug("URL"))?;
    let envelope = serde_json::to_vec(&json!({
        "claim": invitation.claim(),
        "secret": secret_bytes,
    }))
    .map_err(|error| format!("encode invitation: {error}"))?;
    let spawn_hex = envelope
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!("body_url={body_url}");
    println!("spawn_hex={spawn_hex}");

    let first = admit_ambient(
        &listener,
        &body,
        &mut candidates,
        &mut membership,
        &mut manager,
        0,
    )?;
    let second = admit_ambient(
        &listener,
        &body,
        &mut candidates,
        &mut membership,
        &mut manager,
        1,
    )?;
    let plan = cross_browser_plan(&first.advertisement, &second.advertisement)?;
    let stable_plan_id = plan.plan_id.clone();
    let third = admit_spawn(&listener, &mut membership, &mut manager, false, 2)?
        .ok_or("first spawn invitation was unexpectedly refused")?;
    admit_spawn(&listener, &mut membership, &mut manager, true, 3)?;
    if plan.plan_id != stable_plan_id {
        return Err("joining a third Part mutated the active Plan".into());
    }
    let replacement = cross_browser_plan(&second.advertisement, &third.advertisement)?;
    if replacement.plan_id == stable_plan_id
        || !replacement
            .fragments
            .iter()
            .any(|fragment| fragment.host_id == third.advertisement.host_id)
    {
        return Err("explicit replan did not produce a distinct Plan using the new Part".into());
    }

    let expected_parts = 4 + usize::from(pico.is_some());
    let before_offline = PartsView::project(
        &body,
        &membership,
        &candidates,
        &here,
        Some(&plan),
        None,
        true,
    )
    .map_err(debug("Parts projection"))?;
    if before_offline.parts.len() != expected_parts
        || before_offline
            .parts
            .iter()
            .filter(|row| row.in_plan)
            .count()
            != 2
        || !before_offline.new_realization_possibilities
    {
        return Err(
            "Parts projection did not preserve active-Plan/future-possibility truth".into(),
        );
    }
    println!(
        "ready_for_offline body={} plan={} replacement_plan={} parts={} in_plan=2 future_possibilities=true pico_parts={}",
        body.body_id.as_str(),
        plan.plan_id.as_str(),
        replacement.plan_id.as_str(),
        expected_parts,
        usize::from(pico.is_some())
    );

    let mut first = first;
    if first.socket.receive().is_ok() {
        return Err("closed browser unexpectedly sent another admission frame".into());
    }
    membership
        .observe_offline(
            &body.body_id,
            membership.revision,
            &first.part_id,
            &first.advertisement.boot_id,
            SignId::from("browser-parts-capstone/browser-offline"),
        )
        .map_err(debug("observe browser offline"))?;
    let after_offline = PartsView::project(
        &body,
        &membership,
        &candidates,
        &here,
        Some(&plan),
        None,
        true,
    )
    .map_err(debug("offline Parts projection"))?;
    let offline = after_offline
        .parts
        .iter()
        .filter(|row| row.state == PartPresentationState::Offline)
        .count();
    println!(
        "capstone_complete body={} plan={} replacement_plan={} members={} browser_parts=3 pico_parts={} offline={} replay_refused=true plan_unchanged=true replan_distinct=true cross_host_fragments={}",
        body.body_id.as_str(),
        plan.plan_id.as_str(),
        replacement.plan_id.as_str(),
        membership.parts.len(),
        usize::from(pico.is_some()),
        offline,
        plan.fragments.len()
    );
    println!(
        "{}",
        receipt::machine_receipt(
            &body,
            &membership,
            &after_offline,
            &plan,
            &replacement,
            pico.is_some(),
        )?
    );
    drop((second, third, pico));
    Ok(())
}

fn admit_ambient(
    listener: &BrowserAdmissionListener,
    body: &Body,
    candidates: &mut CandidateInventory,
    membership: &mut BodyMembership,
    manager: &mut AdmissionManager,
    sequence: u64,
) -> Result<AdmittedBrowser, String> {
    let mut socket = listener.accept().map_err(debug("accept ambient browser"))?;
    let (frame, encoded_bytes) = socket.receive_with_size().map_err(debug("advertisement"))?;
    let BrowserAdmissionIngress::Advertise {
        advertisement,
        friendly_label,
        verifying_key,
        freshness_sequence,
        ..
    } = frame
    else {
        return Err("ambient browser did not begin with an advertisement".into());
    };
    let candidate_id = candidates
        .observe(CandidateObservation {
            advertisement: advertisement.clone(),
            friendly_label,
            observed_binding_id: LinkBindingId::from(format!("browser/capstone/{sequence}")),
            observation_sign_id: SignId::from(format!("browser/capstone/{sequence}/observed")),
            proof_id: DiscoveryProofId::bind(&format!("browser/capstone/{sequence}/proof"))
                .map_err(debug("discovery proof"))?,
            freshness_sequence,
            encoded_bytes,
        })
        .map_err(debug("observe candidate"))?;
    println!(
        "wants_to_join={} members_before_admit={}",
        candidate_id.as_str(),
        membership.parts.len()
    );
    let challenge = manager
        .begin_ambient(
            candidates,
            &candidate_id,
            verifying_key
                .try_into()
                .map_err(|_| "invalid verifying key")?,
            [u8::try_from(sequence + 1).map_err(|_| "sequence overflow")?; 32],
            1_100 + sequence,
            10_000,
            SignId::from(format!("browser/capstone/{sequence}/requested")),
        )
        .map_err(debug("begin ambient admission"))?;
    socket
        .send(&BrowserAdmissionEgress::Challenge {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            challenge,
        })
        .map_err(debug("send challenge"))?;
    let proof = ambient_proof(socket.receive().map_err(debug("receive proof"))?)?;
    let credential = manager
        .complete_ambient(
            candidates,
            membership,
            &proof,
            1_200 + sequence,
            signs(sequence),
        )
        .map_err(debug("complete ambient admission"))?;
    socket
        .send(&BrowserAdmissionEgress::Admitted {
            protocol: BROWSER_ADMISSION_PROTOCOL,
            credential: credential.clone(),
        })
        .map_err(debug("send admission"))?;
    if credential.body_id != body.body_id {
        return Err("ambient credential escaped the active Body".into());
    }
    Ok(AdmittedBrowser {
        socket,
        advertisement,
        part_id: credential.part_id,
    })
}

fn admit_spawn(
    listener: &BrowserAdmissionListener,
    membership: &mut BodyMembership,
    manager: &mut AdmissionManager,
    expect_replay: bool,
    sequence: u64,
) -> Result<Option<AdmittedBrowser>, String> {
    let mut socket = listener.accept().map_err(debug("accept spawned browser"))?;
    let advertisement = match socket.receive().map_err(debug("spawn advertisement"))? {
        BrowserAdmissionIngress::Advertise { advertisement, .. } => advertisement,
        _ => return Err("spawned browser did not begin with an advertisement".into()),
    };
    let proof = spawn_proof(socket.receive().map_err(debug("spawn proof"))?)?;
    let result = manager.complete_spawn(
        membership,
        &advertisement,
        &proof,
        1_300 + sequence,
        signs(sequence),
    );
    match (expect_replay, result) {
        (false, Ok(credential)) => {
            socket
                .send(&BrowserAdmissionEgress::Admitted {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    credential: credential.clone(),
                })
                .map_err(debug("send spawned admission"))?;
            Ok(Some(AdmittedBrowser {
                socket,
                advertisement,
                part_id: credential.part_id,
            }))
        }
        (true, Err(AdmissionRefusal::Replay)) => {
            socket
                .send(&BrowserAdmissionEgress::Refused {
                    protocol: BROWSER_ADMISSION_PROTOCOL,
                    code: "replay".into(),
                })
                .map_err(debug("send replay refusal"))?;
            Ok(None)
        }
        (false, Err(error)) => Err(format!("unexpected spawn refusal: {error:?}")),
        (true, Ok(_)) => Err("replayed invitation admitted another Part".into()),
        (true, Err(error)) => Err(format!("unexpected replay refusal: {error:?}")),
    }
}

fn cross_browser_plan(
    source: &HostAdvertisement,
    sink: &HostAdvertisement,
) -> Result<conduit_core::Plan, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile)?;
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(|error| format!("canonical webchat check: {error:?}"))?;
    let expanded = expand_canonical_form(&checked, "webchat-browser-demo", &profile)
        .map_err(|error| format!("canonical webchat expansion: {error:?}"))?;
    let mut by_gear = BTreeMap::new();
    for gear in &expanded.gears {
        let target = if gear.kind_id.as_str() == conduit_chat::WEB_TEXT_INPUT_KIND {
            source
        } else {
            sink
        };
        let capability = target
            .capabilities
            .iter()
            .find(|offer| offer.kind_id == gear.kind_id)
            .ok_or_else(|| format!("browser offer missing kind {}", gear.kind_id.as_str()))?;
        by_gear.insert(
            gear.gear_id.clone(),
            PlacementChoice {
                host_id: target.host_id.clone(),
                capability_id: capability.capability_id.clone(),
            },
        );
    }
    let cross = expanded
        .connections
        .iter()
        .find(|connection| {
            expanded
                .gears
                .iter()
                .find(|gear| gear.gear_id == connection.source_gear_id)
                .is_some_and(|gear| gear.kind_id.as_str() == conduit_chat::WEB_TEXT_INPUT_KIND)
        })
        .ok_or("expanded webchat has no text-to-socket connection")?;
    let line = conduit_core::process_owned_line_offer_with_limits(
        "browser/capstone/websocket-line",
        "browser/capstone/websocket-binding",
        ConnectionBase::WebSocket,
        "browser/capstone/websocket-instance",
        source,
        sink,
        LinkLimits {
            maximum_in_flight_items: 4,
            maximum_payload_bytes: 1_024,
            maximum_buffered_bytes: 4_096,
            maximum_frame_bytes: 8_192,
        },
    );
    let line_candidates = BTreeMap::from([(
        (cross.source_gear_id.clone(), cross.sink_gear_id.clone()),
        vec![line.line_id.clone()],
    )]);
    plan_expanded_canonical_with_options(
        &expanded,
        &[source.clone(), sink.clone()],
        &PlacementChoices { by_gear },
        &[ConnectionBase::Local, ConnectionBase::WebSocket],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 4,
            connection_byte_capacity: 1_024,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .map_err(|error| error.to_string())
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

fn spawn_proof(frame: BrowserAdmissionIngress) -> Result<SpawnAdmissionProof, String> {
    match frame {
        BrowserAdmissionIngress::SpawnProof {
            invitation_id,
            body_id,
            host_id,
            boot_id,
            nonce,
            signature,
            ..
        } => Ok(SpawnAdmissionProof {
            invitation_id,
            body_id,
            host_id,
            boot_id,
            nonce: nonce.try_into().map_err(|_| "invalid spawn nonce")?,
            signature: signature
                .try_into()
                .map_err(|_| "invalid spawn signature")?,
        }),
        _ => Err("browser did not answer with spawn proof".into()),
    }
}

fn signs(sequence: u64) -> AdmissionSigns {
    AdmissionSigns {
        part_admitted: SignId::from(format!("browser/capstone/{sequence}/part")),
        host_attached: SignId::from(format!("browser/capstone/{sequence}/host")),
        candidate_admitted: SignId::from(format!("browser/capstone/{sequence}/candidate")),
    }
}

fn debug<T: core::fmt::Debug>(context: &'static str) -> impl FnOnce(T) -> String {
    move |error| format!("{context}: {error:?}")
}
