//! Finite physical acceptance under the actual retained Body owner. Canonical
//! committed publications become ordinary Body Face contributions, not fields
//! invented by a UART diagnostic or an independently scheduled sensor loop.
use super::{acknowledgement::AdmittedBoot, control, exchange, Scope};
use conduit_body::BodyLifecycleSession;
use conduit_core::ActivePlayIdentity;
use conduit_environment_make::NativeBodyBinding;
use conduit_environment_runtime::{
    publication::{PublicationChannel, PublicationSnapshot},
    publication_face::PreparedPublicationFace,
};
use conduit_face::{
    BodyFace, FaceContext, FaceContribution, FaceContributionBasis, FaceContributionRole, FaceFocus,
};
use ed25519_dalek::SigningKey;
use std::{
    error::Error,
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(
    owner: &mut BodyLifecycleSession,
    binding: &mut NativeBodyBinding,
    scope: &Scope<'_>,
    active: &ActivePlayIdentity,
    key: &SigningKey,
    admitted: &AdmittedBoot,
    output: &Path,
) -> Result<(), Box<dyn Error>> {
    binding
        .fragment_play(owner)
        .map_err(|e| format!("held owner: {e:?}"))?;
    let mut last_inspection = 5;
    let observations = (|| -> Result<(u32, u32), Box<dyn Error>> {
        // This is physical time, not the accelerated CPU fixture. Admit the
        // acceptance window from the checked cadence without changing Source.
        let inspections = scope
            .cadence_ms
            .checked_mul(3)
            .and_then(|ms| ms.checked_add(10_000))
            .ok_or("physical acceptance duration exhausted")?
            / 5_000
            + 2;
        if inspections > 40 {
            return Err("physical acceptance work bound".into());
        }
        let mut projector =
            PreparedPublicationFace::prepare().map_err(|e| format!("Face preparation: {e:?}"))?;
        let maximum_age_ms = scope.cadence_ms.checked_mul(2).ok_or("freshness bound")?;
        let release = exchange(key, scope.request(control::Operation::Release, 5, active))?;
        admitted.transition(
            &scope.request(control::Operation::Release, 5, active),
            &release,
        )?;
        fs::write(
            output.join("owner-release.json"),
            serde_json::to_vec_pretty(&release)?,
        )?;
        let mut samples = 0;
        let mut faces = 0;
        let mut last = 0;
        for index in 0..inspections {
            last_inspection = 6 + index;
            let requested_at = Instant::now();
            let reply = exchange(
                key,
                scope.request(control::Operation::Inspect, last_inspection, active),
            )?;
            if let Some(publication) = admitted.publication(
                &scope.request(control::Operation::Inspect, last_inspection, active),
                &reply,
            )? {
                let snapshot = publication.snapshot;
                let canonical = publication.canonical;
                if snapshot.sequence < last {
                    return Err("publication sequence regressed".into());
                }
                if snapshot.sequence > last {
                    binding
                        .fragment_play(owner)
                        .map_err(|e| format!("current owner: {e:?}"))?;
                    let round_trip_ms = u64::try_from(requested_at.elapsed().as_millis())
                        .map_err(|_| "inspection duration bound")?;
                    let fresh = snapshot.fresh_within(maximum_age_ms, round_trip_ms);
                    let face = observation_face(
                        &mut projector,
                        &canonical,
                        snapshot,
                        fresh,
                        owner,
                        scope,
                    )?;
                    faces += 1;
                    let available = face.properties.iter().any(|p| {
                        p.name == "sensor-status"
                            && p.value == conduit_face::FacePropertyValue::Text("available".into())
                    });
                    if available {
                        samples += 1;
                        fs::write(
                            output.join(format!("physical-observation-{samples}.bin")),
                            &canonical,
                        )?;
                        fs::write(
                            output.join(format!("physical-observation-{samples}.json")),
                            serde_json::to_vec_pretty(&reply)?,
                        )?;
                        fs::write(
                            output.join(format!("physical-face-{samples}.json")),
                            serde_json::to_vec_pretty(&face)?,
                        )?;
                    } else {
                        let stem = format!("physical-status-{}", snapshot.sequence);
                        fs::write(output.join(format!("{stem}.bin")), &canonical)?;
                        fs::write(
                            output.join(format!("{stem}.json")),
                            serde_json::to_vec_pretty(&reply)?,
                        )?;
                        fs::write(
                            output.join(format!("{stem}.face.json")),
                            serde_json::to_vec_pretty(&face)?,
                        )?;
                    }
                }
                last = snapshot.sequence;
            }
            if samples == 3 {
                break;
            }
            thread::sleep(Duration::from_millis(5_000));
        }
        Ok((samples, faces))
    })();
    // Quiesce even when the finite sensor acceptance did not obtain readings.
    let quiescence = (|| -> Result<(), Box<dyn Error>> {
        let cancelled = exchange(
            key,
            scope.request(control::Operation::Cancel, last_inspection + 1, active),
        )?;
        admitted.transition(
            &scope.request(control::Operation::Cancel, last_inspection + 1, active),
            &cancelled,
        )?;
        fs::write(
            output.join("owner-quiesced.json"),
            serde_json::to_vec_pretty(&cancelled)?,
        )?;
        Ok(())
    })();
    binding.revoke();
    if let Err(reason) = quiescence {
        fs::write(
            output.join("body-unresolved-evidence.json"),
            serde_json::to_vec_pretty(owner.evidence())?,
        )?;
        return Err(format!("quiescence unknown; Body not lulled: {reason}").into());
    }
    owner
        .lull(
            &scope.host.host_id,
            &scope.host.boot_id,
            Some(scope.body_play),
        )
        .map_err(|e| format!("Body Lull after attributed quiescence: {e:?}"))?;
    fs::write(
        output.join("body-lulled-evidence.json"),
        serde_json::to_vec_pretty(owner.evidence())?,
    )?;
    let request = scope.request(control::Operation::Retire, last_inspection + 2, active);
    let retired = exchange(key, request)?;
    admitted.transition(
        &scope.request(control::Operation::Retire, last_inspection + 2, active),
        &retired,
    )?;
    fs::write(
        output.join("owner-retired.json"),
        serde_json::to_vec_pretty(&retired)?,
    )?;
    let (samples, faces) = observations?;
    if samples != 3 {
        return Err("finite physical run did not obtain three valid observations".into());
    }
    println!(
        "{}",
        serde_json::json!({"event":"owner-run-complete", "completed":true,
        "body_id":scope.body_play.body_id,"fragment_play":active,
        "device_start_attributed":true,"owner_retained_through_execution":true,
        "physical_observations":samples,"body_faces":faces,"worker_quiesced":true,
        "body_active":false,"worker_retired":true,"overnight_recording_claimed":false})
    );
    Ok(())
}
fn observation_face(
    projector: &mut PreparedPublicationFace,
    canonical: &[u8],
    snapshot: super::acknowledgement::receipt::Snapshot,
    fresh: bool,
    owner: &BodyLifecycleSession,
    scope: &Scope<'_>,
) -> Result<conduit_face::Face, Box<dyn Error>> {
    let channel = PublicationChannel::ALL
        .get(usize::from(snapshot.channel))
        .copied()
        .ok_or("publication channel")?;
    let fragment = projector
        .project(
            PublicationSnapshot {
                channel,
                sequence: snapshot.sequence,
                canonical,
            },
            fresh,
            FaceContributionBasis {
                checked_plot_id: scope.native.plan().checked_plot_id.clone(),
                plan_id: scope.body_play.plan_id.clone(),
                active_play_id: scope.body_play.active_play_id.clone(),
                required_interaction_context: None,
            },
        )
        .map_err(|e| format!("environment contribution: {e:?}"))?;
    let face = BodyFace::project(
        &owner.evidence().body,
        owner.realization().map(|r| &r.wake),
        snapshot.sequence,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![FaceContribution::from_face(
            FaceContributionRole::Foreground,
            fragment,
        )],
    )
    .map_err(|e| format!("current Body Face: {e:?}"))?;
    Ok(face.face)
}
