//! Attributed lifecycle for the real registered task, with no Frame producer.
use super::{acknowledgement::AdmittedBoot, control, exchange, Scope};
use conduit_body::BodyLifecycleSession;
use conduit_core as c;
use conduit_environment_make::NativeBodyBinding;
use ed25519_dalek::SigningKey;
use std::{error::Error, fs, path::Path, thread, time::Duration};
fn save(output: &Path, name: &str, value: &impl serde::Serialize) -> Result<(), Box<dyn Error>> {
    fs::write(output.join(name), serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
pub(super) fn run(
    scope: &Scope<'_>,
    key: &SigningKey,
    owner: &mut BodyLifecycleSession,
    binding: &mut NativeBodyBinding,
    active: &c::ActivePlayIdentity,
    admission: &Path,
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    let admitted = AdmittedBoot::load(admission, scope.host)?;
    let started = exchange(key, scope.request(control::Operation::Start, 4, active))?;
    admitted.transition(
        &scope.request(control::Operation::Start, 4, active),
        &started,
    )?;
    if started["control_applied"] != true
        || !started["refusal"].is_null()
        || started["display_worker"]["phase"] != "started-held"
        || started["display_worker"]["steps"] != 0
        || started["display_worker"]["native_calls"] != 0
        || started["display_worker"]["software_calls"] != 0
    {
        return Err("attributed display held Start refused".into());
    }
    let current = owner.realization().ok_or("Body proposal missing")?;
    let sign = |sequence| {
        c::bind_sign(
            &scope.host.host_id,
            &scope.host.boot_id,
            Some(&scope.body_play.active_play_id),
            sequence,
        )
        .sign_id
    };
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .and_then(|wake| wake.body_play_started(&current.plan, scope.body_play, sign(1)))
        .map_err(|e| format!("display lifecycle: {e:?}"))?;
    owner
        .started(
            &scope.host.host_id,
            &scope.host.boot_id,
            scope.body_play.clone(),
            wake,
        )
        .map_err(|e| format!("display Body Start: {e:?}"))?;
    binding
        .confirm_started(owner)
        .map_err(|e| format!("display custody: {e:?}"))?;
    if binding
        .fragment_play(owner)
        .map_err(|e| format!("current display custody: {e:?}"))?
        != active
    {
        return Err("display fragment Play correlation".into());
    }
    save(output, "owner-start-held.json", &started)?;
    save(output, "body-started-evidence.json", owner.evidence())?;
    let mut sequence = 5;
    let awaiting = (|| -> Result<(), Box<dyn Error>> {
        let released = exchange(
            key,
            scope.request(control::Operation::Release, sequence, active),
        )?;
        admitted.transition(
            &scope.request(control::Operation::Release, sequence, active),
            &released,
        )?;
        sequence += 1;
        if released["control_applied"] != true || !released["refusal"].is_null() {
            return Err("attributed display Release refused".into());
        }
        save(output, "owner-release.json", &released)?;
        // Only read-only inspection is polled. No Frame or action is retried.
        for _ in 0..8 {
            let inspected = exchange(
                key,
                scope.request(control::Operation::Inspect, sequence, active),
            )?;
            sequence += 1;
            let task = &inspected["display_worker"];
            if task["phase"] == "waiting-input"
                && task["task_active"] == true
                && task["progress_current"] == false
            {
                save(output, "owner-waiting-input.json", &inspected)?;
                return Ok(());
            }
            if matches!(
                task["phase"].as_str(),
                Some("failed" | "drained" | "cleanup-refused")
            ) {
                return Err("display task terminated before input wait".into());
            }
            thread::sleep(Duration::from_millis(200));
        }
        Err("display input wait observation bound".into())
    })();
    // Quiesce and return custody even if release/observation failed.
    sequence += 1;
    let quiescence = (|| -> Result<serde_json::Value, Box<dyn Error>> {
        let cancelled = exchange(
            key,
            scope.request(control::Operation::Cancel, sequence, active),
        )?;
        admitted.transition(
            &scope.request(control::Operation::Cancel, sequence, active),
            &cancelled,
        )?;
        let task = &cancelled["display_worker"];
        if cancelled["control_applied"] != true
            || !cancelled["refusal"].is_null()
            || task["phase"] != "quiesced"
            || task["task_active"] != false
            || task["progress_current"] != true
            || task["cleanup_failure"] != false
            || task["native_lifecycle"] != "revoked"
            || task["native_in_flight"] != 0
            || task["native_calls"] != 0
            || task["software_calls"] != 0
        {
            return Err("display quiescence/counters refused".into());
        }
        Ok(cancelled)
    })();
    binding.revoke();
    let cancelled = match quiescence {
        Ok(value) => value,
        Err(reason) => {
            save(output, "body-unresolved-evidence.json", owner.evidence())?;
            return Err(reason);
        }
    };
    let task = &cancelled["display_worker"];
    save(output, "owner-quiesced.json", &cancelled)?;
    owner
        .lull(
            &scope.host.host_id,
            &scope.host.boot_id,
            Some(scope.body_play),
        )
        .map_err(|e| format!("display Body Lull: {e:?}"))?;
    save(output, "body-lulled-evidence.json", owner.evidence())?;
    sequence += 1;
    let retired = exchange(
        key,
        scope.request(control::Operation::Retire, sequence, active),
    )?;
    admitted.transition(
        &scope.request(control::Operation::Retire, sequence, active),
        &retired,
    )?;
    if retired["control_applied"] != true
        || !retired["refusal"].is_null()
        || retired["display_worker"]["phase"] != "retired"
        || retired["display_worker"]["native_custody"] != false
    {
        return Err("display retirement refused".into());
    }
    save(output, "owner-retired.json", &retired)?;
    awaiting?;
    let steps = task["steps"].as_u64().ok_or("display Step counter")?;
    if steps == 0 {
        return Err("released task advanced no kernel Step".into());
    }
    println!(
        "{}",
        serde_json::json!({"event":"owner-run-complete","completed":true,
        "display_only":true,"body_id":scope.body_play.body_id,"fragment_play":active,
        "device_start_attributed":true,"owner_retained_through_execution":true,
        "input_wait_observed":true,"kernel_steps":steps,"native_calls":0,"software_calls":0,
        "worker_quiesced":true,"worker_retired":true,"body_active":false,
        "frame_submitted":false,"physical_display_delivery":false,"climate_sampling_claimed":false})
    );
    Ok(())
}
