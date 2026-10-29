//! Authenticated owner exchange. Preparation and lifecycle operations are
//! single attempts; a missing reply never authorizes replay of an effect.
use crate::{
    admission::Authority, checked, native_owner_control as owner, native_owner_receipt as ack,
    planning::Prepared, transport::Line, Result,
};
use ed25519_dalek::Signer;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write;
pub(super) struct Exchange {
    pub reply: Value,
    pub request: Value,
    pub publication: Vec<u8>,
    pub receipt: Option<ack::Receipt>,
}
pub(super) fn command(
    line: &mut Line,
    authority: &Authority,
    prepared: &Prepared,
    device_key: &[u8; 32],
    operation: owner::Operation,
    sequence: u64,
) -> Result<Exchange> {
    let request = owner::Request {
        protocol: owner::PROTOCOL,
        operation,
        sequence,
        host: prepared.active.host_id.as_str(),
        boot: prepared.active.boot_id.as_str(),
        offer_generation: prepared.generation,
        body: prepared.body_plan.body_id.as_str(),
        wake: prepared.body_plan.wake_id.as_str(),
        body_plan: prepared.body_plan.plan_id.as_str(),
        body_play: prepared.play.active_play_id.as_str(),
        plan: prepared.active.plan_id.as_str(),
        fragment: prepared.fragment.as_str(),
        fragment_play: prepared.active.active_play_id.as_str(),
        assignment_seal: conduit_core::assigned_plan_payload_digest(&prepared.wire),
        evidence: None,
    };
    let transcript = checked(request.transcript())?;
    let signature = authority.key.sign(&transcript).to_bytes();
    let bytes = serde_json::to_vec(&request)?;
    line.command("owner-control", "native-owner-control-ready")?;
    line.write(&(bytes.len() as u16).to_le_bytes())?;
    line.credited(&bytes)?;
    line.credited(&signature)?;
    if operation == owner::Operation::Prepare {
        line.event("native-owner-assignment-ready")?;
        line.write(&(prepared.wire.len() as u16).to_le_bytes())?;
        line.credited(&prepared.wire)?;
    }
    let reply = line.event("native-owner-control")?;
    if reply["authenticated"] != true
        || !reply["refusal"].is_null()
        || reply["sequence"] != sequence
    {
        return Err(format!("Root refused authenticated {operation:?}: {reply}").into());
    }
    let size = reply["publication_bytes"]
        .as_u64()
        .ok_or("publication extent")? as usize;
    if size > 4096 {
        return Err("publication extent bound".into());
    }
    let publication = line.exact(size)?;
    let receipt = if reply["acknowledgement"].is_null() {
        None
    } else {
        let receipt: ack::Receipt = serde_json::from_value(reply["acknowledgement"].clone())?;
        let signature: [u8; 64] =
            serde_json::from_value::<Vec<u8>>(reply["acknowledgement_signature"].clone())?
                .try_into()
                .map_err(|_| "acknowledgement signature length")?;
        let expected = match operation {
            owner::Operation::Start => ack::State::StartedHeld,
            owner::Operation::Release => ack::State::Released,
            owner::Operation::Cancel => ack::State::Quiesced,
            owner::Operation::Retire => ack::State::Retired,
            owner::Operation::Inspect => ack::State::Observed,
            _ => return Err("unexpected acknowledgement for preparation".into()),
        };
        checked(receipt.verify(device_key, &transcript, expected, &signature))?;
        if let Some(snapshot) = receipt.snapshot {
            if snapshot.bytes as usize != publication.len()
                || snapshot.sha256 != <[u8; 32]>::from(Sha256::digest(&publication))
            {
                return Err("publication differs from signed receipt".into());
            }
        }
        Some(receipt)
    };
    if matches!(
        operation,
        owner::Operation::Start
            | owner::Operation::Release
            | owner::Operation::Cancel
            | owner::Operation::Retire
    ) && receipt.is_none()
    {
        return Err("missing signed lifecycle acknowledgement".into());
    }
    let request = serde_json::to_value(&request)?;
    if operation != owner::Operation::Inspect {
        use std::os::unix::fs::OpenOptionsExt;
        let mut retained = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(authority.state.join("native-control.ndjson"))?;
        writeln!(
            retained,
            "{}",
            serde_json::json!({"request":request,"root_key":device_key,"reply":reply})
        )?;
        retained.sync_all()?;
    }
    Ok(Exchange {
        reply,
        request,
        publication,
        receipt,
    })
}
