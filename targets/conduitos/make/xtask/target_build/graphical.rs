//! Source-correlated capture of the normal firmware-framebuffer product.
use super::super::{emitted_line, qmp, qmp_display, ConduitosError};
use serde_json::{json, Value};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf, process::Child};

pub(super) struct Capture {
    directory: PathBuf,
    socket_directory: PathBuf,
}
impl Capture {
    pub(super) fn prepare(target: &std::path::Path) -> Result<Self, ConduitosError> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| refuse(e.to_string()))?
            .as_nanos();
        let name = format!("aarch64-frame-{}-{nonce}", std::process::id());
        let directory = target.join(&name);
        let socket_directory = std::env::temp_dir().join(name);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&socket_directory)
            .map_err(io)?;
        fs::create_dir(&directory).map_err(io)?;
        Ok(Self {
            directory,
            socket_directory,
        })
    }
    pub(super) fn record_launch(
        &self,
        command: &std::process::Command,
        root: &std::path::Path,
        firmware: &std::path::Path,
        image: &std::path::Path,
    ) -> Result<(), ConduitosError> {
        let executable = std::env::var_os("PATH")
            .and_then(|path| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join(command.get_program()))
                    .find(|candidate| candidate.is_file())
            })
            .ok_or_else(|| refuse("emulator executable unavailable"))?;
        let version = std::process::Command::new(&executable)
            .arg("--version")
            .output()
            .map_err(io)?;
        if !version.status.success() || version.stdout.len() > 4096 {
            return Err(refuse("emulator version unavailable"));
        }
        let dirty = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(root)
            .output()
            .map_err(io)?;
        if !dirty.status.success() {
            return Err(refuse("source status unavailable"));
        }
        let launch = json!({"source_commit":super::super::report::git_head(root)?,
            "source_dirty":!dirty.stdout.is_empty(),
            "program":executable,"arguments":command.get_args().map(|arg|
                arg.to_string_lossy().into_owned()).collect::<Vec<_>>(),
            "emulator_version":String::from_utf8_lossy(&version.stdout),
            "emulator_sha256":super::super::report::sha256_file(&executable)?,
            "firmware":firmware,"firmware_sha256":super::super::report::sha256_file(firmware)?,
            "image":image,"image_sha256":super::super::report::sha256_file(image)?,
            "boot_deadline_seconds":45,"serial_maximum_bytes":1048576});
        fs::write(
            self.directory.join("launch.json"),
            serde_json::to_vec_pretty(&launch).map_err(|e| refuse(e.to_string()))?,
        )
        .map_err(io)
    }
    pub(super) fn serial_path(&self) -> PathBuf {
        self.directory.join("serial.log")
    }
    pub(super) fn stderr_path(&self) -> PathBuf {
        self.directory.join("qemu-stderr.log")
    }
    pub(super) fn endpoint(&self) -> String {
        format!(
            "unix:{},server=on,wait=off",
            self.socket_directory.join("qmp").display()
        )
    }
    pub(super) fn finish(
        &self,
        child: &mut Child,
        transcript: &str,
        product: &Value,
        started: std::time::Instant,
    ) -> Result<Value, ConduitosError> {
        if transcript.len() > 1024 * 1024 {
            return Err(refuse("serial exceeds 1 MiB"));
        }
        let face: Value = serde_json::from_str(
            emitted_line::complete_json_line(transcript, "CONDUIT_FIRMWARE_FACE ")
                .ok_or_else(|| refuse("no complete graphical Sign"))?,
        )
        .map_err(|e| refuse(e.to_string()))?;
        let mapping: Value = serde_json::from_str(
            emitted_line::complete_json_line(transcript, "CONDUIT_FRAMEBUFFER_MAPPING ")
                .ok_or_else(|| refuse("no complete mapping Sign"))?,
        )
        .map_err(|e| refuse(e.to_string()))?;
        if [
            &face["face"]["identity"],
            &face["show"]["show_id"],
            &face["show"]["mask_plan_id"],
            &face["show"]["mask_active_play_id"],
            &face["framebuffer"]["base_id"],
        ]
        .iter()
        .any(|id| id.as_str().is_none_or(str::is_empty))
            || face["status"] != "shown"
            || face["target"] != "conduitos/aarch64/virt"
            || face["profile_id"] != product["profile_id"]
            || face["build_id"] != product["build_id"]
            || face["image_id"] != product["image_id"]
            || mapping["host_id"] != product["host_id"]
            || mapping["boot_id"] != product["boot_id"]
            || mapping["base_id"] != face["framebuffer"]["base_id"]
            || face["mask_render"]["scanout_pixels_written"]
                .as_u64()
                .unwrap_or(0)
                == 0
            || face["show"]["face_id"] != face["face"]["identity"]
            || face["show"]["manifestation"]["lifecycle"] != "Available"
            || face["show"]["manifestation"]["failure"] != Value::Null
            || face["show"]["manifestation"]["host_id"] != product["host_id"]
            || face["show"]["manifestation"]["boot_id"] != product["boot_id"]
            || face["show"]["mask_plan_id"] != face["mask_render"]["mask_plan_id"]
            || face["show"]["mask_active_play_id"] != face["mask_render"]["active_play_id"]
            || face["show"]["face_revision"] != face["mask_render"]["face_revision"]
            || face["framebuffer"]["base_id"] != face["mask_render"]["display_base_id"]
            || face["local_input_available"] != false
        {
            return Err(refuse("graphical Sign does not match the current product"));
        }
        let (mut stream, mut reader) = qmp::connect_traced(
            &self.socket_directory.join("qmp"),
            child,
            Some(&self.directory.join("qmp.jsonl")),
        )?;
        qmp::request(
            &mut stream,
            &mut reader,
            br#"{"execute":"stop"}"#,
            "capture-stop",
        )?;
        let (frame, unhealthy) = qmp_display::capture(
            &mut stream,
            &mut reader,
            &self.directory,
            "creche-read-only",
        )?;
        if let Some(error) = unhealthy {
            return Err(error);
        }
        if frame["width"] != face["framebuffer"]["width"]
            || frame["height"] != face["framebuffer"]["height"]
        {
            return Err(refuse("capture dimensions do not match admitted scanout"));
        }
        let proof = json!({"schema":"conduit.conduitos/firmware-frame-capture@1",
            "face":face,"mapping":mapping,"frame":frame,"directory":self.directory,
            "boot_to_capture_millis":started.elapsed().as_millis(),
            "physical_scanout_proved":false,"gpu_acceleration_proved":false,
            "local_input_proved":false,"accepted_release":false});
        fs::write(
            self.directory.join("capture.json"),
            serde_json::to_vec_pretty(&proof).map_err(|e| refuse(e.to_string()))?,
        )
        .map_err(io)?;
        qmp::request(
            &mut stream,
            &mut reader,
            br#"{"execute":"cont"}"#,
            "capture-continue",
        )?;
        Ok(proof)
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.socket_directory.join("qmp"));
        let _ = fs::remove_dir(&self.socket_directory);
    }
}
fn refuse(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("aarch64-graphical-proof-invalid", detail)
}
fn io(error: std::io::Error) -> ConduitosError {
    refuse(error.to_string())
}
