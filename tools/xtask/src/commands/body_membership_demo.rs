//! Complete repository entrance for the live native/browser Body membership demo.

use crate::cli::GlobalOpts;
use crate::process::{run_step, Step};
use crate::workspace::workspace_root;
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const STATIC_PORT_START: u16 = 4184;
const STATIC_PORT_END: u16 = 4193;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const PAGE: &str = "/proof/browser/webchat.test.html";
const BUILD_STEPS: &[Step] = &[
    Step::new(
        "demo.body-membership.browser-runtime",
        "Build the browser Part runtime",
        "cargo",
        &[
            "build",
            "-p",
            "conduit-browser-runtime",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ],
    ),
    Step::new(
        "demo.body-membership.native",
        "Build the native Parts steward",
        "cargo",
        &["build", "-p", "patchbay-native"],
    ),
    Step::new(
        "demo.body-membership.chat-line",
        "Build the browser chat Line",
        "cargo",
        &["build", "-p", "conduit-std-host", "--bin", "webchat-server"],
    ),
];

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

pub(super) fn run(opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;
    for step in build_steps() {
        run_step(step, &root, opts)?;
    }
    if opts.dry_run {
        if !opts.quiet && !opts.json {
            println!("» [demo.body-membership] launch bounded browser servers and native Parts");
        }
        return Ok(());
    }

    let static_port = available_static_port()?;
    let mut static_server = ChildGuard(
        Command::new("node")
            .args(["proof/browser/static-server.mjs", &static_port.to_string()])
            .current_dir(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    wait_for_page(&mut static_server.0, static_port)?;

    let mut chat_server = ChildGuard(
        Command::new("target/debug/webchat-server")
            .arg("127.0.0.1:0")
            .current_dir(&root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let stdout = chat_server
        .0
        .stdout
        .take()
        .ok_or("cannot observe browser chat server readiness")?;
    let (chat_url, forwarder) = wait_for_chat_url(stdout)?;

    let page_url = format!("http://127.0.0.1:{static_port}{PAGE}");
    eprintln!("[body-membership] browser page: {page_url}");
    eprintln!("[body-membership] browser chat Line: {chat_url}");
    let mut native = Command::new("target/debug/patchbay-native");
    native.args([
        "--form",
        "forms/hello/main.conduit",
        "--body-parts-demo",
        "--browser-page-url",
        &page_url,
        "--browser-chat-url",
        &chat_url,
    ]);
    if let Ok(path) = std::env::var("PICO_W_LINK_PORT") {
        eprintln!("[body-membership] Pico admission Line: {path}");
        native.args(["--pico-admission-port", &path]);
    }
    let status = native.current_dir(&root).status()?;
    drop(chat_server);
    let _ = forwarder.join();
    if !status.success() {
        return Err(format!("native Parts demonstration exited with {status}").into());
    }
    Ok(())
}

fn build_steps() -> &'static [Step] {
    BUILD_STEPS
}

fn available_static_port() -> Result<u16, Box<dyn std::error::Error>> {
    (STATIC_PORT_START..=STATIC_PORT_END)
        .find(|port| TcpListener::bind(("127.0.0.1", *port)).is_ok())
        .ok_or_else(|| "no bounded Body membership static-server port is available".into())
}

fn wait_for_page(server: &mut Child, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(status) = server.try_wait()? {
            return Err(format!("browser static server exited with {status}").into());
        }
        if serves_page(port) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("browser static server did not become ready within 10 seconds".into())
}

fn serves_page(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let timeout = Some(Duration::from_millis(250));
    if stream.set_read_timeout(timeout).is_err() || stream.set_write_timeout(timeout).is_err() {
        return false;
    }
    let request = format!("GET {PAGE} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    let mut response = [0; 64];
    stream.write_all(request.as_bytes()).is_ok()
        && stream
            .read(&mut response)
            .is_ok_and(|read| response[..read].starts_with(b"HTTP/1.1 200"))
}

fn wait_for_chat_url(
    output: impl Read + Send + 'static,
) -> Result<(String, std::thread::JoinHandle<()>), Box<dyn std::error::Error>> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let forwarder = std::thread::spawn(move || {
        let mut output = std::io::BufReader::new(output);
        let mut line = String::new();
        while output.read_line(&mut line).is_ok_and(|read| read != 0) {
            if let Some(address) = line
                .split_whitespace()
                .find_map(|field| field.strip_prefix("address="))
            {
                let _ = sender.send(format!("ws://{address}"));
            }
            print!("{line}");
            line.clear();
        }
    });
    let url = receiver
        .recv_timeout(STARTUP_TIMEOUT)
        .map_err(|_| "browser chat server did not become ready within 10 seconds")?;
    Ok((url, forwarder))
}

#[cfg(test)]
mod tests {
    use super::build_steps;

    #[test]
    fn demo_builds_exact_browser_and_native_entrances() {
        assert_eq!(build_steps().len(), 3);
        assert_eq!(build_steps()[0].id, "demo.body-membership.browser-runtime");
        assert!(build_steps()[1].args.contains(&"patchbay-native"));
        assert!(build_steps()[2].args.contains(&"webchat-server"));
    }
}
