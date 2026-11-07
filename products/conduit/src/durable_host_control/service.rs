//! Main-thread progress for installed Host service and retained physical equipment.
use super::*;

pub(crate) type ForegroundPoll<'a> = dyn FnMut(&mut DurableHostRuntime) -> Result<(), String> + 'a;

pub(crate) fn serve(state_dir: &Path, runtime: DurableHostRuntime) -> Result<(), String> {
    serve_inner(state_dir, runtime, None)
}

#[cfg(any(test, feature = "native-display"))]
pub(crate) fn serve_with_foreground(
    state_dir: &Path,
    runtime: DurableHostRuntime,
    foreground: &mut ForegroundPoll<'_>,
) -> Result<(), String> {
    serve_inner(state_dir, runtime, Some(foreground))
}

fn serve_inner(
    state_dir: &Path,
    mut runtime: DurableHostRuntime,
    mut foreground: Option<&mut ForegroundPoll<'_>>,
) -> Result<(), String> {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};

    runtime.install_cancellation_signal(state_dir);
    runtime.birth_root = Some(state_dir.to_path_buf());
    let socket = state_dir.join("control.sock");
    if socket.exists() {
        fs::remove_file(&socket)
            .map_err(|error| format!("remove stale control endpoint: {error}"))?;
    }
    let listener = UnixListener::bind(&socket)
        .map_err(|error| format!("bind durable host control endpoint: {error}"))?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("restrict durable host control endpoint: {error}"))?;
    let token = read_secret(&state_dir.join("control.token"))?;
    let mut polling_play = false;
    loop {
        terminal_attach::retire_closed_attachment(state_dir, &mut runtime)?;
        if let Some(poll) = foreground.as_deref_mut() {
            poll(&mut runtime)?;
        }
        let running = foreground.is_some()
            || runtime.host.body_is_running()
            || runtime.speech_worker.is_some()
            || runtime.owner_spoken_worker.is_some()
            || terminal_attach::is_attached(&mut runtime);
        if running != polling_play {
            listener
                .set_nonblocking(running)
                .map_err(|error| format!("set Body owner control responsiveness: {error}"))?;
            polling_play = running;
        }
        runtime.progress_owned_body()?;
        runtime.progress_browser_speech()?;
        runtime.progress_owner_spoken()?;
        let mut stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                let pause = if foreground.is_some()
                    || runtime.host.body_is_running()
                    || runtime.speech_worker.is_some()
                    || runtime.owner_spoken_worker.is_some()
                {
                    10
                } else {
                    100
                };
                std::thread::sleep(std::time::Duration::from_millis(pause));
                continue;
            }
            Err(error) => return Err(format!("accept local host control: {error}")),
        };
        client_frame::serve_client(state_dir, &mut stream, &mut runtime, &token)?;
    }
}

#[cfg(test)]
#[path = "service/tests.rs"]
mod tests;
