//! Loopback browser Mask adapter; actions are routed into authored kernel work.
use crate::execution::{Execution, ResultState};
use conduit_presentation::{PresentationActionAvailability, PresentationFragment};
use conduit_thermostat_plot::{Command, Mode, ThermostatState};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

#[derive(Debug)]
struct ActionRefusal {
    code: &'static str,
    detail: String,
}
impl ActionRefusal {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}
impl From<String> for ActionRefusal {
    fn from(detail: String) -> Self {
        Self::new("thermostat-execution-refused", detail)
    }
}
impl From<&str> for ActionRefusal {
    fn from(detail: &str) -> Self {
        Self::new("thermostat-execution-refused", detail)
    }
}

struct Encounter {
    execution: Execution,
    current: ResultState,
    revision: u64,
}
impl Encounter {
    fn new() -> Result<Self, String> {
        let mut execution = Execution::new()?;
        let current = execution.execute(ThermostatState::default(), Command::SetMode(Mode::Off))?;
        Ok(Self {
            execution,
            current,
            revision: 0,
        })
    }
    fn fragment(&self) -> Result<PresentationFragment, String> {
        conduit_thermostat_face::fragment(&self.current.state, self.current.basis.clone(), true)
            .map_err(str::to_string)
    }
    fn face(&self) -> Result<Value, String> {
        let fragment = self.fragment()?;
        Ok(
            json!({ "revision": self.revision, "basis": { "checked_plot_id": fragment.basis.checked_plot_id, "plan_id": fragment.basis.plan_id, "active_play_id": fragment.basis.active_play_id }, "subjects": fragment.subjects, "relationships": fragment.relationships, "properties": fragment.properties, "text": fragment.text, "actions": fragment.actions, "disclosures": fragment.disclosures }),
        )
    }
    fn action(&mut self, request: Value) -> Result<Value, ActionRefusal> {
        if request["revision"].as_u64() != Some(self.revision) {
            return Err(ActionRefusal::new(
                "stale-face",
                "These controls are out of date. Refresh and try again.",
            ));
        }
        let id = request["action_id"]
            .as_str()
            .ok_or_else(|| ActionRefusal::new("invalid-action", "missing action identity"))?;
        let fragment = self.fragment()?;
        let action = fragment
            .actions
            .iter()
            .find(|action| action.identity == id)
            .ok_or_else(|| ActionRefusal::new("unknown-action", "unknown thermostat action"))?;
        if action.availability != PresentationActionAvailability::Available {
            return Err(ActionRefusal::new(
                "action-unavailable",
                "This thermostat control is unavailable.",
            ));
        }
        let command = conduit_thermostat_face::command_for_action(&self.current.state, id)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Face revision exhausted")?;
        let next = self.execution.execute(self.current.state, command)?;
        self.current = next;
        self.revision = revision;
        self.face().map_err(ActionRefusal::from)
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let port = match args.next().as_deref() {
        None => 8765,
        Some("--port") => args.next().ok_or("--port needs a number")?.parse::<u16>()?,
        _ => return Err("expected --port PORT".into()),
    };
    if args.next().is_some() {
        return Err("unexpected thermostat argument".into());
    }
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let address = listener.local_addr()?;
    let mut encounter = Encounter::new()?;
    println!("Thermostat: http://{address}\nSession settings run through the Conduit kernel. Sensor and equipment are unconnected. Ctrl-C to stop.");
    for stream in listener.incoming() {
        let mut stream = stream?;
        if let Err(error) = serve(&mut stream, &mut encounter, &address.to_string()) {
            let _ = respond(
                &mut stream,
                400,
                "application/json",
                &serde_json::to_vec(&json!({"code":"invalid-request","error": error}))?,
            );
        }
    }
    Ok(())
}
fn serve(stream: &mut TcpStream, encounter: &mut Encounter, address: &str) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let mut input = Vec::with_capacity(4096);
    let mut block = [0u8; 1024];
    let header_end = loop {
        let count = stream.read(&mut block).map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("incomplete request".into());
        }
        input.extend_from_slice(&block[..count]);
        if input.len() > 4096 {
            return Err("request exceeds admitted byte bound".into());
        }
        if let Some(position) = input.windows(4).position(|w| w == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let headers = core::str::from_utf8(&input[..header_end])
        .map_err(|e| e.to_string())?
        .to_owned();
    let mut lines = headers.lines();
    let mut request_line = lines.next().ok_or("missing request")?.split_whitespace();
    let method = request_line.next().ok_or("missing method")?;
    let path = request_line.next().ok_or("missing path")?;
    let headers: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_ascii_lowercase(), value.trim()))
        .collect();
    // Reject cross-origin action delivery and DNS rebinding; no external authority.
    if !headers
        .iter()
        .any(|(key, value)| key == "host" && *value == address)
    {
        return Err("unexpected Host".into());
    }
    if headers
        .iter()
        .any(|(key, value)| key == "origin" && *value != format!("http://{address}"))
    {
        return Err("unexpected Origin".into());
    }
    let length = headers
        .iter()
        .find(|(key, _)| key == "content-length")
        .map(|(_, value)| value.parse::<usize>())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(0);
    if length > 1024 || header_end + length > 4096 {
        return Err("action exceeds admitted byte bound".into());
    }
    while input.len() < header_end + length {
        let count = stream.read(&mut block).map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("incomplete action".into());
        }
        input.extend_from_slice(&block[..count]);
        if input.len() > 4096 {
            return Err("request exceeds admitted byte bound".into());
        }
    }
    let (status, mime, body) = match (method, path) {
        ("GET", "/") => (
            200,
            "text/html; charset=utf-8",
            include_bytes!("../web/index.html").to_vec(),
        ),
        ("GET", "/app.mjs") => (
            200,
            "text/javascript; charset=utf-8",
            include_bytes!("../web/app.mjs").to_vec(),
        ),
        ("GET", "/style.css") => (
            200,
            "text/css; charset=utf-8",
            include_bytes!("../web/style.css").to_vec(),
        ),
        ("GET", "/face") => (
            200,
            "application/json",
            serde_json::to_vec(&encounter.face()?).map_err(|e| e.to_string())?,
        ),
        ("POST", "/action") => {
            if !headers
                .iter()
                .any(|(key, value)| key == "content-type" && *value == "application/json")
            {
                return Err("actions require application/json".into());
            }
            let request: Value = serde_json::from_slice(&input[header_end..header_end + length])
                .map_err(|e| e.to_string())?;
            match encounter.action(request) {
                Ok(face) => (
                    200,
                    "application/json",
                    serde_json::to_vec(&face).map_err(|e| e.to_string())?,
                ),
                Err(error) => (
                    409,
                    "application/json",
                    serde_json::to_vec(&json!({"code":error.code,"error":error.detail}))
                        .map_err(|e| e.to_string())?,
                ),
            }
        }
        _ => (404, "text/plain", b"Not found".to_vec()),
    };
    respond(stream, status, mime, &body).map_err(|e| e.to_string())
}
fn respond(stream: &mut TcpStream, status: u16, mime: &str, body: &[u8]) -> std::io::Result<()> {
    write!(stream,"HTTP/1.1 {status} {}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'\r\n\r\n",if status==200 {"OK"} else {"Refused"},body.len())?;
    stream.write_all(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actions_execute_the_authored_plot_and_stale_actions_preserve_state() {
        let mut app = Encounter::new().unwrap();
        let first = app.face().unwrap();
        let next = app
            .action(json!({"revision":0,"action_id":"thermostat.raise"}))
            .unwrap();
        assert_eq!(app.current.state.target, 215);
        assert_ne!(
            first["basis"]["active_play_id"],
            next["basis"]["active_play_id"]
        );
        assert!(app
            .action(json!({"revision":0,"action_id":"thermostat.lower"}))
            .is_err());
        assert_eq!(app.current.state.target, 215);
        app.action(json!({"revision":1,"action_id":"thermostat.mode.heat"}))
            .unwrap();
        assert_eq!(app.current.state.mode, Mode::Heat);
        assert!(app.current.state.measured.is_none());
    }
    #[test]
    fn no_op_invalid_and_limit_actions_do_not_invent_changes() {
        let mut app = Encounter::new().unwrap();
        app.action(json!({"revision":0,"action_id":"thermostat.mode.off"}))
            .unwrap();
        assert_eq!(app.current.state.revision, 0);
        assert_eq!(app.revision, 1);
        assert!(app
            .action(json!({"revision":1,"action_id":"thermostat.fake"}))
            .is_err());
        for _ in 0..18 {
            let r = app.revision;
            app.action(json!({"revision":r,"action_id":"thermostat.raise"}))
                .unwrap();
        }
        assert_eq!(app.current.state.target, 300);
        assert!(app
            .action(json!({"revision":app.revision,"action_id":"thermostat.raise"}))
            .is_err());
    }
}
