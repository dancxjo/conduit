//! Workspace lifecycle orchestration. The existing browser Body slot executes.
use conduit_body::{BodyBiographyEvidence, BodyPlayIdentity, ResidentForm, Wake};
use conduit_core::{BootId, HostId};
use conduit_workspace_model::WorkspaceBody;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
#[path = "workspace_refusal.rs"]
mod refusal;
use refusal::Refusal;

const CAPACITY: usize = 256 * 1024;
thread_local! {
    static INPUT: RefCell<Box<[u8]>> = RefCell::new(vec![0; CAPACITY].into_boxed_slice());
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(CAPACITY));
    static BODY: RefCell<Option<WorkspaceBody>> = const { RefCell::new(None) };
}

#[derive(Deserialize)]
#[serde(tag = "action", deny_unknown_fields)]
enum Request {
    Arrive,
    Restore {
        evidence: BodyBiographyEvidence,
        host_id: HostId,
        boot_id: BootId,
    },
    Current,
    SelectForm {
        form: ResidentForm,
    },
    LibraryView {
        source: String,
        query: String,
        revision: u32,
    },
    ChangeWorkset {
        host_id: HostId,
        boot_id: BootId,
        expected_revision: u64,
        form: ResidentForm,
        source: String,
        edit: WorksetEdit,
    },
    Propose {
        host_id: HostId,
        boot_id: BootId,
        source: String,
    },
    Started {
        host_id: HostId,
        boot_id: BootId,
        play: BodyPlayIdentity,
        wake_at_start: Wake,
    },
    Lull {
        host_id: HostId,
        boot_id: BootId,
        terminated_play: Option<BodyPlayIdentity>,
    },
}

#[derive(Deserialize)]
enum WorksetEdit {
    Install,
    Remove,
}

#[derive(Serialize)]
struct Snapshot<'a> {
    schema: &'static str,
    evidence: &'a BodyBiographyEvidence,
    realization: Option<&'a conduit_workspace_model::WorkspaceRealization>,
    foreground: Option<&'a ResidentForm>,
    foreground_flow: String,
}

#[no_mangle]
pub extern "C" fn conduit_workspace_input_ptr() -> usize {
    INPUT.with(|value| value.borrow_mut().as_mut_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_workspace_input_capacity() -> usize {
    CAPACITY
}
#[no_mangle]
pub extern "C" fn conduit_workspace_output_ptr() -> usize {
    OUTPUT.with(|value| value.borrow().as_ptr() as usize)
}
#[no_mangle]
pub extern "C" fn conduit_workspace_output_len() -> usize {
    OUTPUT.with(|value| value.borrow().len())
}

#[no_mangle]
pub extern "C" fn conduit_workspace_request(length: usize) -> i32 {
    OUTPUT.with(|output| output.borrow_mut().clear());
    if length == 0 || length > CAPACITY {
        return -1;
    }
    let request = INPUT.with(|input| {
        let mut input = input.borrow_mut();
        let request = serde_json::from_slice::<Request>(&input[..length]);
        input[..length].fill(0);
        request
    });
    let result = request
        .map_err(|error| Refusal::new("InvalidRequest", error.to_string()))
        .and_then(dispatch);
    match result {
        Ok(bytes) => {
            OUTPUT.with(|output| *output.borrow_mut() = bytes);
            0
        }
        Err(error) => {
            let refusal = serde_json::json!({ "schema": "conduit.workspace/refusal@1", "disposition": "refused", "code": error.code, "message": error.message });
            if let Ok(bytes) = serde_json::to_vec(&refusal) {
                OUTPUT.with(|output| *output.borrow_mut() = bytes);
            }
            -2
        }
    }
}

fn dispatch(request: Request) -> Result<Vec<u8>, Refusal> {
    BODY.with(|slot| {
        let mut slot = slot.borrow_mut();
        match request {
            Request::Arrive => {
                if slot.is_some() {
                    return Err("Workspace already has a Body".into());
                }
                let evidence = crate::creche::workspace_evidence()?;
                let body = WorkspaceBody::open(evidence).map_err(debug)?;
                let bytes = snapshot(&body)?;
                crate::creche::handoff_workspace();
                *slot = Some(body);
                return Ok(bytes);
            }
            Request::Restore {
                evidence,
                host_id,
                boot_id,
            } => {
                if slot.is_some() {
                    return Err("Workspace already has a Body".into());
                }
                let body =
                    WorkspaceBody::resume_here(evidence, &host_id, &boot_id).map_err(debug)?;
                let bytes = snapshot(&body)?;
                *slot = Some(body);
                return Ok(bytes);
            }
            _ => {}
        }
        let current = slot.as_ref().ok_or("Workspace has no Body")?;
        let mut candidate = current.clone();
        match request {
            Request::Current => return snapshot(current),
            Request::LibraryView {
                source,
                query,
                revision,
            } => {
                return crate::creche::workspace_library(&source)?
                    .presentation(current, revision, &query)
                    .map_err(|error| Refusal::new("LibraryPresentation", format!("{error:?}")))?
                    .lower()
                    .map_err(|error| Refusal::new("LibraryPresentation", format!("{error:?}")))?
                    .encode()
                    .map_err(|error| Refusal::new("LibraryPresentation", format!("{error:?}")))
                    .and_then(|bytes| {
                        if bytes.len() <= CAPACITY {
                            Ok(bytes)
                        } else {
                            Err(Refusal::new(
                                "OutputBound",
                                "Form library exceeds its presentation bound",
                            ))
                        }
                    });
            }
            Request::ChangeWorkset {
                host_id,
                boot_id,
                expected_revision,
                form,
                source,
                edit,
            } => {
                crate::form_runner::workspace::require_empty()?;
                match edit {
                    WorksetEdit::Install => {
                        crate::creche::require_workspace_form(&source, &form)?;
                        candidate
                            .admit_form(expected_revision, form.clone(), &host_id, &boot_id)
                            .map_err(debug)?;
                        candidate.select_form(&form).map_err(debug)?;
                    }
                    WorksetEdit::Remove => candidate
                        .remove_form(expected_revision, &form, &host_id, &boot_id)
                        .map_err(debug)?,
                }
            }
            Request::SelectForm { form } => candidate.select_form(&form).map_err(debug)?,
            Request::Propose {
                host_id,
                boot_id,
                source,
            } => {
                let forms = crate::creche::plan_workspace_forms(
                    current.evidence(),
                    &source,
                    &host_id,
                    &boot_id,
                )?;
                let realization = candidate
                    .propose(forms, &host_id, &boot_id)
                    .map_err(debug)?;
                let bytes = encode(&serde_json::json!({
                    "schema": "conduit.patchbay/body-execution-proposal@1",
                    "wake": realization.wake, "plan": realization.plan,
                }))?;
                *slot = Some(candidate);
                return Ok(bytes);
            }
            Request::Started {
                host_id,
                boot_id,
                play,
                wake_at_start,
            } => {
                crate::form_runner::workspace::require_started(&play)?;
                candidate
                    .started(&host_id, &boot_id, play, wake_at_start)
                    .map_err(debug)?;
                let bytes = snapshot(&candidate)?;
                *slot = Some(candidate);
                crate::form_runner::workspace::acknowledge_start();
                return Ok(bytes);
            }
            Request::Lull {
                host_id,
                boot_id,
                terminated_play,
            } => {
                crate::form_runner::workspace::require_empty()?;
                candidate
                    .lull(&host_id, &boot_id, terminated_play.as_ref())
                    .map_err(debug)?;
            }
            Request::Arrive | Request::Restore { .. } => {
                unreachable!("handled before current Body")
            }
        }
        let bytes = snapshot(&candidate)?;
        *slot = Some(candidate);
        Ok(bytes)
    })
}

fn snapshot(body: &WorkspaceBody) -> Result<Vec<u8>, Refusal> {
    encode(&Snapshot {
        schema: "conduit.workspace/body@1",
        evidence: body.evidence(),
        realization: body.realization(),
        foreground: body.foreground(),
        foreground_flow: body.foreground_flow(),
    })
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>, Refusal> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| Refusal::new("EncodingFailure", error.to_string()))?;
    if bytes.len() > CAPACITY {
        return Err(Refusal::new(
            "OutputBound",
            "Workspace output exceeds its admitted bound",
        ));
    }
    Ok(bytes)
}
fn debug(error: conduit_workspace_model::WorkspaceBodyError) -> Refusal {
    error.into()
}
