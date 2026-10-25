//! Signed remote custody beneath the ordinary hosted Body lifecycle.
use crate::{
    admission::Authority, checked, native_owner_control as owner, native_owner_receipt as ack,
    planning::Prepared, transport::Line, Result,
};
use conduit_body as b;
use ed25519_dalek::Signer;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    io::Write,
    time::{Duration, Instant},
};
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

struct Exchange {
    reply: Value,
    request: Value,
    publication: Vec<u8>,
    receipt: Option<ack::Receipt>,
}
fn command(
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
pub fn run(
    line: &mut Line,
    authority: &Authority,
    body: &mut b::BodyLifecycleSession,
    prepared: Prepared,
    device_key: [u8; 32],
    limit: u32,
) -> Result<()> {
    let mut face = crate::observation::prepare(&prepared)?;
    // Signal handlers only request cancellation; signed native cleanup stays
    // in the ordinary owner flow, with its existing finite carrier deadlines.
    unsafe {
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
    }
    line.command("assignment-transfer", "native-assignment-transfer-ready")?;
    // This framing embeds its own exact u16 extent. Native effects are still held.
    line.write(&prepared.wire)?;
    let staged = line.event("native-assignment-stage")?;
    if staged["staged"] != true {
        return Err(format!("assignment refused: {staged}").into());
    }
    if let Some(display) = &prepared.display {
        crate::display_check::stage(line, display)?;
    }
    let mut sequence = 1;
    let mut preparation_attempted = false;
    let mut display_preparation_attempted = false;
    let mut display_retired = false;
    let mut body_started = false;
    let result = (|| -> Result<()> {
        for operation in [
            owner::Operation::Reserve,
            owner::Operation::Prepare,
            owner::Operation::Start,
        ] {
            if operation == owner::Operation::Prepare {
                preparation_attempted = true;
            }
            let current = sequence;
            sequence += 1;
            let exchange = command(line, authority, &prepared, &device_key, operation, current)?;
            println!(
                "{}",
                serde_json::json!({"event":"native-host-control","operation":format!("{operation:?}"),"reply":exchange.reply})
            );
            if prepared.display.is_some() {
                if operation == owner::Operation::Reserve {
                    crate::display_check::command(
                        line,
                        authority,
                        &prepared,
                        &device_key,
                        owner::Operation::Reserve,
                        &mut sequence,
                    )?;
                    display_preparation_attempted = true;
                    crate::display_check::command(
                        line,
                        authority,
                        &prepared,
                        &device_key,
                        owner::Operation::Prepare,
                        &mut sequence,
                    )?;
                } else if operation == owner::Operation::Start {
                    crate::display_check::command(
                        line,
                        authority,
                        &prepared,
                        &device_key,
                        owner::Operation::Start,
                        &mut sequence,
                    )?;
                }
            }
        }
        let wake = checked(
            prepared.wake.body_plan_ready(
                &prepared.body_plan,
                conduit_core::bind_sign(
                    &authority.host.host_id,
                    &authority.host.boot_id,
                    Some(&prepared.play.active_play_id),
                    0,
                )
                .sign_id,
            ),
        )?;
        let wake = checked(
            wake.body_play_started(
                &prepared.body_plan,
                &prepared.play,
                conduit_core::bind_sign(
                    &authority.host.host_id,
                    &authority.host.boot_id,
                    Some(&prepared.play.active_play_id),
                    1,
                )
                .sign_id,
            ),
        )?;
        checked(body.started(
            &authority.host.host_id,
            &authority.host.boot_id,
            prepared.play.clone(),
            wake,
        ))?;
        body_started = true;
        authority.save(body, "body-playing.json")?;
        let current = sequence;
        sequence += 1;
        command(
            line,
            authority,
            &prepared,
            &device_key,
            owner::Operation::Release,
            current,
        )?;
        if prepared.display.is_some() {
            crate::display_check::command(
                line,
                authority,
                &prepared,
                &device_key,
                owner::Operation::Release,
                &mut sequence,
            )?;
        }
        println!(
            "{}",
            serde_json::json!({"event":"host-owned-body-playing","body":body.evidence().body_id,"host":prepared.active.host_id,"boot":prepared.active.boot_id,"plan":prepared.active.plan_id,"play":prepared.play.active_play_id})
        );
        let deadline = Instant::now() + Duration::from_secs(u64::from(limit) * 60 + 30);
        let mut observed = false;
        let mut publications = 0;
        let mut output = std::fs::File::create(authority.state.join("native-observations.ndjson"))?;
        while Instant::now() < deadline && !STOP.load(Ordering::Relaxed) {
            if prepared.display.is_some() {
                crate::display_check::command(
                    line,
                    authority,
                    &prepared,
                    &device_key,
                    owner::Operation::Inspect,
                    &mut sequence,
                )?;
            }
            let before = Instant::now();
            let current = sequence;
            sequence += 1;
            let exchange = command(
                line,
                authority,
                &prepared,
                &device_key,
                owner::Operation::Inspect,
                current,
            )?;
            if let Some(receipt) = exchange.receipt {
                let snapshot = receipt.snapshot.ok_or("observed receipt lacks snapshot")?;
                if snapshot.sequence <= face.last_source_sequence() {
                    crate::observation::age(&mut face, snapshot.observed_millis, &authority.state)?;
                    if matches!(
                        exchange.reply["worker"]["phase"].as_str(),
                        Some(
                            "failed"
                                | "drained"
                                | "cleanup-refused"
                                | "ownership-refused"
                                | "retired"
                        )
                    ) {
                        return Err(format!(
                            "native worker ended before requested publication count: {}",
                            exchange.reply["worker"]
                        )
                        .into());
                    }
                    std::thread::sleep(Duration::from_millis(250));
                    continue;
                }
                let channels = conduit_environment_runtime::publication::PublicationChannel::ALL;
                let channel = *channels
                    .get(snapshot.channel as usize)
                    .ok_or("publication channel")?;
                checked(channel.validate_value(&exchange.publication))?;
                let evidence = serde_json::json!({"body":body.evidence().body_id,"host":prepared.active.host_id,"boot":prepared.active.boot_id,
                "plan":prepared.active.plan_id,"play":prepared.active.active_play_id,"receipt":receipt,
                "request":exchange.request,"root_key":device_key,"root_signature":exchange.reply["acknowledgement_signature"],"canonical_bytes":exchange.publication,"delivery_round_trip_ms":before.elapsed().as_millis()});
                writeln!(output, "{evidence}")?;
                output.sync_all()?;
                println!(
                    "{}",
                    serde_json::json!({"event":"verified-native-publication","channel":snapshot.channel,"sequence":snapshot.sequence,"bytes":snapshot.bytes,"heap":exchange.reply["heap_telemetry"],"face":exchange.reply["face"]})
                );
                // A timeout or unavailable result proves execution, but does not
                // establish that this physical sensor produced a measurement.
                observed |= crate::observation::publish(
                    &mut face,
                    snapshot.sequence,
                    channel,
                    &exchange.publication,
                    snapshot.committed_millis,
                    snapshot.observed_millis,
                    &authority.state,
                )?;
                publications += 1;
                if publications == limit {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        if prepared.display.is_some() {
            crate::display_check::finish(line, authority, &prepared, &device_key, &mut sequence)?;
            display_retired = true;
        }
        if !observed {
            return Err("no verified physical measurement in this acceptance run".into());
        }
        if publications != limit && !STOP.load(Ordering::Relaxed) {
            return Err(format!("publication deadline: received {publications} of {limit}").into());
        }
        Ok(())
    })();
    // Errors after preparation still require actual quiescence and retirement.
    // A missing receipt leaves the retained Body evidence unresolved.
    let cleanup: Result<()> = {
        let display_cleanup = if display_preparation_attempted && !display_retired {
            crate::display_check::cleanup(line, authority, &prepared, &device_key, &mut sequence)
        } else {
            Ok(())
        };
        let sampler_cleanup = (|| -> Result<()> {
            if preparation_attempted {
                command(
                    line,
                    authority,
                    &prepared,
                    &device_key,
                    owner::Operation::Cancel,
                    sequence,
                )?;
                command(
                    line,
                    authority,
                    &prepared,
                    &device_key,
                    owner::Operation::Retire,
                    sequence + 1,
                )?;
                if body_started && display_cleanup.is_ok() {
                    checked(body.lull(
                        &authority.host.host_id,
                        &authority.host.boot_id,
                        Some(&prepared.play),
                    ))?;
                    authority.save(body, "body-lulled.json")?;
                }
            }
            Ok(())
        })();
        match (display_cleanup, sampler_cleanup) {
            (Err(a), Err(b)) => Err(format!("display cleanup: {a}; sampler cleanup: {b}").into()),
            (Err(a), _) | (_, Err(a)) => Err(a),
            _ => Ok(()),
        }
    };
    authority.save(body, "body-final.json")?;
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; cleanup unresolved: {cleanup}").into()),
        (Err(error), _) => Err(error),
        (_, cleanup) => cleanup,
    }
}
