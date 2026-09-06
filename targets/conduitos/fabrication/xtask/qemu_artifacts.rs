//! Explicit, bounded visual evidence for one ordinary QEMU journey.
use super::{qmp_display, ConduitosError};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufReader, Read},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Instant,
};

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
        context: Value,
    ) -> Result<Self, ConduitosError> {
        fs::create_dir_all(&directory).map_err(io_error)?;
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
        reader: &mut BufReader<UnixStream>,
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
        let record = text
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("CONDUIT_PRODUCT_JOURNEY "))
            .map(serde_json::from_str::<Value>)
            .transpose()
            .map_err(|error| {
                ConduitosError::refusal("qemu-display-correlation-invalid", error.to_string())
            })?;
        let frame = qmp_display::capture(stream, reader, &self.directory, checkpoint)?;
        let pixels = frame["pixel_sha256"]
            .as_str()
            .expect("capture digest")
            .to_owned();
        let unchanged = expect_change && self.previous_pixels.as_ref() == Some(&pixels);
        self.entries.push(json!({"index":self.entries.len(),"checkpoint":checkpoint,
            "elapsed_millis":self.started.elapsed().as_millis(),"serial_byte_end":serial.len(),
            "guest_record":record,"frame":frame,"expected_change":expect_change,"unchanged":unchanged}));
        self.previous_pixels = Some(pixels);
        self.write("running", None)?;
        if unchanged {
            return Err(ConduitosError::refusal(
                "qemu-display-unchanged-frame",
                checkpoint,
            ));
        }
        Ok(())
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
