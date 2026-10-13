//! One bounded local client; transport refusal must not retire the retained owner.
use super::*;
use std::os::unix::net::UnixStream;

pub(super) fn serve_client(
    state_dir: &Path,
    stream: &mut UnixStream,
    runtime: &mut DurableHostRuntime,
    token: &[u8; 32],
) -> Result<(), String> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .map_err(|error| format!("bound local host control read: {error}"))?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(2)))
        .map_err(|error| format!("bound local host control write: {error}"))?;
    let mut first = [0_u8; 1];
    if let Err(error) = stream.read_exact(&mut first) {
        match error.kind() {
            std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::WouldBlock => {
                // No prefix means no request and no owner effect. Retire only this client.
                eprintln!(
                    "local control request refused: incomplete prefix ({})",
                    error.kind()
                );
                return Ok(());
            }
            _ => return Err(format!("read local host control prefix: {error}")),
        }
    }
    if first[0] == terminal_attach::MAGIC[0] {
        terminal_attach::serve(state_dir, stream, runtime, token, first[0])?;
    } else if first[0] == speech_route::MAGIC[0] {
        speech_route::serve(stream, runtime, token, first[0])?;
    } else {
        let mut request = std::io::Cursor::new(first).chain(&mut *stream);
        let response = match read_frame(&mut request) {
            Ok(request) => handle(request, token, runtime),
            Err(error) => {
                eprintln!("local control request refused: {error}");
                refused("invalid-local-control-frame")
            }
        };
        // A malformed client or oversized response must not terminate the
        // retained Owner. No request is replayed after a transport failure.
        if let Err(error) = write_frame(stream, &response) {
            eprintln!("local control response refused: {error}");
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "client_frame/tests.rs"]
mod tests;
