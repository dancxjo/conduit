//! `xtask demo toggle` — operator command for the S4 toggle-demo.
//!
//! Builds the browser WASM runtime, spawns the distributed-toggle-server,
//! starts the static file server, and pipes stdin through so the operator
//! can press Enter to drive activations.

use crate::workspace::workspace_root;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root()?;

    // 1. Build the WASM runtime.
    eprintln!("[toggle] building browser WASM runtime …");
    let status = std::process::Command::new("cargo")
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

    // 2. Start the static file server on a fixed port.
    eprintln!("[toggle] starting static server …");
    let static_port = "4174";
    let mut static_server = std::process::Command::new("node")
        .args(["hosts/browser/static-server.mjs", static_port])
        .current_dir(&root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    // 3. Spawn the distributed-toggle-server and capture the WS URL.
    eprintln!("[toggle] starting toggle server …");
    let mut server = std::process::Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "-p",
            "conduit-std-host",
            "--bin",
            "distributed-toggle-server",
        ])
        .current_dir(&root)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    // Read the first line (the WebSocket URL) from the server's stdout.
    let server_stdout = server
        .stdout
        .take()
        .ok_or("failed to capture toggle server stdout")?;
    let mut reader = std::io::BufReader::new(server_stdout);
    let mut url = String::new();
    std::io::BufRead::read_line(&mut reader, &mut url)?;
    let url = url.trim().to_string();
    if url.is_empty() {
        let _ = server.kill();
        let _ = static_server.kill();
        return Err("toggle server did not emit a WebSocket URL".into());
    }

    eprintln!("[toggle] WebSocket URL: {url}");
    eprintln!(
        "[toggle] open http://127.0.0.1:{static_port}/hosts/browser/distributed-toggle.test.html?ws={url} in a browser",
    );
    eprintln!("[toggle] then press Enter in this terminal to drive activations");
    eprintln!();

    // Forward the remaining stdout (prompts and summary).
    let copy_thread = std::thread::spawn(move || {
        std::io::copy(&mut reader, &mut std::io::stdout().lock()).ok();
    });

    // Wait for the server to exit (stdin is inherited, user presses Enter).
    let status = server.wait()?;
    let _ = copy_thread.join();
    let _ = static_server.kill();
    let _ = static_server.wait();

    if !status.success() {
        return Err(format!("toggle server exited with {status}").into());
    }
    Ok(())
}
