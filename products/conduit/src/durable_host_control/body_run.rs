//! Service-owned finite Body Play control and its local client entrance.
use super::body::HostSource;
#[cfg(unix)]
use super::body::{call, token};
use super::DurableHostRuntime;
#[cfg(unix)]
use super::{Request, Response, PROTOCOL};
use std::path::Path;

impl DurableHostRuntime {
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

    pub(super) fn start_owned_body(&mut self, maximum_millis: u64) -> Result<(), String> {
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
        *running = Some(owner.start_service_run(root, maximum_millis)?);
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
pub(crate) fn start_owned_body(state_dir: &Path, maximum_millis: u64) -> Result<(), String> {
    match call(
        state_dir,
        Request::BodyStart {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            maximum_millis,
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
pub(crate) fn start_owned_body(_state_dir: &Path, _maximum_millis: u64) -> Result<(), String> {
    Err("service-owned Body Play requires local Unix control".into())
}

#[cfg(not(unix))]
pub(crate) fn lull_owned_body(_state_dir: &Path) -> Result<(), String> {
    Err("service-owned Body Play requires local Unix control".into())
}
