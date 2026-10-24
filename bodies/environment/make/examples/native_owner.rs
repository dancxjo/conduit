//! Attended physical preparation owner, retained through the complete exchange.
//! Default preparation stays held; explicit start-held/run modes require a
//! Boot-attributed acknowledgement before recording the ordinary Body start.
extern crate alloc;
#[path = "native_owner/acknowledgement.rs"]
mod acknowledgement;
#[path = "observed_offers/body.rs"]
mod body;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_owner_control.rs"]
mod control;
#[path = "native_owner/exchange.rs"]
mod exchange;
#[path = "native_owner/key.rs"]
mod key;
#[path = "native_owner/live.rs"]
mod live;
use conduit_body::BodyPlayIdentity;
use conduit_environment_make::{
    compile_appliance_source, NativeApplianceLimits, NativeBodyBinding, NativeSoftwareSourceRecipe,
    PreparedNativeAppliance,
};
use exchange::{document, exchange};
use std::{error::Error, fs, io::Read, path::PathBuf};
struct Scope<'a> {
    native: &'a PreparedNativeAppliance,
    host: &'a conduit_core::HostAdvertisement,
    body_play: &'a BodyPlayIdentity,
    seal: [u8; 32],
    cadence_ms: u64,
}
impl Scope<'_> {
    fn request<'a>(
        &'a self,
        operation: control::Operation,
        sequence: u64,
        active: &'a conduit_core::ActivePlayIdentity,
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
            plan: self.native.plan().plan_id.as_str(),
            fragment: self.native.plan().fragments[0].fragment_id.as_str(),
            fragment_play: active.active_play_id.as_str(),
            assignment_seal: self.seal,
        }
    }
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let live = args
        .get(3)
        .is_some_and(|mode| mode == std::path::Path::new("run"));
    let start_held = args
        .get(3)
        .is_some_and(|mode| mode == std::path::Path::new("start-held"));
    if args.len() > 4 || (args.len() == 4 && !start_held && !live) {
        return Err("unknown owner mode".into());
    }
    let [directory, admission, keys] = args.get(..3).ok_or("owner arguments")? else {
        return Err("expected capture, admission, private-key directory".into());
    };
    let key = key::open(keys, false)?;
    let replies = document(directory.join("replies.json"))?;
    let frames = replies.as_array().ok_or("Root replies")?;
    let [before, software, policy, after] = frames.as_slice() else {
        return Err("Root frame count".into());
    };
    let actual: conduit_core::HostAdvertisement =
        serde_json::from_value(before["advertisement"].clone())?;
    let source = compile_appliance_source()?;
    let recipe = NativeSoftwareSourceRecipe::prepare(&source)
        .map_err(|e| format!("Source recipe: {e:?}"))?;
    let host = recipe
        .observe_native_offers(
            &actual,
            &serde_json::to_vec(software)?,
            &serde_json::from_value(after["advertisement"].clone())?,
        )
        .map_err(|e| format!("current offers: {e:?}"))?;
    let grants = recipe
        .observe_appliance_policy(&host, &serde_json::to_vec(policy)?)
        .map_err(|e| format!("policy: {e:?}"))?;
    let native = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &grants,
        NativeApplianceLimits::resident_local(),
    )
    .map_err(|e| format!("native construction: {e:?}"))?;
    let mut owner = body::propose(&native, &actual, admission, directory)?;
    let admitted = acknowledgement::AdmittedBoot::load(admission, &actual)?;
    let body_play = BodyPlayIdentity::bind(&owner.realization().ok_or("Body proposal")?.plan, 1);
    let mut wire = Vec::new();
    fs::File::open(directory.join("uninstalled-assignment-transfer.bin"))?
        .take(8193)
        .read_to_end(&mut wire)?;
    if wire.is_empty() || wire.len() > 8192 {
        return Err("transfer bound".into());
    }
    let seal = conduit_core::assigned_plan_payload_digest(&wire);
    let fragment = &native.plan().fragments[0];
    let first = conduit_core::bind_active_play(&fragment.plan_id, &host.host_id, &host.boot_id, 1);
    let scope_request = Scope {
        native: &native,
        host: &host,
        body_play: &body_play,
        seal,
        cadence_ms: recipe.cadence().period_ms,
    };
    let inspect = exchange(
        &key,
        scope_request.request(control::Operation::Inspect, 1, &first),
    )?;
    let sequence = inspect["next_play_sequence"]
        .as_u64()
        .ok_or("Root sequence exhausted")?;
    let active =
        conduit_core::bind_active_play(&fragment.plan_id, &host.host_id, &host.boot_id, sequence);
    let mut binding = NativeBodyBinding::prepare(&native, &owner, &body_play, &active)
        .map_err(|e| format!("Body custody: {e:?}"))?;
    let reserved = exchange(
        &key,
        scope_request.request(control::Operation::Reserve, 2, &active),
    )?;
    let scope = &reserved["reservation"];
    if reserved["control_applied"] != true
        || !reserved["refusal"].is_null()
        || scope["offer_generation"] != host.offer_generation.0
        || scope["active"] != serde_json::to_value(&active)?
        || scope["fragment"] != serde_json::to_value(&fragment.fragment_id)?
        || scope["body"] != body_play.body_id.as_str()
        || scope["wake"] != body_play.wake_id.as_str()
        || scope["body_plan"] != body_play.plan_id.as_str()
        || scope["body_play"] != body_play.active_play_id.as_str()
        || scope["assignment_seal"] != serde_json::json!(seal)
    {
        return Err("Root reservation correlation refused".into());
    }
    let prepared = exchange(
        &key,
        scope_request.request(control::Operation::Prepare, 3, &active),
    )?;
    fs::write(
        directory.join("owner-native-preparation.json"),
        serde_json::to_vec_pretty(&prepared)?,
    )?;
    if prepared["control_applied"] != true
        || prepared["appliance_kernel_prepared"] != true
        || prepared["capability_possession_issued"] != true
        || !prepared["refusal"].is_null()
    {
        return Err(format!("native preparation refused: {}", prepared["refusal"]).into());
    }
    let task = &prepared["worker"];
    if prepared["execution_context_prepared"] != true
        || task["phase"] != "ready-held"
        || task["native_live"] != true
        || task["steps"] != 0
        || task["source_sequence"] != 0
        || task["publication_sequence"] != 0
        || task["publication_bytes"] != 0
        || task["native_lifecycles"] != serde_json::json!(["issued", "issued"])
        || task["native_in_flight"] != serde_json::json!([0, 0])
        || task["context_bytes"].as_u64().unwrap_or(0) == 0
        || task["held_future_bytes"].as_u64().unwrap_or(0) == 0
    {
        return Err("actual worker ownership/readiness refused".into());
    }
    if binding.fragment_play(&owner).is_ok() || owner.realization().unwrap().play.is_some() {
        return Err("preparation incorrectly started Body".into());
    }
    if start_held || live {
        let request = scope_request.request(control::Operation::Start, 4, &active);
        let started = exchange(
            &key,
            scope_request.request(control::Operation::Start, 4, &active),
        )?;
        admitted.transition(&request, &started)?;
        if started["control_applied"] != true
            || !started["refusal"].is_null()
            || started["worker"]["phase"] != "started-held"
            || started["worker"]["source_sequence"] != 0
            || started["worker"]["steps"] != 0
        {
            return Err("attributed held start refused".into());
        }
        let current = owner.realization().ok_or("Body proposal missing")?;
        let sign = |sequence| {
            conduit_core::bind_sign(
                &host.host_id,
                &host.boot_id,
                Some(&body_play.active_play_id),
                sequence,
            )
            .sign_id
        };
        let wake = current
            .wake
            .body_plan_ready(&current.plan, sign(0))
            .and_then(|wake| wake.body_play_started(&current.plan, &body_play, sign(1)))
            .map_err(|e| format!("admitted lifecycle: {e:?}"))?;
        owner
            .started(&host.host_id, &host.boot_id, body_play.clone(), wake)
            .map_err(|e| format!("Body start: {e:?}"))?;
        binding
            .confirm_started(&owner)
            .map_err(|e| format!("native custody: {e:?}"))?;
        if binding
            .fragment_play(&owner)
            .map_err(|e| format!("current custody: {e:?}"))?
            != &active
        {
            return Err("fragment Play correlation".into());
        }
        fs::write(
            directory.join("owner-start-held.json"),
            serde_json::to_vec_pretty(&started)?,
        )?;
        fs::write(
            directory.join("body-started-evidence.json"),
            serde_json::to_vec_pretty(owner.evidence())?,
        )?;
    }
    if live {
        return live::run(
            &mut owner,
            &mut binding,
            &scope_request,
            &active,
            &key,
            &admitted,
            directory,
        );
    }
    println!(
        "{}",
        serde_json::json!({"event":if start_held {"owner-start-held-complete"} else {"owner-preparation-complete"}, "completed":true,
        "body_id":body_play.body_id,"plot_plan":native.plan().plan_id,"fragment_play":active,
        "owner_retained_through_preparation":true,"worker_task_held":true,"worker_context_bytes":task["context_bytes"],"worker_held_future_bytes":task["held_future_bytes"],"body_started":start_held,"device_start_attributed":start_held,"climate_sampling_claimed":false})
    );
    Ok(())
}
