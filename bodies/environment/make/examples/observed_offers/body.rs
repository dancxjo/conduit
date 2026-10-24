//! Retained local owner state from this invocation's attended admission proof.
//! A remotely supplied biography/boolean is not an authentication mechanism.
use conduit_body::{BodyBiographyEvidence, BodyLifecycleSession, WakeLifecycle};
use conduit_core as c;
use conduit_environment_make::PreparedNativeAppliance;
use std::{error::Error, fs, io::Read, path::Path};
fn document(path: &Path) -> Result<serde_json::Value, Box<dyn Error>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 128 * 1024 {
        return Err("owner capture capacity exceeded".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn propose(
    prepared: &PreparedNativeAppliance,
    native_offer: &c::HostAdvertisement,
    admission_directory: &Path,
    output: &Path,
) -> Result<BodyLifecycleSession, Box<dyn Error>> {
    propose_plan(prepared.plan(), native_offer, admission_directory, output)
}
pub fn propose_plan(
    plan: &c::Plan,
    native_offer: &c::HostAdvertisement,
    admission_directory: &Path,
    output: &Path,
) -> Result<BodyLifecycleSession, Box<dyn Error>> {
    let observations = document(&admission_directory.join("observations.json"))?;
    let observed: c::HostAdvertisement = serde_json::from_value(
        observations
            .as_array()
            .and_then(|frames| frames.first())
            .ok_or("missing attended offer")?["advertisement"]
            .clone(),
    )?;
    if observed != *native_offer {
        return Err("Root facts changed since admitted observation".into());
    }
    let retained = document(&admission_directory.join("admission.json"))?;
    let evidence: BodyBiographyEvidence = serde_json::from_value(retained["biography"].clone())?;
    evidence
        .validate()
        .map_err(|e| format!("owner evidence: {e:?}"))?;
    let mut owner =
        BodyLifecycleSession::open_admitted(evidence, &native_offer.host_id, &native_offer.boot_id)
            .map_err(|e| format!("owner scope: {e:?}"))?;
    // No Mask topology is inferred from hardware inventory or a display sink.
    // Its actual renderer and interaction realization need separate admission.
    owner
        .propose(
            vec![conduit_body::BodyPlotPlan {
                plot: conduit_body::ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan: plan.clone(),
            }],
            &native_offer.host_id,
            &native_offer.boot_id,
        )
        .map_err(|e| format!("Body proposal: {e:?}"))?;
    let realization = owner.realization().ok_or("proposal missing")?;
    if realization.wake.lifecycle != WakeLifecycle::AwaitingPlan
        || realization.play.is_some()
        || realization.plan.plots.len() != 1
        || realization.plan.plots[0].plan != *plan
    {
        return Err("unexpected Body proposal transition".into());
    }
    fs::write(
        output.join("body-proposal.json"),
        serde_json::to_vec_pretty(realization)?,
    )?;
    fs::write(
        output.join("body-evidence.json"),
        serde_json::to_vec_pretty(owner.evidence())?,
    )?;
    fs::write(
        output.join("body-construction.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "proof_class":"host-body-proposal-from-physical-admission-and-current-root-offers",
            "body_id":realization.plan.body_id, "body_plan_id":realization.plan.plan_id,
            "plot_plan_id":plan.plan_id, "host_id":native_offer.host_id,
            "boot_id":native_offer.boot_id, "wake":"AwaitingPlan", "masks":0,
            "play_started":false, "body_installed_on_mcu":false,
            "capability_possession_issued":false,
        }))?,
    )?;
    Ok(owner)
}
