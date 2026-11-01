//! Explicit, signed remote Body administration. Transport acceptance is never
//! reported as a completed rename; only a pinned Root receipt is success.
use conduit_body::{BodyRenameReceipt, BodyRenameRequest, BODY_RENAME_REQUEST_SCHEMA};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(clap::Args, Debug)]
pub(crate) struct Args {
    #[arg(long)]
    pub endpoint: SocketAddr,
    #[arg(long)]
    pub body_id: String,
    #[arg(long)]
    pub host_id: String,
    #[arg(long)]
    pub boot_id: String,
    #[arg(long)]
    pub expected_sequence: u64,
    #[arg(long)]
    pub name: String,
    /// Existing provisioned owner's Ed25519 seed:32 raw bytes or64 hex digits.
    #[arg(long)]
    pub owner_key_file: PathBuf,
    /// Independently pinned Root verifying key:32 raw bytes or64 hex digits.
    #[arg(long)]
    pub root_key_file: PathBuf,
    #[arg(long)]
    pub attribution: String,
}

pub(crate) fn run(args: Args) -> Result<(), String> {
    let root = read_key(&args.root_key_file)?;
    let mut nonce = [0; 32];
    getrandom::fill(&mut nonce).map_err(|_| "rename request randomness unavailable")?;
    let mut request = BodyRenameRequest {
        schema: BODY_RENAME_REQUEST_SCHEMA.into(),
        body_id: serde_json::from_value(serde_json::Value::String(args.body_id))
            .map_err(|_| "rename Body identity")?,
        host_id: args.host_id.into(),
        boot_id: args.boot_id.into(),
        expected_sequence: args.expected_sequence,
        nonce,
        new_name: args.name,
        attribution: args.attribution,
        signature: vec![],
    };
    let mut seed = read_key(&args.owner_key_file)?;
    request.sign(&seed);
    seed.fill(0);
    let encoded = serde_json::to_vec(&request).map_err(|_| "rename request encoding")?;
    if encoded.len() > 1024 {
        return Err("rename request exceeds device carrier".into());
    }
    let response = exchange(args.endpoint, "POST", "/body/join", &encoded)?;
    let queued: serde_json::Value =
        serde_json::from_slice(&response).map_err(|_| "rename queue response")?;
    if queued.get("schema").and_then(|value| value.as_str())
        != Some("conduit.environment/body-join-queued@1")
    {
        return Err("rename was not queued".into());
    }
    let ticket = queued
        .get("ticket")
        .and_then(|value| value.as_u64())
        .filter(|ticket| *ticket > 0 && *ticket <= u64::from(u32::MAX))
        .ok_or("rename queue ticket")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let response = exchange(
            args.endpoint,
            "GET",
            &format!("/body/join/result?id={ticket}"),
            &[],
        )?;
        if let Ok(receipt) = serde_json::from_slice::<BodyRenameReceipt>(&response) {
            receipt
                .verify(&root, &request)
                .map_err(|e| format!("rename receipt: {e:?}"))?;
            #[derive(serde::Serialize)]
            struct Completion<'a> {
                schema: &'static str,
                request: &'a BodyRenameRequest,
                receipt: &'a BodyRenameReceipt,
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&Completion {
                    schema: "conduit.body/rename-completion@1",
                    request: &request,
                    receipt: &receipt,
                })
                .map_err(|_| "rename receipt encoding")?
            );
            return Ok(());
        }
        let pending: serde_json::Value =
            serde_json::from_slice(&response).map_err(|_| "rename result encoding")?;
        if pending.get("schema").and_then(|v| v.as_str())
            != Some("conduit.environment/body-join-queued@1")
        {
            return Err("Root did not return a successful rename receipt; inspect current state before retrying".into());
        }
        if Instant::now() >= deadline {
            return Err("rename outcome unknown; inspect current state before retrying".into());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
fn read_key(path: &Path) -> Result<[u8; 32], String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "open rename key file")?
        .take(129)
        .read_to_end(&mut bytes)
        .map_err(|_| "read rename key file")?;
    let result = if bytes.len() == 32 {
        bytes.as_slice().try_into().map_err(|_| "rename key length")
    } else {
        let text = std::str::from_utf8(&bytes)
            .map(str::trim)
            .map_err(|_| "rename key encoding")?;
        if text.len() != 64 {
            bytes.fill(0);
            return Err("rename key must contain 32 raw bytes or 64 hex digits".into());
        }
        let mut key = [0; 32];
        for (out, pair) in key.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
            let digit = |b: u8| {
                (b as char)
                    .to_digit(16)
                    .map(|v| v as u8)
                    .ok_or("rename key encoding")
            };
            *out = digit(pair[0])? * 16 + digit(pair[1])?;
        }
        Ok(key)
    };
    bytes.fill(0);
    result.map_err(str::to_owned)
}
fn exchange(
    endpoint: SocketAddr,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    let mut socket = TcpStream::connect_timeout(&endpoint, Duration::from_secs(3))
        .map_err(|e| format!("rename transport: {e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| e.to_string())?;
    write!(socket, "{method} {path} HTTP/1.1\r\nHost: {endpoint}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).map_err(|e| e.to_string())?;
    socket.write_all(body).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut response = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("rename response deadline; inspect current state before retrying".into());
        }
        socket
            .set_read_timeout(Some(remaining))
            .map_err(|e| e.to_string())?;
        let mut chunk = [0; 512];
        let count = socket
            .read(&mut chunk)
            .map_err(|e| format!("rename response: {e}"))?;
        if count == 0 {
            break;
        }
        if response.len() + count > 8192 {
            return Err("rename response capacity".into());
        }
        response.extend_from_slice(&chunk[..count]);
        if let Some(split) = response.windows(4).position(|w| w == b"\r\n\r\n") {
            let header =
                std::str::from_utf8(&response[..split]).map_err(|_| "rename HTTP header")?;
            let mut length = None;
            for line in header.lines().skip(1) {
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("transfer-encoding") {
                        return Err("unsupported rename HTTP framing".into());
                    }
                    if name.eq_ignore_ascii_case("content-length") {
                        if length.is_some() {
                            return Err("ambiguous rename HTTP length".into());
                        }
                        length = Some(
                            value
                                .trim()
                                .parse::<usize>()
                                .map_err(|_| "rename HTTP length")?,
                        );
                    }
                }
            }
            if let Some(length) = length {
                if length > 8192 - split - 4 {
                    return Err("rename response capacity".into());
                }
                if response.len() >= split + 4 + length {
                    response.truncate(split + 4 + length);
                    break;
                }
            }
        }
    }
    let split = response
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("rename HTTP response")?;
    let header = std::str::from_utf8(&response[..split]).map_err(|_| "rename HTTP header")?;
    if !header.starts_with("HTTP/1.1 200 ") && !header.starts_with("HTTP/1.1 202 ") {
        return Err("rename HTTP refusal; inspect current state before retrying".into());
    }
    Ok(response[split + 4..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::net::TcpListener;

    #[test]
    fn body_rename_requires_exact_scope_keys_and_attribution() {
        let parsed = crate::cli::Cli::try_parse_from([
            "conduit",
            "body",
            "rename",
            "--endpoint",
            "127.0.0.1:8080",
            "--body-id",
            "body/house",
            "--host-id",
            "host/root",
            "--boot-id",
            "boot/current",
            "--expected-sequence",
            "42",
            "--name",
            "Hamm Creek House",
            "--owner-key-file",
            "/private/owner",
            "--root-key-file",
            "/private/root",
            "--attribution",
            "House operator",
        ])
        .unwrap();
        assert!(matches!(
            parsed.command,
            Some(crate::cli::Command::Body {
                command: Some(crate::cli::BodyCommand::Rename(_))
            })
        ));
        assert!(crate::cli::Cli::try_parse_from([
            "conduit",
            "body",
            "rename",
            "--name",
            "Hamm Creek House"
        ])
        .is_err());
    }

    #[test]
    fn body_rename_http_refusal_is_not_success() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 256];
                let count = socket.read(&mut chunk).unwrap();
                assert!(count > 0 && request.len() + count <= 2048);
                request.extend_from_slice(&chunk[..count]);
                if request
                    .windows(4)
                    .position(|w| w == b"\r\n\r\n")
                    .is_some_and(|split| request.len() >= split + 6)
                {
                    break;
                }
            }
            socket
                .write_all(
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });
        assert!(exchange(address, "POST", "/body/join", b"{}").is_err());
        server.join().unwrap();
    }
}

#[cfg(test)]
mod framing_tests {
    use super::*;
    use std::net::TcpListener;
    #[test]
    fn body_rename_content_length_completes_without_waiting_for_eof() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 256];
                let count = socket.read(&mut chunk).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&chunk[..count]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\n{}",
                )
                .unwrap();
            assert_eq!(socket.read(&mut [0]).unwrap(), 0);
        });
        assert_eq!(
            exchange(address, "GET", "/body/join/result?id=1", &[]).unwrap(),
            b"{}"
        );
        server.join().unwrap();
    }
}
