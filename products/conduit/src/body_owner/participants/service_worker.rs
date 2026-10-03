//! Bounded loopback network wait outside the authoritative service actor.
use super::{
    admission::remaining, nonce, serve_presence, service::BrowserWindowAuthorization,
    transport::Listener,
};
use crate::durable_host_control::browser;
use conduit_core::LinkBindingId;
use conduit_std_host::browser_admission::{
    BrowserAdmissionEgress as Out, BROWSER_ADMISSION_PROTOCOL as PROTOCOL,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

const MAX_CONNECTIONS: usize = 8;

pub(crate) fn run_service_window(
    state_dir: &Path,
    authorization: BrowserWindowAuthorization,
) -> Result<String, String> {
    let listener = Listener::bind()?;
    let url = listener.url()?;
    let directory = state_dir.to_path_buf();
    std::thread::Builder::new()
        .name("conduit-browser-admission".into())
        .spawn(move || {
            let window_id = authorization.window_id.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                serve(listener, &directory, authorization)
            }));
            let cleanup = browser::cancel(&directory, &window_id);
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => eprintln!("browser admission window ended: {error}"),
                Err(_) => eprintln!("browser admission worker panicked"),
            }
            if let Err(error) = cleanup {
                eprintln!("browser admission window cleanup failed: {error}");
            }
        })
        .map_err(|error| format!("start browser admission worker: {error}"))?;
    Ok(url)
}

fn serve(
    listener: Listener,
    state_dir: &Path,
    authorization: BrowserWindowAuthorization,
) -> Result<(), String> {
    let clock = Instant::now();
    let deadline = clock + Duration::from_millis(authorization.maximum_millis);
    for _ in 0..MAX_CONNECTIONS {
        let Some(left) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        let Some(mut socket) = listener.accept(left)? else {
            break;
        };
        let binding = LinkBindingId::from(format!(
            "line/service-browser/{}",
            nonce()?
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ));
        let window_id = &authorization.window_id;
        let mut active = None;
        let result = (|| {
            let (advertisement, encoded_bytes) =
                socket.receive(remaining(deadline)?.min(Duration::from_secs(2)))?;
            let challenge = browser::begin(
                state_dir,
                window_id,
                binding.clone(),
                advertisement,
                encoded_bytes,
            )?;
            socket.send(&challenge)?;
            let (proof, _) = socket.receive(remaining(deadline)?.min(Duration::from_secs(2)))?;
            let snapshot = browser::complete(state_dir, window_id, proof)?;
            active = Some(snapshot.credential.clone());
            serve_presence(
                &snapshot,
                &mut socket,
                &binding,
                clock,
                deadline,
                Some(state_dir),
            )
        })();
        if let Some(credential) = active {
            match browser::leave(state_dir, window_id, credential) {
                Ok(biography) => {
                    let _ = socket.send(&Out::BiographyEvidence {
                        protocol: PROTOCOL,
                        evidence: Box::new(biography),
                    });
                }
                Err(error) => eprintln!("browser presence leave could not be retained: {error}"),
            }
        } else if let Err(error) = browser::abort(state_dir, window_id) {
            eprintln!("browser challenge could not be cancelled: {error}");
        }
        socket.close();
        if let Err(error) = result {
            eprintln!("browser admission carrier ended: {error}");
        }
    }
    Ok(())
}
