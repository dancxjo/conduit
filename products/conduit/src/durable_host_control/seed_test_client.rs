//! One bounded real authenticated control request for the installed Seed journey.
use super::*;

pub(crate) fn serve_one(
    root: &Path,
    mut runtime: DurableHostRuntime,
    ready: std::sync::mpsc::Sender<HostAdvertisement>,
) -> Result<(), String> {
    use std::os::unix::net::UnixListener;
    runtime.install_cancellation_signal(root);
    runtime.birth_root = Some(root.to_path_buf());
    let listener = UnixListener::bind(root.join("control.sock")).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let token = read_secret(&root.join("control.token"))?;
    ready
        .send(runtime.truth().advertisement)
        .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                return client_frame::serve_client(root, &mut stream, &mut runtime, &token)
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(5))
            }
            Err(e) => return Err(format!("bounded Seed test control accept: {e}")),
        }
    }
}
