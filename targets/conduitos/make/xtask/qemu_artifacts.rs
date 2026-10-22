//! Explicit, bounded visual evidence for one ordinary QEMU journey.
use super::{ConduitosError, qmp_display};
use serde_json::{Value, json};
use std::{fs, io::Read, os::unix::net::UnixStream, path::PathBuf, time::Instant};

// The bounded lifecycle, refusal, navigation, and reentry journey plus failure capture.
const DEFAULT_MAX_CHECKPOINTS: usize = 20;
// Ordinary journey plus the admitted six typing and three navigation observations,
// including one retained failure capture.
const MAX_ADMITTED_CHECKPOINTS: usize = 32;

pub(super) struct Artifacts {
    directory: PathBuf,
    serial: PathBuf,
    started: Instant,
    entries: Vec<Value>,
    capture_limit: usize,
    context: Value,
    previous_pixels: Option<String>,
    viewport: Option<super::proof_viewport::Viewport>,
    viewport_focus_seen: bool,
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
        context["capture_contract"] = json!({"maximum_checkpoints": DEFAULT_MAX_CHECKPOINTS});
        let result = Self {
            directory,
            serial,
            started: Instant::now(),
            entries: Vec::with_capacity(DEFAULT_MAX_CHECKPOINTS),
            capture_limit: DEFAULT_MAX_CHECKPOINTS,
            context,
            previous_pixels: None,
            viewport: None,
            viewport_focus_seen: false,
        };
        result.write("running", None)?;
        Ok(result)
    }

    pub(super) fn with_viewport(
        mut self,
        viewport: Option<super::proof_viewport::Viewport>,
    ) -> Result<Self, ConduitosError> {
        if let Some(viewport) = viewport {
            if !self.entries.is_empty() {
                return Err(ConduitosError::refusal(
                    "proof-viewport-admission",
                    "capture already started",
                ));
            }
            let (width, height) = viewport.dimensions();
            self.viewport = Some(viewport);
            self.context["visual_contract"]["width"] = json!(width);
            self.context["visual_contract"]["height"] = json!(height);
            self.context["viewport_request"] = json!({"width":width,"height":height,"selection":"stdVGA EDID preference; actual boot selection must match", "provider_format":"not serialized; actual Mask display bounds and QMP pixels checked"});
            self.write("running", None)?;
        }
        Ok(self)
    }

    pub(super) fn observation_origin(&self) -> Instant {
        self.started
    }

    /// Prepare a finite capture plan before the journey starts. Once any
    /// capture exists, the admitted bound cannot be changed to erase pressure.
    pub(super) fn with_capture_limit(mut self, maximum: usize) -> Result<Self, ConduitosError> {
        if !(1..=MAX_ADMITTED_CHECKPOINTS).contains(&maximum) || !self.entries.is_empty() {
            return Err(ConduitosError::refusal(
                "qemu-display-capture-admission",
                "capture limit is invalid or the journey already started",
            ));
        }
        if maximum > self.entries.capacity() {
            self.entries.reserve_exact(maximum - self.entries.len());
        }
        self.capture_limit = maximum;
        self.context["capture_contract"] = json!({"maximum_checkpoints": maximum});
        self.write("running", None)?;
        Ok(self)
    }

    pub(super) fn capture(
        &mut self,
        stream: &mut UnixStream,
        reader: &mut super::qmp::Reader,
        checkpoint: &str,
        expect_change: bool,
    ) -> Result<(), ConduitosError> {
        if self.entries.len() >= self.capture_limit {
            return Err(ConduitosError::refusal(
                "qemu-display-checkpoint-bound",
                format!("at most {} captures admitted", self.capture_limit),
            ));
        }
        let observation_start_ns = self.started.elapsed().as_nanos();
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
        // QEMU may still be writing the last record (including a UTF-8 scalar).
        // Correlate only newline-committed bytes; retain the full raw log.
        serial.truncate(complete_serial_end(&serial));
        let text = std::str::from_utf8(&serial).map_err(|error| {
            ConduitosError::refusal("qemu-display-serial-invalid", error.to_string())
        })?;
        if let Some(profile) = text
            .lines()
            .find_map(|line| line.strip_prefix("CONDUIT_GRAPHICAL_PROFILE "))
        {
            self.context["graphical_profile"] = serde_json::from_str(profile).map_err(|error| {
                ConduitosError::refusal("graphical-profile-record-invalid", error.to_string())
            })?;
        }
        let record = super::journey_records::latest_checkpoint(text)?;
        let boot = super::journey_records::boot(text)?;
        let native = super::journey_records::native_checkpoint(text)?;
        if let Some(viewport) = self.viewport {
            let boot = boot.as_ref().ok_or_else(|| {
                ConduitosError::refusal("proof-viewport-mismatch", "guest Boot identity absent")
            })?;
            let custody = viewport.guest(text, boot)?;
            self.viewport_focus_seen |= custody["focus_records"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty());
            self.context["viewport_guest_evidence"] = custody;
        }
        let timing = super::native_timing::parse(text)?;
        let frame_request_start_ns = self.started.elapsed().as_nanos();
        let (frame, health_refusal) =
            qmp_display::capture(stream, reader, &self.directory, checkpoint)?;
        let frame_response_end_ns = self.started.elapsed().as_nanos();
        let health_refusal = health_refusal.or_else(|| {
            if self
                .viewport
                .unwrap_or(super::proof_viewport::Viewport::Wide)
                .capture(&frame)
                .is_err()
            {
                Some(ConduitosError::refusal(
                    "qemu-display-dimensions",
                    if self.viewport.is_some() {
                        "actual capture does not match admitted proof viewport"
                    } else {
                        "ordinary journey expects 1280x800"
                    },
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
            "journey_step_ids":journey_step_ids(checkpoint),
            "elapsed_millis":self.started.elapsed().as_millis(),"serial_byte_end":serial.len(),
            "native_timing":timing,
            "host_observation":{"clock":"std::time::Instant","units":"nanoseconds",
                "checkpoint_start":observation_start_ns,"frame_request_start":frame_request_start_ns,
                "frame_response_end":frame_response_end_ns,
                "interpretation":"Observer intervals include serial reading and QMP capture. They are not input-to-paint latency; QMP acceptance is not guest input acceptance."},
            "guest_record":record,"guest_boot_record":boot,"native_receipts":native,"frame":frame,"expected_change":expect_change,"unchanged":unchanged,
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

    pub(super) fn diagnostic_failure(&mut self, error: &ConduitosError) {
        self.context["failure_capture_error"] =
            json!({"reason":error.reason,"detail":error.detail});
    }

    pub(super) fn finish(&self, failure: Option<&ConduitosError>) -> Result<(), ConduitosError> {
        if failure.is_none() && self.viewport.is_some() && !self.viewport_focus_seen {
            let error = ConduitosError::refusal(
                "proof-viewport-mismatch",
                "no acknowledged guest Mask extent evidence",
            );
            self.write("failed", Some(&error))?;
            return Err(error);
        }
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

fn journey_step_ids(checkpoint: &str) -> &'static [&'static str] {
    match checkpoint {
        "home" => &[conduit_home_model::HOME_ARRIVED_STEP_ID],
        "patchbay-workspace" => &[conduit_home_model::PATCHBAY_REQUESTED_STEP_ID],
        _ => &[],
    }
}
fn io_error(error: std::io::Error) -> ConduitosError {
    ConduitosError::refusal("qemu-display-manifest-io", error.to_string())
}

/// A live serial read is a prefix, not an atomic record snapshot.
fn complete_serial_end(serial: &[u8]) -> usize {
    serial
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |end| end + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_viewport_cannot_finish_without_guest_extent_evidence() {
        let directory =
            std::env::temp_dir().join(format!("conduit-viewport-artifacts-{}", std::process::id()));
        let artifacts = Artifacts::new(directory.clone(), directory.join("serial.log"), json!({}))
            .unwrap()
            .with_viewport(Some(super::super::proof_viewport::Viewport::Constrained))
            .unwrap();
        assert!(artifacts.finish(None).is_err());
        let manifest: Value =
            serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["status"], "failed");
        assert_eq!(manifest["context"]["visual_contract"]["width"], 640);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn live_serial_snapshot_excludes_only_the_uncommitted_tail() {
        let complete = b"CONDUIT_NATIVE_TIMING malformed\n";
        let mut snapshot = complete.to_vec();
        snapshot.extend_from_slice(b"CONDUIT_NATIVE_FACE {\"text\":\"\xf0\x9f");
        let end = complete_serial_end(&snapshot);
        assert_eq!(&snapshot[..end], complete);
        let text = std::str::from_utf8(&snapshot[..end]).unwrap();
        // A complete malformed record still refuses; framing is not recovery.
        assert!(super::super::native_timing::parse(text).is_err());
        assert_eq!(complete_serial_end(b"unfinished"), 0);
        assert_eq!(complete_serial_end(complete), complete.len());
    }

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
        let (release_server, await_release) = std::sync::mpsc::sync_channel(0);
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
            await_release.recv().unwrap();
        });
        let mut reader = super::super::qmp::Reader::new(client.try_clone().unwrap());
        let failure = artifacts
            .capture(&mut client, &mut reader, "uniform", false)
            .unwrap_err();
        release_server.send(()).unwrap();
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
    #[test]
    fn capture_plan_is_finite_and_cannot_expand_after_capture() {
        let directory =
            std::env::temp_dir().join(format!("conduit-qemu-admission-{}", std::process::id()));
        let new =
            || Artifacts::new(directory.clone(), directory.join("serial.log"), json!({})).unwrap();
        assert!(new().with_capture_limit(0).is_err());
        assert!(new().with_capture_limit(33).is_err());
        let mut admitted = new().with_capture_limit(32).unwrap();
        assert_eq!(
            admitted.context["capture_contract"]["maximum_checkpoints"],
            32
        );
        assert!(admitted.entries.capacity() >= 32);
        for _ in 0..32 {
            admitted.entries.push(json!({"fixture": true}));
        }
        let (mut client, _server) = UnixStream::pair().unwrap();
        let mut reader = super::super::qmp::Reader::new(client.try_clone().unwrap());
        // Refuse before reading serial or sending another QMP request.
        assert_eq!(
            admitted
                .capture(&mut client, &mut reader, "overflow", false)
                .unwrap_err()
                .reason,
            "qemu-display-checkpoint-bound"
        );
        assert!(admitted.with_capture_limit(32).is_err());
        fs::remove_file(directory.join("manifest.json")).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
