//! Foreground owner of one installed Linux Host's retained Body.
mod controller;
mod image;
mod native_observation;
mod state;
use conduit_body::ResidentPlot;
#[cfg(unix)]
pub(crate) use controller::run_service_window;
pub(crate) use controller::{
    clock_interval_action, BrowserAdmittedSnapshot, BrowserWindowAuthorization, ClockAction, Owner,
    RunWorker, CLOCK_RUN_MAXIMUM_MILLIS,
};
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
pub(crate) fn resume_service(
    host: conduit_std_host::StdHost,
    root: &Path,
) -> Result<Owner, String> {
    state::recover(root)?;
    let retained = state::load(root)?.ok_or("installed Host has no retained Body")?;
    let mut owner = Owner::resume(host, retained)?;
    if let Some(checked) = checked_retained_source(root)? {
        owner.set_resident_plot_name(&checked)?;
    }
    owner.restore_execution(root)?;
    owner.persist(root)?;
    Ok(owner)
}

/// Recheck the retained source through the product's ordinary checked Plot
/// entrance. An older or interrupted installation can lack these source bytes;
/// its Face then retains the exact resident identity with a generic name.
fn checked_retained_source(
    root: &Path,
) -> Result<Option<conduit_plot::ExpandedAuthoringPlot>, String> {
    let path = root.join("body/source.conduit");
    if !path.exists() {
        return Ok(None);
    }
    let bytes = super::bounded_read(&path, MAXIMUM_SOURCE)?;
    let source = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
    crate::plot_source::parse(source)?
        .expand_entry_for_authoring()
        .map(Some)
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Inspect,
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
    let checked = crate::plot_source::parse(source_text)?.expand_entry_for_authoring()?;
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id.clone(),
        checked.expanded.checked_plot_id.clone(),
    );
    let retained = state::load(&root)?;
    let (mut status, runtime) = super::prepare_runtime(&root)?;
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
                        Request::Plan => owner.plan(&checked)?,
                        Request::Run { maximum_millis } => {
                            owner.persist(&root)?;
                            owner.execute(maximum_millis)?;
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
