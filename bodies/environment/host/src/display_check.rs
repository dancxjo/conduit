//! Attended physical checkpoint: second ordinary output Plan in the same Body.
//! The MCU supplies its actual local Face; this never sends register commands.
use crate::{
    admission::Authority, checked, native_owner_control as owner, native_owner_receipt as ack,
    planning::Prepared, transport::Line, Result,
};
use conduit_body as b;
use conduit_core as c;
use conduit_environment_make::{DisplayOutputSource, PreparedDisplayOutput};
use ed25519_dalek::Signer;
use serde_json::Value;
use std::{
    io::Write,
    time::{Duration, Instant},
};
pub struct Output {
    pub plan: c::Plan,
    pub active: c::ActivePlayIdentity,
    pub fragment: c::FragmentId,
    pub generation: u64,
    pub wire: Vec<u8>,
}
pub fn resident(source: &DisplayOutputSource) -> b::ResidentPlot {
    b::ResidentPlot::new(
        source.plot.expanded.source_document_id.clone(),
        source.plot.expanded.checked_plot_id.clone(),
    )
}
pub fn prepare(
    source: &DisplayOutputSource,
    host: &c::HostAdvertisement,
    policy: &Value,
) -> Result<Output> {
    let identity = &source.plot.expanded;
    if policy["host_id"] != serde_json::to_value(&host.host_id)?
        || policy["boot_id"] != serde_json::to_value(&host.boot_id)?
        || policy["offer_generation"] != host.offer_generation.0
        || policy["source_document_id"] != serde_json::to_value(&identity.source_document_id)?
        || policy["checked_plot_id"] != serde_json::to_value(&identity.checked_plot_id)?
        || policy["expanded_plot_id"] != serde_json::to_value(&identity.expanded_plot_id)?
    {
        return Err("display current scope/source differs".into());
    }
    let grants: Vec<c::AuthorityGrant> =
        serde_json::from_value(policy["authority_grants"].clone())?;
    let output = checked(PreparedDisplayOutput::prepare(
        source,
        host,
        &["conduit.base/local@1".into()],
        &grants,
    ))?;
    let plan = output.plan().clone();
    let fragment = plan.fragments[0].fragment_id.clone();
    let image = checked(conduit_embedded_build::generate_fore_native_plan(
        &plan,
        &fragment,
        conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
        2,
    ))?;
    let bytes = checked(image.encode(c::AssignedPlanMaxima {
        encoded_bytes: 16384,
        runtime_state_bytes: 32768,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    }))?;
    let records = checked(c::AssignedPlanRecords::new(&bytes))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| format!("{e:?}"))?;
    let material = records
        .iter()
        .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
        .copied()
        .collect::<Vec<_>>();
    let mut wire =
        vec![0; conduit_environment_runtime::appliance::assignment_transfer::MAXIMUM_WIRE_BYTES];
    let size = checked(
        conduit_environment_runtime::appliance::assignment_transfer::encode(
            &bytes[..c::ASSIGNED_PLAN_HEADER_BYTES],
            &records,
            conduit_environment_runtime::appliance::assigned::ResidentSourceIdentity::from_expanded(
                &plan.expanded_plot_id,
            ),
            &material,
            &mut wire,
        ),
    )?;
    wire.truncate(size);
    let active = c::bind_active_play(&plan.plan_id, &host.host_id, &host.boot_id, 2);
    Ok(Output {
        plan,
        active,
        fragment,
        generation: host.offer_generation.0,
        wire,
    })
}
pub fn stage(line: &mut Line, output: &Output) -> Result<()> {
    line.command(
        "display-check-assignment",
        "native-assignment-transfer-ready",
    )?;
    line.write(&output.wire)?;
    let result = line.event("native-assignment-stage")?;
    if result["staged"] != true {
        return Err(format!("display stage refused: {result}").into());
    }
    Ok(())
}
pub fn command(
    line: &mut Line,
    authority: &Authority,
    prepared: &Prepared,
    device_key: &[u8; 32],
    operation: owner::Operation,
    sequence: &mut u64,
) -> Result<Value> {
    command_with_evidence(
        line, authority, prepared, device_key, operation, sequence, None,
    )
}
#[allow(clippy::too_many_arguments)]
fn command_with_evidence(
    line: &mut Line,
    authority: &Authority,
    prepared: &Prepared,
    device_key: &[u8; 32],
    operation: owner::Operation,
    sequence: &mut u64,
    evidence: Option<owner::EvidenceReference>,
) -> Result<Value> {
    let output = prepared.display.as_ref().ok_or("display not planned")?;
    let request = owner::Request {
        protocol: owner::PROTOCOL,
        operation,
        sequence: *sequence,
        host: output.active.host_id.as_str(),
        boot: output.active.boot_id.as_str(),
        offer_generation: output.generation,
        body: prepared.body_plan.body_id.as_str(),
        wake: prepared.body_plan.wake_id.as_str(),
        body_plan: prepared.body_plan.plan_id.as_str(),
        body_play: prepared.play.active_play_id.as_str(),
        plan: output.active.plan_id.as_str(),
        fragment: output.fragment.as_str(),
        fragment_play: output.active.active_play_id.as_str(),
        assignment_seal: c::assigned_plan_payload_digest(&output.wire),
        evidence,
    };
    *sequence += 1;
    let transcript = checked(request.transcript())?;
    let signature = authority.key.sign(&transcript).to_bytes();
    let bytes = serde_json::to_vec(&request)?;
    line.command("display-check-control", "native-owner-control-ready")?;
    line.write(&(bytes.len() as u16).to_le_bytes())?;
    line.credited(&bytes)?;
    line.credited(&signature)?;
    let reply = line.event("native-display-check-control")?;
    if reply["authenticated"] != true
        || !reply["refusal"].is_null()
        || reply["sequence"] != request.sequence
    {
        return Err(format!("display {operation:?} refused: {reply}").into());
    }
    if matches!(
        operation,
        owner::Operation::Start
            | owner::Operation::Release
            | owner::Operation::Cancel
            | owner::Operation::Retire
            | owner::Operation::AcceptEvidence
    ) {
        let receipt: ack::Receipt = serde_json::from_value(reply["acknowledgement"].clone())?;
        let signature: [u8; 64] =
            serde_json::from_value::<Vec<u8>>(reply["acknowledgement_signature"].clone())?
                .try_into()
                .map_err(|_| "display ack length")?;
        let expected = match operation {
            owner::Operation::Start => ack::State::StartedHeld,
            owner::Operation::Release => ack::State::Released,
            owner::Operation::Cancel => ack::State::Quiesced,
            owner::Operation::Retire => ack::State::Retired,
            owner::Operation::AcceptEvidence => ack::State::EvidenceAccepted,
            _ => unreachable!(),
        };
        checked(receipt.verify(device_key, &transcript, expected, &signature))?;
    }
    use std::os::unix::fs::OpenOptionsExt;
    let page_size = reply["evidence_bytes"]
        .as_u64()
        .ok_or("display evidence length")? as usize;
    if page_size > conduit_environment_runtime::display_evidence::MAXIMUM_BYTES {
        return Err("display evidence bound".into());
    }
    if page_size > 0 {
        if operation != owner::Operation::Inspect {
            return Err("unexpected display evidence".into());
        }
        let bytes = line.exact(page_size)?;
        let offer: crate::native_display_evidence::Offer =
            serde_json::from_value(reply["evidence"].clone())?;
        let signature: [u8; 64] =
            serde_json::from_value::<Vec<u8>>(reply["evidence_signature"].clone())?
                .try_into()
                .map_err(|_| "display evidence signature length")?;
        checked(offer.verify(device_key, &transcript, &signature, &bytes))?;
        crate::display_evidence::retain(authority, &request, &reply, &offer, &bytes)?;
        command_with_evidence(
            line,
            authority,
            prepared,
            device_key,
            owner::Operation::AcceptEvidence,
            sequence,
            Some(offer.page),
        )?;
    } else if !reply["evidence"].is_null() {
        return Err("display evidence metadata without page".into());
    }
    if operation != owner::Operation::Inspect || page_size > 0 {
        let mut evidence = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(authority.state.join("display-control.ndjson"))?;
        writeln!(
            evidence,
            "{}",
            serde_json::json!({"request":request,"reply":reply})
        )?;
        evidence.sync_all()?;
        println!(
            "{}",
            serde_json::json!({"event":"native-display-physical-check","operation":format!("{operation:?}"),"reply":reply})
        );
    }
    Ok(reply)
}
pub fn finish(
    line: &mut Line,
    authority: &Authority,
    prepared: &Prepared,
    device_key: &[u8; 32],
    sequence: &mut u64,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let needs_cancel;
    loop {
        let reply = command(
            line,
            authority,
            prepared,
            device_key,
            owner::Operation::Inspect,
            sequence,
        )?;
        match reply["worker"]["phase"].as_str() {
            Some("drained" | "evidence-pending" | "waiting-input")
                if reply["worker"]["native_calls"]
                    .as_u64()
                    .is_some_and(|n| n >= 2)
                    && reply["worker"]["cleanup_failure"] == false =>
            {
                needs_cancel = reply["worker"]["phase"] != "drained";
                break;
            }
            Some("failed" | "cleanup-refused" | "ownership-refused" | "quiesced" | "retired") => {
                return Err(format!("display checkpoint failed: {reply}").into())
            }
            _ => (),
        }
        if Instant::now() >= deadline {
            return Err("display completion deadline".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    if needs_cancel {
        command(
            line,
            authority,
            prepared,
            device_key,
            owner::Operation::Cancel,
            sequence,
        )?;
    }
    drain_evidence(line, authority, prepared, device_key, sequence)?;
    command(
        line,
        authority,
        prepared,
        device_key,
        owner::Operation::Retire,
        sequence,
    )?;
    Ok(())
}

pub fn cleanup(
    line: &mut Line,
    authority: &Authority,
    prepared: &Prepared,
    device_key: &[u8; 32],
    sequence: &mut u64,
) -> Result<()> {
    let reply = command(
        line,
        authority,
        prepared,
        device_key,
        owner::Operation::Inspect,
        sequence,
    )?;
    if reply["worker"]["phase"] != "drained" {
        command(
            line,
            authority,
            prepared,
            device_key,
            owner::Operation::Cancel,
            sequence,
        )?;
    }
    drain_evidence(line, authority, prepared, device_key, sequence)?;
    command(
        line,
        authority,
        prepared,
        device_key,
        owner::Operation::Retire,
        sequence,
    )?;
    Ok(())
}

fn drain_evidence(
    line: &mut Line,
    authority: &Authority,
    prepared: &Prepared,
    key: &[u8; 32],
    sequence: &mut u64,
) -> Result<()> {
    for _ in 0..3 {
        let reply = command(
            line,
            authority,
            prepared,
            key,
            owner::Operation::Inspect,
            sequence,
        )?;
        if reply["evidence_bytes"] == 0 {
            return Ok(());
        }
    }
    Err("display evidence did not quiesce".into())
}
