//! Service-owned finite Body Play control and its local client entrance.
use super::body::HostSource;
#[cfg(unix)]
use super::body::{call, token};
use super::DurableHostRuntime;
#[cfg(unix)]
use super::{Request, Response, PROTOCOL};
use conduit_presentation::{FaceInteraction, MaskShow};
use conduit_std_host::BodyLiveForeAdmission;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) enum OwnedRunWorker {
    General(crate::durable_host::owner::RunWorker),
    Todo(Box<crate::durable_host::owner::TodoWaitingWorker>),
}

impl OwnedRunWorker {
    fn progress(
        &mut self,
        owner: &mut crate::durable_host::owner::Owner,
        root: &Path,
    ) -> Result<bool, String> {
        match self {
            Self::General(worker) => worker.progress(owner, root),
            Self::Todo(worker) => Ok(worker.progress(owner, root)?.is_some()),
        }
    }

    fn request_lull(&self) -> Result<(), String> {
        match self {
            Self::General(worker) => worker.request_lull(),
            Self::Todo(worker) => worker.request_lull(),
        }
    }
}

impl DurableHostRuntime {
    /// A returned success means the exact checkpoint output and terminal Sign
    /// were retained. A slow or lost local reply is outcome-unknown, never an
    /// acknowledgement of mere Host queue staging.
    pub(super) fn submit_owned_todo_action(
        &mut self,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        let selected = crate::durable_host::selected_todo_checkpoint(match &self.host {
            HostSource::Body { root, .. } => root,
            _ => return Err("installed Todo does not own a Body".into()),
        })?
        .ok_or("installed Todo has no selected checkpoint residence")?;
        let HostSource::Body {
            owner,
            root,
            running,
        } = &mut self.host
        else {
            return Err("installed Todo has no current waiting Play".into());
        };
        let Some(OwnedRunWorker::Todo(worker)) = running else {
            return Err("installed Todo has no current waiting Play".into());
        };
        let admitted = worker.submit_interaction(owner, show, interaction)?;
        if admitted == BodyLiveForeAdmission::Full {
            return Err("Todo command Fore is full".into());
        }
        let deadline = Instant::now()
            + super::body::CONTROL_DEADLINE.saturating_sub(Duration::from_millis(250));
        loop {
            if let Some(committed) = worker.progress(owner, root)? {
                let receipt = owner
                    .todo_commit_receipt()
                    .ok_or("Todo commit receipt was not retained")?
                    .clone();
                *running = None;
                let restored = owner.read_committed_todo(
                    root,
                    &selected.root,
                    &selected.content,
                    &committed,
                    5_000,
                )?;
                if let Some(equipment) = &mut self.selected_speech_equipment {
                    equipment.advance_offer_generation(owner.host.current())?;
                }
                let read_receipt = owner
                    .todo_verified_read_receipt()
                    .ok_or("Todo read receipt was not retained")?
                    .clone();
                return Ok(serde_json::json!({
                    "schema":"conduit.todo/committed-action@1",
                    "interaction_id":interaction.identity,
                    "state_revision":restored.revision,
                    "receipt":receipt,
                    "read_receipt":read_receipt,
                }));
            }
            if Instant::now() >= deadline {
                return Err(super::CONTROL_OUTCOME_UNKNOWN.into());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn progress_owned_body(&mut self) -> Result<(), String> {
        if let HostSource::Body {
            owner,
            root,
            running: Some(worker),
        } = &mut self.host
        {
            if worker.progress(owner, root)? {
                if let HostSource::Body { running, .. } = &mut self.host {
                    *running = None;
                }
            }
        }
        Ok(())
    }

    pub(super) fn start_owned_body(
        &mut self,
        maximum_millis: u64,
        todo_new_list: Option<String>,
    ) -> Result<(), String> {
        let next_todo = match &self.host {
            HostSource::Body { owner, root, .. }
                if todo_new_list.is_none() && owner.has_verified_todo() =>
            {
                Some(crate::durable_host::next_selected_todo_checkpoint(root)?)
            }
            _ => None,
        };
        let selected = if todo_new_list.is_some() {
            Some(
                crate::durable_host::selected_todo_checkpoint(match &self.host {
                    HostSource::Body { root, .. } => root,
                    _ => return Err("installed Host does not own a Body".into()),
                })?
                .ok_or("installed Host has no selected Todo checkpoint residence")?,
            )
        } else {
            None
        };
        #[cfg(unix)]
        if super::terminal_attach::is_attached(self) {
            return Err("terminal-attachment-must-detach-before-start".into());
        }
        let HostSource::Body {
            owner,
            root,
            running,
        } = &mut self.host
        else {
            return Err("installed Host does not own a live Body session".into());
        };
        if running.is_some() {
            return Err("Body Play is already running".into());
        }
        *running = Some(match (todo_new_list, selected, next_todo) {
            (Some(list_key), Some(selected), None) => {
                OwnedRunWorker::Todo(Box::new(owner.start_selected_new_todo_list(
                    root,
                    selected.root,
                    &selected.content,
                    list_key,
                    maximum_millis,
                )?))
            }
            (None, None, Some(next)) => OwnedRunWorker::Todo(Box::new(
                owner.start_next_todo_action(root, &next, maximum_millis)?,
            )),
            (None, None, None) => {
                OwnedRunWorker::General(owner.start_service_run(root, maximum_millis)?)
            }
            _ => return Err("selected Todo list and Host residence differ".into()),
        });
        Ok(())
    }

    pub(super) fn request_owned_body_lull(&mut self) -> Result<Option<String>, String> {
        let HostSource::Body {
            owner,
            root,
            running,
        } = &mut self.host
        else {
            return Err("installed Host does not own a live Body session".into());
        };
        if let Some(worker) = running {
            let play = owner
                .current_play_id()
                .ok_or("Body Play has not started yet")?
                .as_str()
                .to_owned();
            worker.request_lull()?;
            return Ok(Some(play));
        }
        owner.lull()?;
        owner.persist(root)?;
        Ok(None)
    }
}

#[cfg(unix)]
pub(crate) fn start_owned_body(
    state_dir: &Path,
    maximum_millis: u64,
    todo_new_list: Option<String>,
) -> Result<(), String> {
    match call(
        state_dir,
        Request::BodyStart {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            maximum_millis,
            todo_new_list,
        },
    )? {
        Response::BodyRunRequested {
            protocol: PROTOCOL,
            maximum_millis,
        } => {
            println!(
                "{}",
                serde_json::json!({
                    "schema":"conduit.body/service-run-requested@1",
                    "maximum_millis":maximum_millis,
                    "play_state":"preparing",
                })
            );
            Ok(())
        }
        Response::Refused { code, .. } => Err(format!("Body owner refused start: {code}")),
        _ => Err("Body owner returned the wrong start response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn lull_owned_body(state_dir: &Path) -> Result<(), String> {
    match call(
        state_dir,
        Request::BodyLull {
            protocol: PROTOCOL,
            token: token(state_dir)?,
        },
    )? {
        Response::BodyLullRequested {
            protocol: PROTOCOL,
            active_play_id,
        } => {
            let play_state = if active_play_id.is_some() {
                "stop-requested"
            } else {
                "lulled"
            };
            println!(
                "{}",
                serde_json::json!({
                    "schema":"conduit.body/service-lull-requested@1",
                    "active_play_id":active_play_id,
                    "play_state":play_state,
                })
            );
            Ok(())
        }
        Response::Refused { code, .. } => Err(format!("Body owner refused lull: {code}")),
        _ => Err("Body owner returned the wrong lull response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn start_owned_body(
    _state_dir: &Path,
    _maximum_millis: u64,
    _todo_new_list: Option<String>,
) -> Result<(), String> {
    Err("service-owned Body Play requires local Unix control".into())
}

#[cfg(not(unix))]
pub(crate) fn lull_owned_body(_state_dir: &Path) -> Result<(), String> {
    Err("service-owned Body Play requires local Unix control".into())
}
