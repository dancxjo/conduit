//! IA-32 checkpoint at the ELF entry, before guest mutation of EBX handoff truth.
use super::guest::Guest;
use super::{fail, io, multiboot, ConduitosError};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) const ENTRY_SECONDS: u64 = 45;

pub(super) fn observe(
    guest: &mut Guest,
    entry: u64,
    output: &Path,
) -> Result<Value, ConduitosError> {
    let version = Command::new("gdb")
        .arg("--version")
        .output()
        .map_err(|error| {
            fail(
                "graphics-probe-gdb-unavailable",
                format!("run `cargo xtask ci pipeline setup conduitos-ia32` for GDB: {error}"),
            )
        })?;
    if !version.status.success() {
        return Err(fail(
            "graphics-probe-gdb-version",
            "GDB version query failed",
        ));
    }
    let version = String::from_utf8_lossy(&version.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let address = guest.debugger.as_ref().ok_or_else(|| {
        fail(
            "graphics-probe-debugger-unavailable",
            "IA-32 debugger endpoint absent",
        )
    })?;
    let input = output.join("multiboot-information.bin");
    let commands = [
        "set pagination off".to_owned(),
        "set architecture i386".into(),
        format!("target remote {address}"),
        format!("hbreak *{entry:#x}"),
        "continue".into(),
        "printf \"MAGIC=%#x\\n\", $eax".into(),
        "printf \"INFO=%#x\\n\", $ebx".into(),
        "dump binary memory multiboot-information.bin $ebx $ebx+128".into(),
        "disconnect".into(),
    ];
    let stdout = output.join("debugger.log");
    let mut command = Command::new("gdb");
    command.current_dir(output).args(["-nx", "-batch"]);
    for operation in commands {
        command.arg("-ex").arg(operation);
    }
    let log = fs::File::create(&stdout).map_err(io)?;
    let mut child = Debugger(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone().map_err(io)?))
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(|e| {
                fail(
                    "graphics-probe-gdb-unavailable",
                    format!("run `cargo xtask ci pipeline setup conduitos-ia32` for the IA-32 handoff checkpoint: {e}"),
                )
            })?,
    );
    let deadline = Instant::now() + Duration::from_secs(ENTRY_SECONDS);
    let status = loop {
        if let Some(status) = child.0.try_wait().map_err(io)? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.0.kill();
            let _ = child.0.wait();
            return Err(fail(
                "graphics-probe-debugger-timeout",
                "entry checkpoint exceeded 45 seconds",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    };
    let mut text = String::new();
    fs::File::open(&stdout)
        .map_err(io)?
        .take(65537)
        .read_to_string(&mut text)
        .map_err(io)?;
    if text.len() > 65536 {
        return Err(fail(
            "graphics-probe-debugger-log-bound",
            "debugger log exceeds 64 KiB",
        ));
    }
    if !status.success() || !text.lines().any(|line| line == "MAGIC=0x2badb002") {
        return Err(fail(
            "graphics-probe-debugger-checkpoint",
            "debugger did not reach the exact Multiboot product entry",
        ));
    }
    let bytes = fs::read(&input).map_err(io)?;
    if bytes.len() != 128 {
        return Err(fail(
            "graphics-probe-multiboot-extent",
            "debugger must retain exactly 128 handoff bytes",
        ));
    }
    let observed = multiboot::observe(0x2bad_b002, &bytes)
        .map_err(|reason| fail(reason, "Multiboot observation invalid"))?;
    Ok(
        json!({"handoff":"multiboot1","debugger_version":version,"entry_address":format!("{entry:#x}"),"framebuffer_information_present":observed.is_some(),"framebuffer":observed.map(|frame|json!({"physical_address":format!("{:#x}",frame.address),"width":frame.width,"height":frame.height,"pitch":frame.pitch,"bits_per_pixel":frame.bits_per_pixel,"framebuffer_type":frame.framebuffer_type,"red":frame.red,"green":frame.green,"blue":frame.blue}))}),
    )
}

struct Debugger(Child);
impl Drop for Debugger {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
