//! Explicit, bounded visual evidence for one ordinary QEMU journey.
use super::{qmp_display, ConduitosError};
use serde_json::{json, Value};
use std::{fs, io::Read, os::unix::net::UnixStream, path::PathBuf, time::Instant};

pub(super) struct Artifacts {
    directory: PathBuf,
    serial: PathBuf,
    started: Instant,
    entries: Vec<Value>,
    context: Value,
    previous_pixels: Option<String>,
}
impl Artifacts {
    pub(super) fn new(
        directory: PathBuf,
        serial: PathBuf,
        mut context: Value,
    ) -> Result<Self, ConduitosError> {
        fs::create_dir_all(&directory).map_err(io_error)?;
        context["visual_contract"] = json!({"mode":"invariant", "width":1280, "height":800,
            "minimum_non_background_pixels":64,"dynamic_identity_pixels":"allowed", "display":"std-vga"});
        let result = Self {
            directory,
            serial,
            started: Instant::now(),
            entries: Vec::new(),
            context,
            previous_pixels: None,
        };
        result.write("running", None)?;
        Ok(result)
    }

    pub(super) fn capture(
        &mut self,
        stream: &mut UnixStream,
        reader: &mut super::qmp::Reader,
        checkpoint: &str,
        expect_change: bool,
    ) -> Result<(), ConduitosError> {
        if self.entries.len() >= 16 {
            return Err(ConduitosError::refusal(
                "qemu-display-checkpoint-bound",
                "at most sixteen captures admitted",
            ));
        }
        let mut serial = Vec::new();
        fs::File::open(&self.serial)
            .map_err(io_error)?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut serial)
            .map_err(io_error)?;
        if serial.len() > 16 * 1024 * 1024 {
            return Err(ConduitosError::refusal(
                "qemu-display-serial-bound",
                "serial correlation exceeds 16 MiB",
            ));
        }
        let text = std::str::from_utf8(&serial).map_err(|error| {
            ConduitosError::refusal("qemu-display-serial-invalid", error.to_string())
        })?;
        let record = super::journey_records::decode(text)?.pop();
        let boot = super::journey_records::boot(text)?;
        let (frame, health_refusal) =
            qmp_display::capture(stream, reader, &self.directory, checkpoint)?;
        let health_refusal = health_refusal.or_else(|| {
            if frame["width"] != 1280 || frame["height"] != 800 {
                Some(ConduitosError::refusal(
                    "qemu-display-dimensions",
                    "ordinary journey expects 1280x800",
                ))
            } else if frame["non_background_pixels"].as_u64().unwrap_or(0) < 64 {
                Some(ConduitosError::refusal(
                    "qemu-display-content-region",
                    "fewer than 64 non-background pixels",
                ))
            } else {
                None
            }
        });
        let pixels = frame["pixel_sha256"]
            .as_str()
            .expect("capture digest")
            .to_owned();
        let unchanged = expect_change && self.previous_pixels.as_ref() == Some(&pixels);
        self.entries.push(json!({"index":self.entries.len(),"checkpoint":checkpoint,
            "elapsed_millis":self.started.elapsed().as_millis(),"serial_byte_end":serial.len(),
            "guest_record":record,"guest_boot_record":boot,"frame":frame,"expected_change":expect_change,"unchanged":unchanged,
            "health_refusal":health_refusal.as_ref().map(|error|json!({"reason":error.reason,"detail":error.detail}))}));
        self.previous_pixels = Some(pixels);
        self.write("running", None)?;
        if let Some(error) = health_refusal {
            return Err(error);
        }
        if unchanged {
            return Err(ConduitosError::refusal(
                "qemu-display-unchanged-frame",
                checkpoint,
            ));
        }
        Ok(())
    }

    pub(super) fn stopped(&mut self, status: &std::process::ExitStatus, cause: &str) {
        if self.context.get("process_stop").is_some() {
            return;
        }
        self.context["process_stop"] =
            json!({"cause":cause,"status":status.to_string(),"code":status.code()});
    }

    pub(super) fn registers(&mut self, result: Result<Value, ConduitosError>) {
        self.context["register_diagnostic"] = match result {
            Ok(value) => json!({"result":value}),
            Err(error) => json!({"reason":error.reason,"detail":error.detail}),
        };
    }

    pub(super) fn diagnostic_failure(&mut self, error: &ConduitosError) {
        self.context["failure_capture_error"] =
            json!({"reason":error.reason,"detail":error.detail});
    }

    pub(super) fn finish(&self, failure: Option<&ConduitosError>) -> Result<(), ConduitosError> {
        self.write(
            if failure.is_some() {
                "failed"
            } else {
                "complete"
            },
            failure,
        )
    }

    fn write(&self, status: &str, failure: Option<&ConduitosError>) -> Result<(), ConduitosError> {
        let manifest = json!({"schema":"conduit.conduitos/visual-journey@1","proof_class":"freestanding-emulator",
            "status":status,"context":self.context,"checkpoints":self.entries,
            "failure":failure.map(|error|json!({"reason":error.reason,"detail":error.detail})),
            "serial_path":self.serial});
        fs::write(
            self.directory.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).expect("serializable manifest"),
        )
        .map_err(io_error)
    }
}
fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("qemu-display-manifest-io", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uniform_capture_retains_a_correlated_png_before_refusal() {
        use std::io::{BufRead, BufReader, Write};
        let directory =
            std::env::temp_dir().join(format!("conduit-qemu-uniform-{}", std::process::id()));
        let serial = directory.join("serial.log");
        let mut artifacts =
            Artifacts::new(directory.clone(), serial.clone(), json!({"fixture":true})).unwrap();
        fs::write(&serial, "CONDUIT_BOOT_STAGE front-door-ready\n").unwrap();
        let (mut client, mut server) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            let mut input = String::new();
            BufReader::new(server.try_clone().unwrap())
                .read_line(&mut input)
                .unwrap();
            let command: Value = serde_json::from_str(&input).unwrap();
            fs::write(
                command["arguments"]["filename"].as_str().unwrap(),
                b"P6\n2 1\n255\nxxxxxx",
            )
            .unwrap();
            writeln!(server, "{}", json!({"return":{},"id":command["id"]})).unwrap();
        });
        let mut reader = super::super::qmp::Reader::new(client.try_clone().unwrap());
        let failure = artifacts
            .capture(&mut client, &mut reader, "uniform", false)
            .unwrap_err();
        worker.join().unwrap();
        assert_eq!(failure.reason, "qemu-display-uniform-frame");
        artifacts.finish(Some(&failure)).unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["checkpoints"][0]["frame"]["png"], "uniform.png");
        assert_eq!(
            manifest["checkpoints"][0]["health_refusal"]["reason"],
            failure.reason
        );
        assert!(directory.join("uniform.png").is_file());
        assert!(!directory.join("uniform.ppm").exists());
        for name in ["manifest.json", "serial.log", "uniform.png"] {
            fs::remove_file(directory.join(name)).unwrap();
        }
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn failure_manifest_preserves_primary_and_capture_refusals() {
        let directory =
            std::env::temp_dir().join(format!("conduit-qemu-artifacts-{}", std::process::id()));
        let mut artifacts = Artifacts::new(
            directory.clone(),
            directory.join("serial.log"),
            json!({"image_sha256":"fixture"}),
        )
        .unwrap();
        let primary = ConduitosError::refusal("fixture-guest-failed", "primary guest failure");
        artifacts.diagnostic_failure(&ConduitosError::refusal(
            "qemu-qmp-unavailable",
            "capture unavailable",
        ));
        artifacts.finish(Some(&primary)).unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["status"], "failed");
        assert_eq!(manifest["failure"]["reason"], "fixture-guest-failed");
        assert_eq!(
            manifest["context"]["failure_capture_error"]["reason"],
            "qemu-qmp-unavailable"
        );
        assert_eq!(manifest["proof_class"], "freestanding-emulator");
        assert_eq!(manifest["checkpoints"], json!([]));
        fs::remove_file(directory.join("manifest.json")).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
