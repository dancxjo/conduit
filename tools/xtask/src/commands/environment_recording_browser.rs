//! Fresh original-recording Owner, current product binary, and pinned Chromium.
//! Sealed recording inputs remain a CPU producer fixture, not physical capture.
use std::{
    error::Error,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SERVICE_TEST: &str = "retain_complete_recording_owner_for_live_browser_fixture";

pub(super) fn prove(root: &Path, handbook_package: &Path) -> Result<()> {
    if !cfg!(target_os = "linux") {
        return Err("recording Owner browser qualification requires Linux".into());
    }
    let handbook = handbook_package.canonicalize()?;
    for file in [
        "application.application.json",
        "index.html",
        "sdk/package.json",
    ] {
        if !handbook.join(file).is_file() {
            return Err(format!("Handbook package is missing {file}").into());
        }
    }
    let cli = root.join("proof/browser/node_modules/@playwright/test/cli.js");
    if !cli.is_file() {
        super::execute(Command::new("npm").current_dir(root).args([
            "--prefix",
            "proof/browser",
            "ci",
            "--ignore-scripts",
            "--prefer-offline",
        ]))?;
    }
    let parent = root.join("target/environment-recording-source");
    fs::create_dir_all(&parent)?;
    let seed = SystemTime::now().duration_since(UNIX_EPOCH)?.subsec_nanos() as u16;
    let mut selected = None;
    for offset in 0..=u16::MAX {
        let candidate = parent.join(format!("b{:04x}", seed.wrapping_add(offset)));
        match fs::create_dir(&candidate) {
            Ok(()) => {
                selected = Some(candidate);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let run = selected
        .ok_or("short recording browser run namespace exhausted")?
        .canonicalize()?;
    let service = run.join("s");
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    #[cfg(unix)]
    if service
        .join("state/control.sock")
        .as_os_str()
        .as_bytes()
        .len()
        > 107
    {
        return Err("recording service control socket exceeds Linux's 107-byte path limit".into());
    }
    let browser = run.join("browser");
    println!("recording_browser_run={}", run.display());
    let binary = build_product(root, &run)?;
    let native_abi = super::environment_recording_source::owner64_abi_evidence(root)?;
    let log_path = run.join("service.log");
    let log = File::create(&log_path)?;
    let child = Command::new("cargo")
        .current_dir(root)
        .args([
            "test",
            "--locked",
            "-p",
            "conduit",
            "--bin",
            "conduit",
            SERVICE_TEST,
            "--",
            "--ignored",
            "--test-threads=1",
        ])
        .env("CONDUIT_ENVIRONMENT_HISTORY64_SERVICE_DIR", &service)
        .env("CONDUIT_ENVIRONMENT_HISTORY64_SERVICE_BINARY", binary)
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log))
        .spawn()?;
    let mut owner = OwnerChild {
        child,
        stop: service.join("stop"),
        finished: false,
    };
    let browser_result: Result<()> = (|| {
        wait_ready(&mut owner.child, &service, Duration::from_secs(15 * 60))?;
        let log = File::create(run.join("browser.log"))?;
        let mut playwright = Command::new("node")
            .current_dir(root)
            .arg(&cli)
            .args([
                "test",
                "--config",
                "proof/browser/environment-recording-owner.config.mjs",
                "--project",
                "chromium",
                "--workers=1",
                "--retries=0",
            ])
            .env("CONDUIT_RECORDING_NATIVE_ABI_PROOF", &native_abi)
            .env("CONDUIT_RECORDING_OWNER_SERVICE", &service)
            .env("CONDUIT_RECORDING_HANDBOOK_PACKAGE", &handbook)
            .env("CONDUIT_RECORDING_BROWSER_OUTPUT", &browser)
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        let status = match wait_exit(&mut playwright, Duration::from_secs(100)) {
            Ok(Some(status)) => status,
            Ok(None) => {
                let _ = playwright.kill();
                let _ = playwright.wait();
                return Err("recording browser timed out; see browser.log".into());
            }
            Err(error) => {
                let _ = playwright.kill();
                let _ = playwright.wait();
                return Err(error);
            }
        };
        if !status.success() {
            return Err(
                format!("recording Chromium proof failed: {status}; see browser.log").into(),
            );
        }
        let result: serde_json::Value =
            serde_json::from_slice(&fs::read(browser.join("result.json"))?)?;
        if result["schema"] != "conduit.proof/environment-recording-owner-browser@1"
            || result["accepted"] != true
        {
            return Err("recording browser did not produce its exact accepted result".into());
        }
        Ok(())
    })();
    // Always request ordinary retirement, including readiness/browser refusal.
    let cleanup = owner.retire();
    browser_result?;
    let status = cleanup?;
    if !status.success() || !service.join("retired").is_file() {
        return Err(
            "recording Owner did not complete ordinary Lull/retirement; see service.log".into(),
        );
    }
    let log = fs::read_to_string(&log_path)?;
    let exact_summary = log
        .lines()
        .any(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"));
    let exact_test = log.lines().any(|line| {
        line.starts_with("test ") && line.contains(SERVICE_TEST) && line.ends_with(" ... ok")
    });
    if !exact_summary || !exact_test {
        return Err(
            "recording service did not execute exactly its one required ignored test".into(),
        );
    }
    println!(
        "recording_browser_accepted={} service_retired={}",
        browser.join("result.json").display(),
        service.join("retired").display()
    );
    Ok(())
}

fn build_product(root: &Path, run: &Path) -> Result<PathBuf> {
    let built = Command::new("cargo")
        .current_dir(root)
        .args([
            "build",
            "--locked",
            "-p",
            "conduit",
            "--bin",
            "conduit",
            "--message-format=json",
        ])
        .output()?;
    fs::write(run.join("build.jsonl"), &built.stdout)?;
    fs::write(run.join("build.stderr.log"), &built.stderr)?;
    if !built.status.success() {
        return Err("current product build failed; see build.stderr.log/build.jsonl".into());
    }
    let mut executables = Vec::new();
    for line in String::from_utf8(built.stdout)?.lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if message["reason"] == "compiler-artifact"
            && message["target"]["name"] == "conduit"
            && message["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
        {
            if let Some(path) = message["executable"].as_str() {
                executables.push(PathBuf::from(path));
            }
        }
    }
    if executables.len() != 1 {
        return Err("Cargo did not report exactly one current conduit executable".into());
    }
    let executable = executables.remove(0).canonicalize()?;
    if !executable.is_file() {
        return Err("Cargo product artifact is missing".into());
    }
    Ok(executable)
}

fn wait_ready(child: &mut Child, service: &Path, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!(
                "recording service exited before readiness: {status}; see service.log"
            )
            .into());
        }
        if service.join("ready.json").is_file() && service.join("state/control.sock").exists() {
            let ready: serde_json::Value =
                serde_json::from_slice(&fs::read(service.join("ready.json"))?)?;
            if ready["schema"] != "conduit.proof/environment-recording-owner-service@1" {
                return Err("recording service readiness schema mismatch".into());
            }
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("recording service readiness timeout; see service.log".into());
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn wait_exit(child: &mut Child, timeout: Duration) -> Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

struct OwnerChild {
    child: Child,
    stop: PathBuf,
    finished: bool,
}
impl OwnerChild {
    fn retire(&mut self) -> Result<ExitStatus> {
        // On compiler/readiness failure the directory may not yet exist. Keep
        // the stop request for a late fixture start; never delete its evidence.
        if let Some(parent) = self.stop.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.stop, b"ordinary fixture retirement requested\n")?;
        match wait_exit(&mut self.child, Duration::from_secs(30))? {
            Some(status) => {
                self.finished = true;
                Ok(status)
            }
            None => {
                let _ = self.child.kill();
                let _ = self.child.wait();
                self.finished = true;
                Err("recording service retirement timed out; child killed after ordinary stop request".into())
            }
        }
    }
}
impl Drop for OwnerChild {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.retire();
            if !self.finished {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }
}
