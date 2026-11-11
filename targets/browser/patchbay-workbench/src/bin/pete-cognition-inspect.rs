//! Serve an actual exported cognition run as a read-only historical projection.
use conduit_browser_patchbay_workbench::{snapshot_from_pete_cognition_run, PatchbayHtmlServer};
use std::{error::Error, fs::OpenOptions, io::Write, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = PathBuf::from(
        args.next()
            .ok_or("usage: pete-cognition-inspect RUN_DIRECTORY [--snapshot-output PATH]")?,
    );
    let mut snapshot_output = None;
    while let Some(arg) = args.next() {
        if arg == "--snapshot-output" && snapshot_output.is_none() {
            snapshot_output = Some(PathBuf::from(
                args.next().ok_or("--snapshot-output requires PATH")?,
            ));
        } else {
            return Err("unsupported inspection argument".into());
        }
    }
    let snapshot = snapshot_from_pete_cognition_run(&directory)?;
    if let Some(path) = snapshot_output {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&snapshot.encode()?)?;
    }
    let server = PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .map_err(|e| format!("historical inspector server: {e:?}"))?;
    println!(
        "http://{}",
        server
            .local_addr()
            .map_err(|e| format!("inspector address: {e:?}"))?
    );
    std::io::stdout().flush()?;
    server
        .serve()
        .map_err(|e| format!("historical inspector stopped: {e:?}").into())
}
