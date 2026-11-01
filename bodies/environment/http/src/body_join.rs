//! Public proof transport only. No invitation bearer secret belongs in these
//! routes; only Root's canonical admission pump may produce a signed receipt.
use alloc::vec::Vec;
use core::fmt::Write;
pub enum Action {
    Submit(Vec<u8>),
    Result(u32),
}
pub enum Reply {
    Queued(u32),
    Receipt(Vec<u8>),
    Refused(u32, &'static str),
    Unknown,
    Unavailable,
}
pub type Provider = fn(Action) -> Reply;
static PROVIDER: critical_section::Mutex<core::cell::RefCell<Option<Provider>>> =
    critical_section::Mutex::new(core::cell::RefCell::new(None));
/// Trusted composition installs one data callback. Installing it confers no
/// membership; canonical verification and durable custody remain in Root.
pub fn install_provider(provider: Provider) -> Result<(), &'static str> {
    critical_section::with(|cs| {
        let mut current = PROVIDER.borrow(cs).borrow_mut();
        if current.is_some() {
            return Err("body-join-provider-already-installed");
        }
        *current = Some(provider);
        Ok(())
    })
}
pub fn available() -> bool {
    critical_section::with(|cs| PROVIDER.borrow(cs).borrow().is_some())
}
pub fn dispatch(action: Action) -> Reply {
    let provider = critical_section::with(|cs| *PROVIDER.borrow(cs).borrow());
    provider.map_or(Reply::Unavailable, |provider| provider(action))
}
pub const PAGE: &str = r#"<!doctype html><meta charset="utf-8"><title>Join this Body</title><h1>Join this Body</h1><p>The thermometer keeps measuring while another host joins.</p><ol><li>Use the provisioned owner's private USB invitation command.</li><li>Open that private invitation in your local Conduit browser or Pi joining helper.</li><li>The helper submits its signed proof here and saves the Root-signed membership receipt.</li></ol><p>This public page contains no invitation secret. Private invitations stay in the owner's USB capture and the joining host's private state. Local history is available after admission through the joined recorder command.</p>"#;
/// Render into the service owner's existing bounded response backing.
pub fn render(reply: Reply, response: &mut heapless::String<2048>) -> bool {
    response.clear();
    let (status, body) = match reply {
        Reply::Queued(ticket) => {
            return write!(response,"HTTP/1.1 202 Accepted\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{{\"schema\":\"conduit.environment/body-join-queued@1\",\"ticket\":{ticket}}}").is_ok();
        }
        Reply::Refused(ticket, reason) => {
            let reason = bounded_refusal(reason);
            return write!(response,"HTTP/1.1 403 Forbidden\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{{\"schema\":\"conduit.environment/body-join-refused@1\",\"ticket\":{ticket},\"reason\":\"{reason}\"}}").is_ok();
        }
        Reply::Receipt(bytes) => {
            let Ok(body) = core::str::from_utf8(&bytes) else {
                return false;
            };
            return write!(response,"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).is_ok();
        }
        Reply::Unknown => ("404 Not Found", ""),
        Reply::Unavailable => ("503 Service Unavailable", ""),
    };
    write!(response,"HTTP/1.1 {status}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).is_ok()
}
pub fn page(response: &mut heapless::String<2048>) -> bool {
    response.clear();
    write!(response,"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Security-Policy: default-src 'none'\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",PAGE.len(),PAGE).is_ok()
}
pub fn preflight(response: &mut heapless::String<2048>) -> bool {
    response.clear();
    response.push_str("HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").is_ok()
}

/// Static Root codes only; arbitrary text cannot enter JSON or grow the frame.
fn bounded_refusal(reason: &'static str) -> &'static str {
    if !reason.is_empty()
        && reason.len() <= 64
        && reason
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        reason
    } else {
        "body-join-refused"
    }
}

#[cfg(test)]
mod refusal_tests {
    use super::*;
    #[test]
    fn refusal_keeps_ticket_and_bounded_root_reason() {
        let mut response = heapless::String::new();
        assert!(render(
            Reply::Refused(17, "body-return-observer-profile"),
            &mut response
        ));
        assert!(response.starts_with("HTTP/1.1 403 Forbidden\r\n"));
        let (_, body) = response.split_once("\r\n\r\n").unwrap();
        let body: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["ticket"], 17);
        assert_eq!(body["reason"], "body-return-observer-profile");
    }
    #[test]
    fn arbitrary_text_cannot_escape_refusal_code() {
        for reason in [
            "untrusted\"text",
            "",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            let mut response = heapless::String::new();
            assert!(render(Reply::Refused(1, reason), &mut response));
            let (_, body) = response.split_once("\r\n\r\n").unwrap();
            let body: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(body["reason"], "body-join-refused");
        }
    }
}
