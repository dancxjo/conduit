//! Ordinary Mask Plan and authenticated cold staging in the same Body. This
//! prepares a dormant kernel; no Available Show or physical deployment claim.
use crate::{
    admission::Authority, checked, native_owner_control as owner, native_owner_receipt as ack,
    planning::Prepared, transport::Line, Result,
};
use conduit_body as b;
use conduit_core as c;
use ed25519_dalek::Signer;
use std::{io::Write, os::unix::fs::OpenOptionsExt};
pub struct Mask {
    pub planned: conduit_face::PlannedMaskPlot,
    pub active: c::ActivePlayIdentity,
    pub wire: Vec<u8>,
    pub generation: u64,
}
pub fn prepare(host: &c::HostAdvertisement, policy: &serde_json::Value) -> Result<Mask> {
    let source = checked(conduit_environment_make::SevenSegmentMaskSource::compile())?;
    if policy["host_id"] != serde_json::to_value(&host.host_id)?
        || policy["boot_id"] != serde_json::to_value(&host.boot_id)?
        || policy["offer_generation"] != host.offer_generation.0
        || policy["source"] != serde_json::to_value(&source.role.plot_identity)?
    {
        return Err("Mask Source/current Host policy differs".into());
    }
    let grants: Vec<c::AuthorityGrant> =
        serde_json::from_value(policy["authority_grants"].clone())?;
    let planned = checked(source.plan(host, 8192, &grants))?;
    checked(conduit_face::validate_local_mask_offer(
        host, &planned, &grants,
    ))?;
    let wire = serde_json::to_vec(&planned)?;
    if wire.len() > 8192 {
        return Err("Mask complete wire bound".into());
    }
    let active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 2);
    Ok(Mask {
        planned,
        active,
        wire,
        generation: host.offer_generation.0,
    })
}
pub fn topology(mask: &Mask, plot: b::ResidentPlot) -> Result<b::BodyMaskTopology> {
    let topology = checked(conduit_face::MaskTopologyAdmission::from_plan(
        &mask.planned.plan,
    ))?;
    if topology.chains.len() != 1 {
        return Err("expected one ordinary seven-segment chain".into());
    }
    Ok(b::BodyMaskTopology {
        face: b::BodyFaceSelector {
            plot: Some(plot),
            source_placement_id: None,
        },
        chains: topology
            .chains
            .iter()
            .map(|chain| b::BodyMaskChainPlan {
                plan: mask.planned.plan.clone(),
                stage_placement_ids: chain
                    .stages
                    .iter()
                    .map(|s| s.placement_id.clone())
                    .collect(),
            })
            .collect(),
    })
}
pub fn command(
    line: &mut Line,
    authority: &Authority,
    prepared: &Prepared,
    key: &[u8; 32],
    sequence: &mut u64,
    operation: owner::Operation,
) -> Result<()> {
    let mask = prepared.mask.as_ref().ok_or("Mask not planned")?;
    let request = owner::Request {
        protocol: owner::PROTOCOL,
        operation,
        sequence: *sequence,
        host: mask.active.host_id.as_str(),
        boot: mask.active.boot_id.as_str(),
        offer_generation: mask.generation,
        body: prepared.body_plan.body_id.as_str(),
        wake: prepared.body_plan.wake_id.as_str(),
        body_plan: prepared.body_plan.plan_id.as_str(),
        body_play: prepared.play.active_play_id.as_str(),
        plan: mask.active.plan_id.as_str(),
        fragment: mask.planned.plan.fragments[0].fragment_id.as_str(),
        fragment_play: mask.active.active_play_id.as_str(),
        assignment_seal: c::assigned_plan_payload_digest(&mask.wire),
        evidence: None,
    };
    *sequence += 1;
    let transcript = checked(request.transcript())?;
    let signature = authority.key.sign(&transcript).to_bytes();
    let bytes = serde_json::to_vec(&request)?;
    line.command("mask-context", "native-mask-control-ready")?;
    line.write(&(bytes.len() as u16).to_le_bytes())?;
    line.credited(&bytes)?;
    line.credited(&signature)?;
    if operation == owner::Operation::Prepare {
        line.event("native-mask-context-ready")?;
        line.write(&(mask.wire.len() as u16).to_le_bytes())?;
        line.credited(&mask.wire)?;
    }
    let reply = line.event("native-mask-control")?;
    if reply["authenticated"] != true
        || !reply["refusal"].is_null()
        || reply["sequence"] != request.sequence
    {
        return Err(format!("Mask cold stage refused: {reply}").into());
    }
    let receipt: ack::Receipt = serde_json::from_value(reply["acknowledgement"].clone())?;
    let signature: [u8; 64] =
        serde_json::from_value::<Vec<u8>>(reply["acknowledgement_signature"].clone())?
            .try_into()
            .map_err(|_| "Mask ack signature length")?;
    checked(receipt.verify(key, &transcript, ack::State::MaskContextStaged, &signature))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(authority.state.join(match operation {
            owner::Operation::Prepare => "mask-context-staged.json",
            _ => "mask-context-retired.json",
        }))?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({"request":request,"reply":reply}),
    )?;
    file.flush()?;
    file.sync_all()?;
    std::fs::File::open(&authority.state)?.sync_all()?;
    println!(
        "{}",
        serde_json::json!({"event":"ordinary-mask-preparation-control","operation":format!("{operation:?}"),"physical_mask_plot_installed":false,"reply":reply})
    );
    Ok(())
}
