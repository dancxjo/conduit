//! Foreground owner of one installed Linux Host's retained Body.
mod controller;
#[cfg(test)]
pub(crate) use controller::published_todo_test_fixture;
mod image;
mod member_presentation;
mod membership_receipt_document;
pub(crate) use member_presentation::MemberPresentationBasis;
pub(crate) use membership_receipt_document::MembershipReceiptDocument;
mod native_observation;
mod retained_source;
mod state;
use conduit_body::ResidentPlot;
use conduit_plot::ActivationSyntax;
#[cfg(unix)]
pub(crate) use controller::run_service_window;
pub(crate) use controller::serve_member_presence;
#[cfg(test)]
pub(crate) use controller::serve_with_carrier;
#[cfg(test)]
pub(crate) use controller::serve_with_dispatch;
#[cfg(target_os = "linux")]
pub(crate) use controller::OwnedForeLineSelection;
pub(crate) use controller::{
    clock_interval_action, is_clock_control_intent, BrowserAdmittedSnapshot,
    BrowserCarrierLineEvidence, BrowserWindowAuthorization, ClockAction, DirectSpokenStart,
    LlmSpokenStart, NotebookWaitingWorker, Owner, RunWorker, ThermostatWorker, TodoWaitingWorker,
    CLOCK_RUN_MAXIMUM_MILLIS,
};
use retained_source::checked as checked_retained_source;
use serde::Deserialize;
use std::{
    io::{BufRead, Read, Write},
    path::Path,
};
const MAXIMUM_SOURCE: u64 = 256 * 1024;
const MAXIMUM_CONTROL_REQUEST: u64 = 4096;
const MAXIMUM_ADMISSION_REQUEST: u64 = super::MAXIMUM_BODY_ADMISSION_BYTES;

pub(super) fn recover_retained_state(root: &Path) -> Result<(), String> {
    state::recover(root)
}
/// Select an exact old biography under exclusive OS continuity ownership;
/// only the ordinary trusted restart transition may advance that basis before
/// this one local File policy is consumed. No network proposal selects it.
#[cfg(target_os = "linux")]
pub(crate) fn resume_selected_source_service(
    host: conduit_std_host::StdHost,
    root: &Path,
    expected: &conduit_body::BodyBiographyEvidence,
) -> Result<(Owner, conduit_body::BodyBiographyEvidence), String> {
    let retained = state::load(root)?.ok_or("selected File has no retained Body")?;
    if &retained != expected {
        return Err("selected File revision differs from exact retained Body basis".into());
    }
    let owner = resume_service(host, root)?;
    let resumed = owner.selected_source_current_basis();
    Ok((owner, resumed))
}

pub(crate) fn resume_service(
    host: conduit_std_host::StdHost,
    root: &Path,
) -> Result<Owner, String> {
    state::recover(root)?;
    let retained = state::load(root)?.ok_or("installed Host has no retained Body")?;
    let mut owner = Owner::resume(host, retained)?;
    if let Some(sources) = retained_source::workset(root)? {
        for source in &sources {
            owner.set_foreground_plot_name(&source.plot)?;
        }
    }
    owner.restore_execution(root)?;
    #[cfg(target_os = "linux")]
    if owner.recover_notebook_on_startup(root)? {
        return Ok(owner);
    }
    if owner.has_retained_verified_todo_read() {
        let selected = super::selected_todo_checkpoint(root)?
            .ok_or("Todo re-encounter has no selected checkpoint")?;
        owner.reencounter_committed_todo(root, &selected.root, &selected.content, 5_000)?;
    } else if owner.has_retained_failed_todo_read() {
        let selected = super::selected_todo_checkpoint(root)?
            .ok_or("Todo failed read has no selected checkpoint")?;
        owner.retry_retained_failed_todo_read(root, &selected.root, &selected.content, 5_000)?;
    } else {
        owner.persist(root)?;
    }
    Ok(owner)
}

/// Only a checked, exact Todo scan can request the scoped production offer.
/// Its initial Form comes from authored source, never from the display name.
fn scoped_todo_initial(
    checked: &conduit_plot::ExpandedAuthoringPlot,
) -> Result<Option<(conduit_todo_plot::TodoState, u16)>, String> {
    if checked.expanded.activations.is_empty() {
        return Ok(None);
    }
    if checked.expanded.name != "todo/main" || checked.expanded.activations.len() != 1 {
        return Err("installed Body supports no other activation source".into());
    }
    let activation = &checked.expanded.activations[0];
    if activation.selected_plot != "todo/transition" {
        return Err("installed Todo scan requires the exact transition child".into());
    }
    let ActivationSyntax::Scan { maximum_items, .. } = &activation.mode else {
        return Err("installed Todo activation is not scan".into());
    };
    let bytes = activation
        .initial_accumulator_bytes
        .as_deref()
        .ok_or("installed Todo scan has no checked initial Form")?;
    let initial = conduit_todo_plot::TodoState::decode_info(bytes)
        .map_err(|error| format!("installed Todo initial Form: {error:?}"))?;
    if initial
        .encode_info()
        .map_err(|error| format!("installed Todo initial Form: {error:?}"))?
        != bytes
    {
        return Err("installed Todo initial Form is not canonical".into());
    }
    Ok(Some((initial, *maximum_items)))
}
/// Recover only the exact checked Thermostat source's scoped initial Form.
/// Service boot must advertise this Back before the retained owner can plan it.
pub(super) fn retained_thermostat_scan(
    root: &Path,
) -> Result<Option<(conduit_thermostat_plot::ThermostatState, u16)>, String> {
    let Some(checked) = checked_retained_source(root)? else {
        return Ok(None);
    };
    if checked.expanded.name != "thermostat/main" {
        return Ok(None);
    }
    scoped_thermostat_initial(&checked)
}

/// Only a checked, exact Thermostat scan can request the scoped production offer.
/// Its initial Form comes from authored source, never from the display name.
fn scoped_thermostat_initial(
    checked: &conduit_plot::ExpandedAuthoringPlot,
) -> Result<Option<(conduit_thermostat_plot::ThermostatState, u16)>, String> {
    if checked.expanded.activations.is_empty() {
        return Ok(None);
    }
    if checked.expanded.name != "thermostat/main" || checked.expanded.activations.len() != 1 {
        return Err("installed Body supports no other activation source".into());
    }
    let activation = &checked.expanded.activations[0];
    if activation.selected_plot != "thermostat/transition" {
        return Err("installed Thermostat scan requires the exact transition child".into());
    }
    let ActivationSyntax::Scan { maximum_items, .. } = &activation.mode else {
        return Err("installed Thermostat activation is not scan".into());
    };
    if *maximum_items == 0 || *maximum_items > 256 {
        return Err("installed Thermostat scan admits 1..=256 controls".into());
    }
    let bytes = activation
        .initial_accumulator_bytes
        .as_deref()
        .ok_or("installed Thermostat scan has no checked initial Form")?;
    let initial = conduit_thermostat_plot::ThermostatState::decode_info(bytes)
        .map_err(|error| format!("installed Thermostat initial Form: {error:?}"))?;
    if initial
        .encode_info()
        .map_err(|error| format!("installed Thermostat initial Form: {error:?}"))?
        != bytes
    {
        return Err("installed Thermostat initial Form is not canonical".into());
    }
    Ok(Some((initial, *maximum_items)))
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Inspect,
    ExecutionHistory,
    Invite {
        ttl_seconds: u64,
    },
    AdmitBrowser {
        expected_host_id: String,
        new_host_verifying_key: Option<[u8; 32]>,
        maximum_millis: u64,
    },
    AdmitInvited {
        expected_host_id: String,
        request: Box<conduit_body::PortableSpawnAdmissionRequest>,
    },
    AdmitNativeObservation {
        expected_host_id: String,
        observation: Box<native_observation::NativeSerialSpawnObservation>,
    },
    Plan,
    Run {
        maximum_millis: u64,
    },
    Lull,
    Close,
}

pub(crate) fn run(source: &Path, directory: &Path, name: &str) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("foreground Body ownership currently requires Linux".into());
    }
    let root = directory
        .canonicalize()
        .map_err(|e| format!("installed Host state: {e}"))?;
    let _ownership = super::owner_lock::acquire(&root)?;
    let _body_ownership = super::owner_lock::body(&root)?;
    state::recover(&root)?;
    image::verify(&root)?;
    let source_bytes = super::bounded_read(source, MAXIMUM_SOURCE)?;
    let source_text = std::str::from_utf8(&source_bytes).map_err(|e| e.to_string())?;
    let canonical_source = crate::plot_source::parse(source_text)?;
    let checked = canonical_source.expand_entry_for_authoring()?;
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id.clone(),
        checked.expanded.checked_plot_id.clone(),
    );
    let retained = state::load(&root)?;
    let (mut status, runtime) = if checked.expanded.name == "thermostat/main" {
        let thermostat = scoped_thermostat_initial(&checked)?;
        super::prepare_runtime_with_scans(
            &root,
            None,
            thermostat
                .as_ref()
                .map(|(initial, maximum)| (initial, *maximum)),
        )?
    } else {
        let todo = scoped_todo_initial(&checked)?;
        super::prepare_runtime_with_todo(
            &root,
            todo.as_ref().map(|(initial, maximum)| (initial, *maximum)),
        )?
    };
    let result = (|| {
        let mut owner =
            controller::Owner::open(runtime.into_owner_host(), resident, retained, name)?;
        owner.set_resident_plot_name(&checked)?;
        owner.restore_execution(&root)?;
        owner.persist(&root)?;
        status.body_id =
            super::current_body_id(&super::read_installation(&root.join("installation.json"))?)
                .map(str::to_owned);
        super::write_json_atomic(&root.join("runtime.json"), &status)?;
        super::write_bytes_atomic(&root.join("body/source.conduit"), &source_bytes)?;
        // Subsequent Plan/Run requests independently recheck retained Source.
        // Retire this startup parse/catalog before another preparation begins.
        drop(checked);
        drop(canonical_source);
        drop(source_bytes);
        emit(&owner.truth())?;
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        loop {
            let mut bytes = Vec::new();
            let length = (&mut input)
                .take(MAXIMUM_ADMISSION_REQUEST + 1)
                .read_until(b'\n', &mut bytes)
                .map_err(|e| e.to_string())?;
            if length == 0 {
                owner.lull()?;
                owner.persist(&root)?;
                break;
            }
            if length as u64 > MAXIMUM_ADMISSION_REQUEST {
                owner.lull()?;
                owner.persist(&root)?;
                return Err("owner admission request exceeds 512 KiB".into());
            }
            let result = serde_json::from_slice::<Request>(&bytes)
                .map_err(|e| format!("invalid owner request: {e}"))
                .and_then(|request| {
                    if length as u64 > MAXIMUM_CONTROL_REQUEST
                        && !matches!(
                            request,
                            Request::AdmitInvited { .. } | Request::AdmitNativeObservation { .. }
                        )
                    {
                        return Err("owner control request exceeds 4096 bytes".into());
                    }
                    match request {
                        Request::Inspect => {}
                        Request::ExecutionHistory => {
                            emit(&state::execution_history(&root)?)?;
                        }
                        Request::Invite { ttl_seconds } => {
                            let invitation = owner.issue_invitation(&root, ttl_seconds, None)?;
                            emit(&serde_json::to_value(invitation).map_err(|e| e.to_string())?)?;
                        }
                        Request::AdmitBrowser {
                            expected_host_id,
                            new_host_verifying_key,
                            maximum_millis,
                        } => {
                            owner.admit_browser(
                                &root,
                                &expected_host_id,
                                new_host_verifying_key,
                                maximum_millis,
                            )?;
                        }
                        Request::AdmitInvited {
                            expected_host_id,
                            request,
                        } => {
                            let receipt =
                                owner.admit_invited(&root, *request, &expected_host_id)?;
                            emit(&serde_json::to_value(receipt).map_err(|e| e.to_string())?)?;
                        }
                        Request::AdmitNativeObservation {
                            expected_host_id,
                            observation,
                        } => {
                            let request = observation.into_request(&expected_host_id)?;
                            let receipt = owner.admit_invited(&root, request, &expected_host_id)?;
                            emit(&serde_json::to_value(receipt).map_err(|e| e.to_string())?)?;
                        }
                        Request::Plan => owner.plan_retained(&root)?,
                        Request::Run { maximum_millis } => {
                            owner.persist(&root)?;
                            state::require_execution_capacity(&root)?;
                            owner.execute_prepared(&root, maximum_millis)?;
                        }
                        Request::Lull => owner.lull()?,
                        Request::Close => {
                            owner.lull()?;
                            owner.persist(&root)?;
                            return Ok(true);
                        }
                    }
                    Ok(false)
                });
            owner.persist(&root)?;
            match result {
                Ok(close) => {
                    emit(&owner.truth())?;
                    if close {
                        break;
                    }
                }
                Err(error) => emit(
                    &serde_json::json!({"schema":"conduit.body/owner-refusal@1", "message":error, "truth":owner.truth()}),
                )?,
            }
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(root.join("runtime.json"));
    result
}
fn emit(value: &serde_json::Value) -> Result<(), String> {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, value).map_err(|e| e.to_string())?;
    writeln!(output)
        .and_then(|()| output.flush())
        .map_err(|e| e.to_string())
}

#[cfg(all(test, target_os = "linux"))]
mod cli_smoke;
#[cfg(all(test, target_os = "linux"))]
mod service_clock_smoke;

#[cfg(test)]
mod todo_speech_tests;

mod member_return_document;
pub(crate) use member_return_document::MemberReturnProofDocument;

#[cfg(test)]
pub(crate) use controller::member_return_owner_fixture;
