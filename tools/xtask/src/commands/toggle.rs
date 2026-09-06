//! `xtask demo toggle` — operator command for the distributed toggle demo.
//!
//! Builds the browser WASM runtime, spawns the distributed-toggle-server,
//! starts the static file server, and pipes stdin through so the operator
//! can press Enter to drive triggers.

use crate::workspace::workspace_root;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const STATIC_PORTS: std::ops::RangeInclusive<u16> = 4174..=4183;
const STATIC_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn spawn(command: &mut Command) -> std::io::Result<Self> {
        command.spawn().map(|child| Self { child: Some(child) })
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("guard always owns its child")
    }

    fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.child_mut().wait()
    }

    fn terminate_with_output(&mut self) -> std::io::Result<std::process::Output> {
        let mut child = self.child.take().expect("guard always owns its child");
        if child.try_wait()?.is_none() {
            let _ = child.kill();
        }
        child.wait_with_output()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
}

fn available_static_port() -> Result<u16, Box<dyn std::error::Error>> {
    for port in STATIC_PORTS {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    Err(format!(
        "no available static-server port in {}..={}",
        STATIC_PORTS.start(),
        STATIC_PORTS.end()
    )
    .into())
}

fn serves_page(port: u16, page: &str) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let timeout = Some(Duration::from_millis(250));
    if stream.set_read_timeout(timeout).is_err() || stream.set_write_timeout(timeout).is_err() {
        return false;
    }
    let request = format!("GET {page} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = [0_u8; 64];
    stream
        .read(&mut response)
        .is_ok_and(|read| response[..read].starts_with(b"HTTP/1.1 200"))
}

fn wait_for_static_server(
    server: &mut ChildGuard,
    port: u16,
    page: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + STATIC_STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if server.child_mut().try_wait()?.is_some() {
            break;
        }
        if serves_page(port, page) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    let output = server.terminate_with_output()?;
    Err(format!(
        "static server failed to serve on 127.0.0.1:{port}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
    .into())
}

fn encode_query_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    run_page("toggle", "/proof/browser/distributed-toggle.test.html")
}

pub fn run_site() -> Result<(), Box<dyn std::error::Error>> {
    run_page("site", "/proof/browser/conduit-site.html")
}

fn run_page(label: &str, page: &str) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;

    // 1. Build the WASM runtime.
    eprintln!("[{label}] building browser WASM runtime …");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .current_dir(&root)
        .status()?;
    if !status.success() {
        return Err("WASM build failed".into());
    }

    // 2. Start the static file server on an available port and prove it is serving.
    eprintln!("[{label}] starting static server …");
    let static_port = available_static_port()?;
    let mut static_command = Command::new("node");
    static_command
        .args(["proof/browser/static-server.mjs", &static_port.to_string()])
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut static_server = ChildGuard::spawn(&mut static_command)?;
    wait_for_static_server(&mut static_server, static_port, page)?;

    // 3. Spawn the distributed-toggle-server and capture the WS URL.
    eprintln!("[{label}] starting toggle server …");
    let mut server_command = Command::new("cargo");
    server_command
        .args([
            "run",
            "--quiet",
            "-p",
            "conduit-std-host",
            "--bin",
            "distributed-toggle-server",
        ])
        .current_dir(&root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut server = ChildGuard::spawn(&mut server_command)?;

    // Read the first line (the WebSocket URL) from the server's stdout.
    let server_stdout = server
        .child_mut()
        .stdout
        .take()
        .ok_or("failed to capture toggle server stdout")?;
    let mut reader = std::io::BufReader::new(server_stdout);
    let mut url = String::new();
    reader.read_line(&mut url)?;
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("toggle server did not emit a WebSocket URL".into());
    }

    let encoded_url = encode_query_component(&url);
    eprintln!("[{label}] WebSocket URL: {url}");
    eprintln!("[{label}] open http://127.0.0.1:{static_port}{page}?ws={encoded_url} in a browser",);
    eprintln!("[{label}] then press Enter in this terminal to drive triggers");
    eprintln!();

    // Forward the remaining stdout (prompts and summary).
    let copy_thread = std::thread::spawn(move || {
        std::io::copy(&mut reader, &mut std::io::stdout().lock()).ok();
    });

    // Wait for the server to exit (stdin is inherited, user presses Enter).
    let status = server.wait()?;
    let _ = copy_thread.join();

    if !status.success() {
        return Err(format!("toggle server exited with {status}").into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::encode_query_component;

    #[test]
    fn websocket_url_is_encoded_as_one_query_value() {
        assert_eq!(
            encode_query_component("ws://127.0.0.1:4174/conduit?x=1&y=2"),
            "ws%3A%2F%2F127.0.0.1%3A4174%2Fconduit%3Fx%3D1%26y%3D2"
        );
    }
}
