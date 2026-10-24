//! Retain the real admitted Body owner through dormant display preparation.
//! No Start, Release, Frame submission or physical output is requested here.
extern crate alloc;
#[allow(dead_code)]
#[path = "observed_offers/body.rs"]
mod body;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_owner_control.rs"]
mod control;
#[path = "native_owner/exchange.rs"]
mod exchange;
#[path = "native_owner/key.rs"]
mod key;
use conduit_body::BodyPlayIdentity;
use conduit_core as c;
use conduit_environment_make::{DisplayOutputSource, NativeBodyBinding, PreparedDisplayOutput};
use exchange::{document, exchange};
use std::{error::Error, fs, io::Read, path::PathBuf};
struct Scope<'a> {
    plan: &'a c::Plan,
    host: &'a c::HostAdvertisement,
    body_play: &'a BodyPlayIdentity,
    seal: [u8; 32],
}
impl Scope<'_> {
    fn request<'a>(
        &'a self,
        operation: control::Operation,
        sequence: u64,
        active: &'a c::ActivePlayIdentity,
    ) -> control::Request<'a> {
        control::Request {
            protocol: control::PROTOCOL,
            operation,
            sequence,
            host: self.host.host_id.as_str(),
            boot: self.host.boot_id.as_str(),
            offer_generation: self.host.offer_generation.0,
            body: self.body_play.body_id.as_str(),
            wake: self.body_play.wake_id.as_str(),
            body_plan: self.body_play.plan_id.as_str(),
            body_play: self.body_play.active_play_id.as_str(),
            plan: self.plan.plan_id.as_str(),
            fragment: self.plan.fragments[0].fragment_id.as_str(),
            fragment_play: active.active_play_id.as_str(),
            assignment_seal: self.seal,
        }
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [directory, admission, keys] = args.as_slice() else {
        return Err("expected capture, display-source admission and private-key directory".into());
    };
    let key = key::open(keys, false)?;
    let before = document(directory.join("readiness.json"))?;
    let software = document(directory.join("software-offer.json"))?;
    let after = document(directory.join("readiness-after.json"))?;
    let actual: c::HostAdvertisement = serde_json::from_value(before["advertisement"].clone())?;
    let source = DisplayOutputSource::compile()?;
    let (host, grants) = source
        .observe_native_offers(
            &actual,
            &serde_json::to_vec(&software)?,
            &serde_json::from_value(after["advertisement"].clone())?,
        )
        .map_err(|e| format!("current display offers: {e:?}"))?;
    let bases: Vec<_> = core::iter::once("conduit.base/local@1".into())
        .chain(host.bases.iter().map(|base| base.implementation_id.clone()))
        .collect();
    let prepared = PreparedDisplayOutput::prepare(&source, &host, &bases, &grants)
        .map_err(|e| format!("display construction: {e:?}"))?;
    let plan = prepared.plan();
    let owner = body::propose_plan(plan, &actual, admission, directory)?;
    let body_play = BodyPlayIdentity::bind(&owner.realization().ok_or("Body proposal")?.plan, 1);
    let mut wire = Vec::new();
    fs::File::open(directory.join("uninstalled-output-assignment-transfer.bin"))?
        .take(8193)
        .read_to_end(&mut wire)?;
    if wire.is_empty() || wire.len() > 8192 {
        return Err("display transfer bound".into());
    }
    let seal = c::assigned_plan_payload_digest(&wire);
    let fragment = &plan.fragments[0];
    let first = c::bind_active_play(&plan.plan_id, &host.host_id, &host.boot_id, 1);
    let scope = Scope {
        plan,
        host: &host,
        body_play: &body_play,
        seal,
    };
    let inspect = exchange(&key, scope.request(control::Operation::Inspect, 1, &first))?;
    let sequence = inspect["next_play_sequence"]
        .as_u64()
        .ok_or("Root sequence exhausted")?;
    let active = c::bind_active_play(&plan.plan_id, &host.host_id, &host.boot_id, sequence);
    let binding = NativeBodyBinding::prepare_display(&prepared, &owner, &body_play, &active)
        .map_err(|e| format!("display Body custody: {e:?}"))?;
    let reserved = exchange(&key, scope.request(control::Operation::Reserve, 2, &active))?;
    let reservation = &reserved["reservation"];
    if reserved["control_applied"] != true
        || !reserved["refusal"].is_null()
        || reservation["active"] != serde_json::to_value(&active)?
        || reservation["fragment"] != fragment.fragment_id.as_str()
        || reservation["offer_generation"] != host.offer_generation.0
        || reservation["body"] != body_play.body_id.as_str()
        || reservation["wake"] != body_play.wake_id.as_str()
        || reservation["body_plan"] != body_play.plan_id.as_str()
        || reservation["body_play"] != body_play.active_play_id.as_str()
        || reservation["assignment_seal"] != serde_json::json!(seal)
    {
        return Err("display reservation correlation refused".into());
    }
    let prepared = exchange(&key, scope.request(control::Operation::Prepare, 3, &active))?;
    fs::write(
        directory.join("owner-native-preparation.json"),
        serde_json::to_vec_pretty(&prepared)?,
    )?;
    if prepared["control_applied"] != true
        || prepared["appliance_kernel_prepared"] != true
        || prepared["execution_context_prepared"] != true
        || prepared["capability_possession_issued"] != true
        || !prepared["refusal"].is_null()
    {
        return Err(format!("display preparation refused: {}", prepared["refusal"]).into());
    }
    let task = &prepared["display_worker"];
    if task["phase"] != "ready-held"
        || task["task_active"] != true
        || task["progress_current"] != true
        || task["steps"] != 0
        || task["native_calls"] != 0
        || task["software_calls"] != 0
        || task["native_custody"] != true
        || task["native_lifecycle"] != "issued"
        || task["native_in_flight"] != 0
        || task["context_bytes"].as_u64().unwrap_or(0) == 0
        || task["held_future_bytes"].as_u64().unwrap_or(0) == 0
    {
        return Err("actual display task readiness refused".into());
    }
    if binding.fragment_play(&owner).is_ok() || owner.realization().unwrap().play.is_some() {
        return Err("dormant preparation incorrectly started Body".into());
    }
    println!(
        "{}",
        serde_json::json!({
            "event":"owner-preparation-complete", "completed":true,
            "body_id":body_play.body_id,"plot_plan":plan.plan_id,"fragment_play":active,
            "owner_retained_through_preparation":true,"display_only":true,
            "worker_task_held":true,"worker_context_bytes":task["context_bytes"],
            "worker_held_future_bytes":task["held_future_bytes"],
            "body_started":false,"climate_sampling_claimed":false,"physical_display_delivery":false,
        })
    );
    Ok(())
}
