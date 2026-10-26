//! Signed remote custody beneath the ordinary hosted Body lifecycle.
use crate::{
    admission::Authority, checked, native_owner_control as owner, owner_exchange::command,
    planning::Prepared, transport::Line, Result,
};
use conduit_body as b;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    io::Write,
    time::{Duration, Instant},
};
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

pub fn run(
    line: &mut Line,
    authority: &Authority,
    body: &mut b::BodyLifecycleSession,
    prepared: Prepared,
    device_key: [u8; 32],
    limit: u32,
    history_peer: Option<std::net::SocketAddr>,
) -> Result<()> {
    let mut face = crate::observation::prepare(&prepared)?;
    let mut history = crate::observed_history::Collector::new(&prepared, &device_key)?;
    let history_peer = history_peer.or_else(|| line.http_peer());
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
    let mut mask_staged = false;
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
            if prepared.mask.is_some() && operation == owner::Operation::Reserve {
                crate::mask::command(
                    line,
                    authority,
                    &prepared,
                    &device_key,
                    &mut sequence,
                    owner::Operation::Prepare,
                )?;
                mask_staged = true;
            }
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
        let mut retained_sequence = 0;
        let mut output = std::fs::File::create(authority.state.join("native-observations.ndjson"))?;
        while Instant::now() < deadline && !STOP.load(Ordering::Relaxed) {
            if prepared.display.is_some() {
                if let Err(error) = crate::display_check::command(
                    line,
                    authority,
                    &prepared,
                    &device_key,
                    owner::Operation::Inspect,
                    &mut sequence,
                ) {
                    if line.can_reconnect() && crate::transport::is_recoverable(error.as_ref()) {
                        crate::owner_reconnect::recover(
                            line,
                            authority,
                            &prepared,
                            &device_key,
                            &mut sequence,
                            deadline,
                            &STOP,
                        )?;
                        continue;
                    }
                    return Err(error);
                }
            }
            let before = Instant::now();
            let current = sequence;
            sequence += 1;
            let exchange = match command(
                line,
                authority,
                &prepared,
                &device_key,
                owner::Operation::Inspect,
                current,
            ) {
                Ok(exchange) => exchange,
                Err(error)
                    if line.can_reconnect() && crate::transport::is_recoverable(error.as_ref()) =>
                {
                    crate::owner_reconnect::recover(
                        line,
                        authority,
                        &prepared,
                        &device_key,
                        &mut sequence,
                        deadline,
                        &STOP,
                    )?;
                    continue;
                }
                Err(error) => return Err(error),
            };
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
                if snapshot.sequence > retained_sequence {
                    writeln!(output, "{evidence}")?;
                    output.sync_all()?;
                    retained_sequence = snapshot.sequence;
                }
                println!(
                    "{}",
                    serde_json::json!({"event":"verified-native-publication","channel":snapshot.channel,"sequence":snapshot.sequence,"bytes":snapshot.bytes,"heap":exchange.reply["heap_telemetry"],"face":exchange.reply["face"]})
                );
                if !exchange.reply["observation_history"].is_null() {
                    if let Some(peer) = history_peer {
                        match history.collect(peer, &authority.state) {
                            Ok(()) => println!(
                                "{}",
                                serde_json::json!({"event":"verified-observation-history", "through_sequence":history.after})
                            ),
                            Err(error) => eprintln!(
                                "observation history remains on MCU; collection deferred: {error}"
                            ),
                        }
                    }
                }
                if snapshot.sequence > face.last_source_sequence() + 1 {
                    if let Err(error) = history.replay_gap(
                        &authority.state,
                        &mut face,
                        snapshot.sequence,
                        snapshot.observed_millis,
                    ) {
                        eprintln!("Face gap remains pending signed history: {error}");
                    }
                    if snapshot.sequence > face.last_source_sequence() + 1 {
                        crate::observation::age(
                            &mut face,
                            snapshot.observed_millis,
                            &authority.state,
                        )?;
                        std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                }
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
                publications = face.last_source_sequence().try_into()?;
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
        let mask_cleanup = if mask_staged {
            crate::mask::command(
                line,
                authority,
                &prepared,
                &device_key,
                &mut sequence,
                owner::Operation::Retire,
            )
        } else {
            Ok(())
        };
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
                if body_started && display_cleanup.is_ok() && mask_cleanup.is_ok() {
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
        let display_cleanup = display_cleanup.and(mask_cleanup);
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
