use crate::{
    cli::{GlobalOpts, ProveArgs},
    process::StepError,
};
use conduit_core::{BaseImplementationId, BootId, HostId, OfferGeneration};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::hosted_calendar::{
    CalendarHostedOperation, GoogleBearerToken, GoogleCalendarClient, GoogleCalendarRefusal,
    GoogleCalendarResource, GoogleCalendarService, GoogleHttpsTransport, HostedCalendarAdapter,
};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env,
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const PROOF_ID: &str = "prove.calendar-google";
const MAXIMUM_CONFIG_BYTES: usize = 4_096;

#[derive(Deserialize)]
struct CalendarConfig {
    account_identity: String,
    calendar_id: String,
    time_min: String,
    time_max: String,
    time_zone: String,
}

#[derive(Default)]
struct ObservedResults {
    operations: Vec<CalendarHostedOperation>,
    result_hashes: Vec<String>,
}

struct RecordingAdapter<A> {
    inner: A,
    observed: Arc<Mutex<ObservedResults>>,
}

impl<A: HostedCalendarAdapter> HostedCalendarAdapter for RecordingAdapter<A> {
    fn execute(
        &mut self,
        operation: CalendarHostedOperation,
        semantic_json: &[u8],
        prior_realization_json: Option<&[u8]>,
    ) -> Result<Vec<u8>, GoogleCalendarRefusal> {
        let result = self
            .inner
            .execute(operation, semantic_json, prior_realization_json)?;
        let hash = format!("{:x}", Sha256::digest(&result));
        let mut observed = self
            .observed
            .lock()
            .map_err(|_| GoogleCalendarRefusal::ProviderLost)?;
        observed.operations.push(operation);
        observed.result_hashes.push(hash);
        Ok(result)
    }
}

#[derive(Serialize)]
struct LiveProof {
    schema_version: u16,
    proof_class: &'static str,
    operations: Vec<&'static str>,
    result_sha256: Vec<String>,
    plan_ids: Vec<String>,
    active_play_ids: Vec<String>,
    exact_account_resource_selected: bool,
    credential_redacted: bool,
    success: bool,
}

pub fn run(args: &ProveArgs, root: &Path, opts: &GlobalOpts) -> Result<(), StepError> {
    if opts.json || opts.quiet {
        return Err(StepError::prereq(
            PROOF_ID,
            "--json and --quiet are not supported by the interactive live calendar proof",
        ));
    }
    if opts.dry_run {
        println!("calendar-google: would execute read, free-busy, create, update, and cancel through ordinary Plan/Play");
        return Ok(());
    }

    let credential = named_environment(args.credential_env.as_deref(), "--credential-env")?;
    let config_json =
        named_environment(args.calendar_config_env.as_deref(), "--calendar-config-env")?;
    if config_json.len() > MAXIMUM_CONFIG_BYTES {
        return Err(StepError::prereq(
            PROOF_ID,
            "--calendar-config-env value exceeds the bounded configuration size",
        ));
    }
    let config: CalendarConfig = serde_json::from_str(&config_json).map_err(|_| {
        StepError::prereq(
            PROOF_ID,
            "--calendar-config-env value is not exact calendar proof configuration JSON",
        )
    })?;
    let reference_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StepError::prereq(PROOF_ID, "system clock precedes the Unix epoch"))?
        .as_secs();
    let event_identity = format!("conduit/calendar-proof/{reference_unix_seconds}");

    let client = GoogleCalendarClient::new(
        GoogleHttpsTransport::default(),
        GoogleBearerToken::new(credential.into_bytes())
            .map_err(|error| refusal("initialize credential", error))?,
        GoogleCalendarResource {
            account_identity: config.account_identity,
            calendar_id: config.calendar_id.clone(),
        },
    )
    .map_err(|error| refusal("initialize resource", error))?;
    let observed = Arc::new(Mutex::new(ObservedResults::default()));
    let adapter = RecordingAdapter {
        inner: GoogleCalendarService::new(client),
        observed: Arc::clone(&observed),
    };
    let mut host = StdHost::new_with_calendar(
        StdHostConfig {
            host_id: HostId::from("std/calendar-google-live"),
            boot_id: BootId::from(format!("calendar-google-live/{reference_unix_seconds}")),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        Box::new(adapter),
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error))?;

    let read = serde_json::json!({
        "time_min": config.time_min, "time_max": config.time_max, "maximum_results": 16
    });
    let free_busy = serde_json::json!({
        "time_min": config.time_min, "time_max": config.time_max,
        "reference_unix_seconds": reference_unix_seconds, "maximum_age_seconds": 300,
        "participants": [{"participant_identity": "proof/self", "contact_reference": config.calendar_id}]
    });
    let create = event_request(
        &event_identity,
        "Conduit calendar proof",
        &config.time_min,
        &config.time_max,
        &config.time_zone,
    );
    let update = event_request(
        &event_identity,
        "Conduit calendar proof updated",
        &config.time_min,
        &config.time_max,
        &config.time_zone,
    );
    let cancel =
        serde_json::json!({"event_identity": event_identity, "notify_participants": false});

    let forms = [
        single_form("read", conduit_semantic_catalog::CALENDAR_READ_KIND, &read),
        single_form(
            "free_busy",
            conduit_semantic_catalog::CALENDAR_FREE_BUSY_KIND,
            &free_busy,
        ),
        write_form(&create, &update, &cancel),
    ];
    let operations: [&[CalendarHostedOperation]; 3] = [
        &[CalendarHostedOperation::Read],
        &[CalendarHostedOperation::FreeBusy],
        &[
            CalendarHostedOperation::Create,
            CalendarHostedOperation::Update,
            CalendarHostedOperation::Cancel,
        ],
    ];
    let mut plan_ids = Vec::with_capacity(forms.len());
    let mut active_play_ids = Vec::with_capacity(forms.len());
    for (source, required_operations) in forms.iter().zip(operations) {
        let fragment = plan(&host, source, required_operations)?;
        plan_ids.push(fragment.plan_id.as_str().to_string());
        let mut operator_output = Vec::with_capacity(8_192);
        let report = host
            .run_fragment_to(fragment, &mut operator_output, &mut ThreadTimer)
            .map_err(|error| StepError::prereq(PROOF_ID, error))?;
        let kernel = report
            .kernel
            .ok_or_else(|| StepError::prereq(PROOF_ID, "calendar Play emitted no kernel proof"))?;
        active_play_ids.push(kernel.active_play_id.as_str().to_string());
    }
    drop(host);

    let observed = Arc::try_unwrap(observed)
        .map_err(|_| StepError::prereq(PROOF_ID, "calendar observation still has owners"))?
        .into_inner()
        .map_err(|_| StepError::prereq(PROOF_ID, "calendar observation lock was poisoned"))?;
    let expected = [
        CalendarHostedOperation::Read,
        CalendarHostedOperation::FreeBusy,
        CalendarHostedOperation::Create,
        CalendarHostedOperation::Update,
        CalendarHostedOperation::Cancel,
    ];
    if observed.operations != expected {
        return Err(StepError::prereq(
            PROOF_ID,
            "live calendar operations did not complete in the exact admitted sequence",
        ));
    }
    let proof = LiveProof {
        schema_version: 1,
        proof_class: "live-transport",
        operations: expected
            .iter()
            .map(|operation| operation.contract())
            .collect(),
        result_sha256: observed.result_hashes,
        plan_ids,
        active_play_ids,
        exact_account_resource_selected: true,
        credential_redacted: true,
        success: true,
    };
    let path = root.join("target/calendar-google-live.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&proof)
            .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?,
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    println!(
        "calendar-google: live Plan/Play proof complete ({})",
        path.display()
    );
    Ok(())
}

fn plan(
    host: &StdHost,
    source: &str,
    operations: &[CalendarHostedOperation],
) -> Result<conduit_core::PlanFragment, StepError> {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_semantic_catalog::install_calendar_provider_catalogs(&mut startup, &mut profiles)
        .map_err(|error| StepError::prereq(PROOF_ID, error))?;
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &startup)
        .map_err(|error| StepError::prereq(PROOF_ID, error.message))?;
    let expanded = expand_canonical_form(&checked, "proof", &profiles)
        .map_err(|error| StepError::prereq(PROOF_ID, error.message))?;
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    let mut grants = Vec::new();
    for operation in operations {
        grants.extend(
            host.calendar_authority_grants(*operation, operation.contract())
                .map_err(|error| StepError::prereq(PROOF_ID, error))?,
        );
    }
    let connection_bases = Default::default();
    let line_candidates = Default::default();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::CALENDAR_MAXIMUM_RESULT_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| StepError::prereq(PROOF_ID, error.to_string()))?;
    plan.fragments
        .into_iter()
        .next()
        .ok_or_else(|| StepError::prereq(PROOF_ID, "calendar Plan has no local fragment"))
}

fn single_form(name: &str, kind: &str, request: &serde_json::Value) -> String {
    format!(
        "form proof {{\n {name}: {kind}(request = {{semantic_json: {}}})\n}}\n",
        serde_json::to_string(&request.to_string()).expect("JSON string encoding cannot fail")
    )
}

fn write_form(
    create: &serde_json::Value,
    update: &serde_json::Value,
    cancel: &serde_json::Value,
) -> String {
    format!(
        "form proof {{\n create: calendar/create-event(request = {{semantic_json: {}}})\n update: calendar/update-event(request = {{semantic_json: {}}})\n cancel: calendar/cancel-event(request = {{semantic_json: {}}})\n create.receipt > update.prior\n update.receipt > cancel.prior\n}}\n",
        serde_json::to_string(&create.to_string()).unwrap(),
        serde_json::to_string(&update.to_string()).unwrap(),
        serde_json::to_string(&cancel.to_string()).unwrap(),
    )
}

fn event_request(
    identity: &str,
    title: &str,
    start: &str,
    end: &str,
    time_zone: &str,
) -> serde_json::Value {
    serde_json::json!({"event": {
        "identity": identity, "title": title, "description": "temporary live acceptance proof",
        "location": "", "time": {"kind": "timed", "start": start, "end": end, "time_zone": time_zone},
        "recurrence": []
    }})
}

fn named_environment(name: Option<&str>, flag: &str) -> Result<String, StepError> {
    let name = required(name, flag)?;
    env::var(name).map_err(|_| {
        StepError::prereq(
            PROOF_ID,
            format!("{flag} variable is absent or non-Unicode"),
        )
    })
}

fn required<'a>(value: Option<&'a str>, flag: &str) -> Result<&'a str, StepError> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| StepError::prereq(PROOF_ID, format!("{flag} is required")))
}

fn refusal(context: &str, refusal: GoogleCalendarRefusal) -> StepError {
    StepError::prereq(PROOF_ID, format!("{context}: {refusal:?}"))
}
