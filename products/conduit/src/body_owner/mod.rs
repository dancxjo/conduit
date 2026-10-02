//! Foreground owner of one installed Linux Host's retained Body.
mod controller;
mod image;
mod state;
use conduit_body::ResidentPlot;
use serde::Deserialize;
use std::{
    io::{BufRead, Read, Write},
    path::Path,
};
const MAXIMUM_SOURCE: u64 = 256 * 1024;
const MAXIMUM_REQUEST: u64 = 4096;
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum Request {
    Inspect,
    AdmitBrowser {
        expected_host_id: String,
        new_host_verifying_key: Option<[u8; 32]>,
        maximum_millis: u64,
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
                .take(MAXIMUM_REQUEST + 1)
                .read_until(b'\n', &mut bytes)
                .map_err(|e| e.to_string())?;
            if length == 0 {
                owner.lull()?;
                owner.persist(&root)?;
                break;
            }
            if length as u64 > MAXIMUM_REQUEST {
                owner.lull()?;
                owner.persist(&root)?;
                return Err("owner request exceeds 4096 bytes".into());
            }
            let result = serde_json::from_slice::<Request>(&bytes)
                .map_err(|e| format!("invalid owner request: {e}"))
                .and_then(|request| {
                    match request {
                        Request::Inspect => {}
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
