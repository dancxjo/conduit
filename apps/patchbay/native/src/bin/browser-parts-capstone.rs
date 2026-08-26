//! One bounded native Body proof for mixed browser Parts and stable replanning truth.

use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, AmbientAdmissionProof,
    AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory, CandidateObservation,
    DiscoveryProofId, MembershipProofId, PartId, SpawnAdmissionProof, SpawnInvitationSecret,
};
use conduit_core::{HostAdvertisement, LinkBindingId, OfferGeneration, SignId};
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress, BrowserAdmissionIngress, BrowserAdmissionListener,
    BrowserAdmissionSocket, BROWSER_ADMISSION_PROTOCOL,
};
use patchbay_model::{PartPresentationState, PartsView};
use serde_json::json;

#[path = "browser_parts_capstone/navigation.rs"]
mod navigation;
#[path = "browser_parts_capstone/physical_body.rs"]
mod physical_body;
#[path = "browser_parts_capstone/physical_pico.rs"]
mod physical_pico;
#[path = "browser_parts_capstone/planning.rs"]
mod planning;
#[path = "browser_parts_capstone/receipt.rs"]
mod receipt;

struct AdmittedBrowser {
    socket: BrowserAdmissionSocket,
    advertisement: HostAdvertisement,
    part_id: PartId,
}

fn main() -> Result<(), String> {
    let mut physical = physical_body::PhysicalBody::prepare()?;
    let browser_basis = physical
        .plan()
        .is_none()
        .then(planning::cross_browser_form_basis)
        .transpose()?;
    let body = physical.birth(browser_basis)?;
    let mut membership = BodyMembership::new(body.body_id.clone()).map_err(debug("membership"))?;
    let here = PartId::bind(&body.body_id, "part/here", 1).map_err(debug("Here identity"))?;
    let (here_host, here_boot) = physical.here_identity();
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
                host_id: here_host,
                boot_id: here_boot,
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
    let pico = physical
        .take_pending()
        .map(|pending| pending.admit(&body, &mut candidates, &mut membership, &mut manager))
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
    let cross_browser = if physical.plan().is_none() {
        Some(planning::cross_browser_plan(
            &first.advertisement,
            &second.advertisement,
        )?)
    } else {
        None
    };
    let plan = match &cross_browser {
        Some(cross) => cross.plan.clone(),
        None => physical
            .plan()
            .ok_or("browser and physical Plan sources are both absent")?
            .plan
            .clone(),
    };
    let stable_plan_id = plan.plan_id.clone();
    let third = admit_spawn(&listener, &mut membership, &mut manager, false, 2)?
        .ok_or("first spawn invitation was unexpectedly refused")?;
    admit_spawn(&listener, &mut membership, &mut manager, true, 3)?;
    if plan.plan_id != stable_plan_id {
        return Err("joining a third Part mutated the active Plan".into());
    }
    let replacement = if let Some(exact) = physical.plan() {
        conduit_system_continuity::exact_r1_control_plan(
            exact.pico_advertisement.boot_id.clone(),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )?
        .plan
    } else {
        planning::cross_browser_plan(&second.advertisement, &third.advertisement)?.plan
    };
    if replacement.plan_id == stable_plan_id {
        return Err("explicit replan did not produce a distinct Plan".into());
    }
    if physical.plan().is_none()
        && !replacement
            .fragments
            .iter()
            .any(|fragment| fragment.host_id == third.advertisement.host_id)
    {
        return Err("browser replacement Plan did not use the new Part".into());
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
    let expected_current_plan_parts = if pico.is_some() { 1 } else { 2 };
    if before_offline.parts.len() != expected_parts
        || before_offline
            .parts
            .iter()
            .filter(|row| row.in_plan)
            .count()
            != expected_current_plan_parts
        || !before_offline.new_realization_possibilities
    {
        return Err(
            "Parts projection did not preserve active-Plan/future-possibility truth".into(),
        );
    }
    let navigation_receipt = cross_browser
        .as_ref()
        .map(|cross| {
            navigation::cord_line_receipt(
                &body,
                &membership,
                &candidates,
                &here,
                cross,
                &first.advertisement,
                &second.advertisement,
            )
        })
        .transpose()?;
    println!(
        "ready_for_offline body={} plan={} replacement_plan={} parts={} current_boots_in_plan={} future_possibilities=true pico_parts={}",
        body.body_id.as_str(),
        plan.plan_id.as_str(),
        replacement.plan_id.as_str(),
        expected_parts,
        expected_current_plan_parts,
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
    let machine_receipt = receipt::machine_receipt(
        &body,
        &membership,
        &after_offline,
        &plan,
        &replacement,
        pico.is_some(),
        navigation_receipt,
    )?;
    receipt::retain_if_requested(&machine_receipt)?;
    println!("{machine_receipt}");
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
