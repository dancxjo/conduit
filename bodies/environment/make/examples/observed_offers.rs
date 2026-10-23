//! Internal stage of `cargo xtask prove environment software-ready-esp32`.
//! Consume that invocation's local UART capture, never synthesize native facts.
use conduit_environment_make::{compile_appliance_source, NativeSoftwareSourceRecipe};
use std::{error::Error, fs, io::Read, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = PathBuf::from(args.next().ok_or("expected capture directory")?);
    if args.next().is_some() {
        return Err("expected one capture directory".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(directory.join("replies.json"))?
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 128 * 1024 {
        return Err("capture exceeded finite host ingress bound".into());
    }
    let replies: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
    let [before, software, policy, after] = replies.as_slice() else {
        return Err("expected four scoped Root replies".into());
    };
    for root in [before, after] {
        if root["event"] != "native-root-offer"
            || root["body_installed"] != false
            || root["native_plan_installed"] != false
        {
            return Err("unexpected Root preparation state".into());
        }
    }
    let source = compile_appliance_source().map_err(|why| format!("Source: {why:?}"))?;
    let recipe = NativeSoftwareSourceRecipe::prepare(&source)
        .map_err(|why| format!("Source recipe: {why:?}"))?;
    let advertisement = recipe
        .observe_native_offers(
            &serde_json::from_value(before["advertisement"].clone())?,
            &serde_json::to_vec(software)?,
            &serde_json::from_value(after["advertisement"].clone())?,
        )
        .map_err(|why| format!("Root offer composition: {why:?}"))?;
    let grants = recipe
        .observe_appliance_policy(&advertisement, &serde_json::to_vec(policy)?)
        .map_err(|why| format!("Root policy: {why:?}"))?;
    let plan = source.plan_local(&advertisement, &["conduit.base/local@1".into()], &grants)?;
    if !conduit_core::verify_plan(&plan) {
        return Err("ordinary Plan verification refused".into());
    }
    fs::write(
        directory.join("uninstalled-plan.json"),
        serde_json::to_vec_pretty(&plan)?,
    )?;
    let receipt = serde_json::json!({
        "proof_class": "source-checked-appliance-planning-from-captured-root-policy",
        "host_id": advertisement.host_id,
        "boot_id": advertisement.boot_id,
        "offer_generation": advertisement.offer_generation,
        "capabilities": advertisement.capabilities.len(),
        "bases": advertisement.bases.len(),
        "resources": advertisement.resources.len(),
        "authority_grants_observed": grants.len(),
        "plan_prepared": true,
        "plan_id": plan.plan_id,
        "fragment_count": plan.fragments.len(),
        "placements": plan.fragments[0].placements.len(),
        "cords": plan.fragments[0].connections.len(),
        "capability_possession_issued": false,
        "body_started": false,
        "plan_installed": false
    });
    fs::write(
        directory.join("planning-advertisement.json"),
        serde_json::to_vec_pretty(&advertisement)?,
    )?;
    fs::write(
        directory.join("composition.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    println!("{receipt}");
    Ok(())
}
