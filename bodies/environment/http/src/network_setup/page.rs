//! The HTTP Look and response lifetime for a private network request. No radio
//! or scheduler lives here; Root installs the route only with its admitted service.
use super::{
    NetworkSetupInput, NetworkSetupLimits, NetworkSetupMailbox, NetworkSetupRefusal, Submission,
};
use core::fmt::Write;

pub fn network_setup_page(
    limits: NetworkSetupLimits,
) -> Result<heapless::String<2048>, NetworkSetupRefusal> {
    limits.validate()?;
    let mut body = heapless::String::<1536>::new();
    write!(body, "<!doctype html><html lang=en><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>Conduit Wi-Fi</title><style>body{{font:20px system-ui;max-width:34rem;margin:2rem auto;padding:1rem}}label,input,button{{display:block;margin:1rem 0}}input,button{{font:inherit;max-width:100%}}</style><h1>Connect to Wi-Fi</h1><p>Enter the network name and password. After you connect, this device may leave its own Wi-Fi network. If joining fails, reconnect to the device's Wi-Fi.</p><form method=post action=/network enctype=application/x-www-form-urlencoded><label>Network name<input name=ssid required autocomplete=off></label><label>Password<input type=password name=password required minlength={} maxlength={} autocomplete=off></label><button>Connect</button></form><p>Password-protected networks only.</p><a href=/>Back to readings</a></html>", limits.minimum_credential_bytes, limits.maximum_credential_bytes).map_err(|_| NetworkSetupRefusal::TooLarge)?;
    let mut response = heapless::String::<2048>::new();
    write!(response, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}", body.len(), body).map_err(|_| NetworkSetupRefusal::TooLarge)?;
    Ok(response)
}

/// Borrows the mailbox until the response finishes, preventing early consumption.
/// Dropping this guard after a failed write, timeout or cancellation cancels and
/// erases the pending request. The response never reflects submitted values.
pub struct PendingNetworkResponse<'a> {
    mailbox: &'a mut NetworkSetupMailbox,
    submission: Submission,
    pending: bool,
}
impl<'a> PendingNetworkResponse<'a> {
    pub fn prepare(
        mailbox: &'a mut NetworkSetupMailbox,
        input: NetworkSetupInput,
    ) -> Result<Self, NetworkSetupRefusal> {
        let submission = mailbox.submit(input)?;
        Ok(Self {
            mailbox,
            submission,
            pending: true,
        })
    }
    pub fn response(&self) -> heapless::String<768> {
        // Accepted is not a claim of station readiness.
        const BODY: &str = "<!doctype html><html lang=en><meta charset=utf-8><title>Connection requested</title><h1>Connection requested</h1><p>This page may disconnect while the device joins your network. If joining fails, reconnect to the device's Wi-Fi.</p></html>";
        let mut response = heapless::String::new();
        write!(response, "HTTP/1.1 202 Accepted\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}", BODY.len(), BODY).expect("fixed response fits");
        response
    }
    /// Call only after the complete response was written and transport flush
    /// succeeded. This releases input to the separately admitted credentials call.
    pub fn response_flushed(mut self) -> Result<(), NetworkSetupRefusal> {
        self.mailbox.acknowledge(self.submission)?;
        self.pending = false;
        Ok(())
    }
}
impl Drop for PendingNetworkResponse<'_> {
    fn drop(&mut self) {
        if self.pending {
            let _ = self.mailbox.cancel(self.submission);
        }
    }
}
