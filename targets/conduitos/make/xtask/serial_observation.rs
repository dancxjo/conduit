//! Bounded host observation of append-only guest serial records.
//! Timestamps describe host reads; guest execution and display scanout are separate.
use super::{observation_clock, ConduitosError};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
const MAX_SERIAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_RECORDS: usize = 32768;
const MAX_OBSERVATION_BYTES: usize = 16 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(1);

pub(super) struct SerialObservation {
    stop: Arc<AtomicBool>,
    finished_ok: AtomicBool,
    worker: Mutex<Option<JoinHandle<Result<(), ConduitosError>>>>,
}
impl SerialObservation {
    pub(super) fn start(serial: PathBuf, output: PathBuf) -> Result<Self, ConduitosError> {
        let sink = File::create(output).map_err(io_error)?;
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = thread::Builder::new()
            .name("qemu-serial-observer".into())
            .spawn(move || observe(serial, sink, signal))
            .map_err(io_error)?;
        Ok(Self {
            stop,
            finished_ok: AtomicBool::new(false),
            worker: Mutex::new(Some(worker)),
        })
    }
    pub(super) fn finish(&self) -> Result<(), ConduitosError> {
        self.stop.store(true, Ordering::Release);
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| refusal("observer lock poisoned"))?;
        match worker.take() {
            Some(worker) => {
                let result = worker
                    .join()
                    .map_err(|_| refusal("observer thread panicked"))?;
                if result.is_ok() {
                    self.finished_ok.store(true, Ordering::Release);
                }
                result
            }
            None if self.finished_ok.load(Ordering::Acquire) => Ok(()),
            None => Err(refusal("observer previously failed")),
        }
    }
}
impl Drop for SerialObservation {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
fn observe(serial: PathBuf, mut sink: File, stop: Arc<AtomicBool>) -> Result<(), ConduitosError> {
    let mut source = loop {
        match OpenOptions::new().read(true).open(&serial) {
            Ok(file) => break file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if stop.load(Ordering::Acquire) {
                    return Err(refusal("serial never appeared"));
                }
                thread::sleep(POLL);
            }
            Err(error) => return Err(io_error(error)),
        }
    };
    let mut recorder = Recorder::default();
    let mut buffer = [0u8; 65536];
    let mut last_eof_ns = None;
    loop {
        let read_started_ns = observation_clock::nanos();
        let n = source.read(&mut buffer).map_err(io_error)?;
        let read_completed_ns = observation_clock::nanos();
        if source.metadata().map_err(io_error)?.len() < recorder.received as u64 {
            return Err(refusal("serial was truncated during observation"));
        }
        if n == 0 {
            last_eof_ns = Some(read_completed_ns);
            if stop.load(Ordering::Acquire) {
                break;
            }
            thread::sleep(POLL);
            continue;
        }
        recorder.append(
            &buffer[..n],
            &mut sink,
            read_started_ns,
            read_completed_ns,
            last_eof_ns,
        )?;
    }
    recorder.write(
        &mut sink,
        json!({"kind":"finish", "serial_bytes":recorder.received,
        "complete_records":recorder.records,"incomplete_tail_bytes":recorder.pending.len(),
        "observer_clock":observation_clock::ID,"observer_timestamp_ns":observation_clock::nanos()}),
    )?;
    sink.flush().map_err(io_error)
}
#[derive(Default)]
struct Recorder {
    pending: Vec<u8>,
    received: usize,
    committed: usize,
    records: usize,
    output_bytes: usize,
}
impl Recorder {
    fn append(
        &mut self,
        bytes: &[u8],
        sink: &mut impl Write,
        start: u64,
        end: u64,
        eof: Option<u64>,
    ) -> Result<(), ConduitosError> {
        if bytes.len() > MAX_SERIAL_BYTES - self.received {
            return Err(refusal("serial exceeds 16 MiB"));
        }
        self.received += bytes.len();
        self.pending.extend_from_slice(bytes);
        let mut consumed = 0;
        while let Some(relative) = self.pending[consumed..].iter().position(|b| *b == b'\n') {
            if self.records == MAX_RECORDS {
                return Err(refusal("serial record bound exceeded"));
            }
            let next = consumed + relative + 1;
            let line = &self.pending[consumed..next];
            let length = line.len();
            let hash = format!("{:x}", Sha256::digest(line));
            self.write(
                sink,
                json!({"kind":"record","index":self.records,
                "serial_byte_start":self.committed,"serial_byte_end":self.committed+length,
                "sha256":hash,"observer_clock":observation_clock::ID,
                "read_started_ns":start,"read_completed_ns":end,"last_eof_observed_ns":eof}),
            )?;
            self.committed += length;
            self.records += 1;
            consumed = next;
        }
        self.pending.drain(..consumed);
        Ok(())
    }
    fn write(
        &mut self,
        sink: &mut impl Write,
        value: serde_json::Value,
    ) -> Result<(), ConduitosError> {
        let encoded = serde_json::to_vec(&value).map_err(|error| refusal(error.to_string()))?;
        if encoded.len() + 1 > MAX_OBSERVATION_BYTES - self.output_bytes {
            return Err(refusal("observation byte bound exceeded"));
        }
        sink.write_all(&encoded)
            .and_then(|()| sink.write_all(b"\n"))
            .map_err(io_error)?;
        self.output_bytes += encoded.len() + 1;
        Ok(())
    }
}
fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("qemu-serial-observation-refused", detail.into())
}
fn io_error(error: std::io::Error) -> ConduitosError {
    refusal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_records_keep_exact_bytes_offsets_and_observation_windows() {
        let mut r = Recorder::default();
        let mut out = Vec::new();
        r.append(b"first", &mut out, 10, 12, None).unwrap();
        assert!(out.is_empty());
        r.append(b"\n\xff\npartial", &mut out, 20, 24, Some(18))
            .unwrap();
        let rows: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|x| serde_json::from_str(x).unwrap())
            .collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(r.pending, b"partial");
        assert_eq!(rows[0]["serial_byte_start"], 0);
        assert_eq!(rows[0]["serial_byte_end"], 6);
        assert_eq!(rows[1]["serial_byte_start"], 6);
        assert_eq!(rows[1]["serial_byte_end"], 8);
        assert_eq!(
            rows[1]["sha256"],
            format!("{:x}", Sha256::digest(b"\xff\n"))
        );
        assert_eq!(rows[0]["read_completed_ns"], 24);
        assert_eq!(rows[0]["last_eof_observed_ns"], 18);
    }
    #[test]
    fn record_and_byte_limits_refuse_instead_of_silently_dropping_evidence() {
        let mut r = Recorder {
            records: MAX_RECORDS,
            ..Recorder::default()
        };
        assert!(r.append(b"x\n", &mut Vec::new(), 0, 1, None).is_err());
        let mut r = Recorder {
            received: MAX_SERIAL_BYTES,
            ..Recorder::default()
        };
        assert!(r.append(b"x", &mut Vec::new(), 0, 1, None).is_err());
        let mut r = Recorder {
            output_bytes: MAX_OBSERVATION_BYTES,
            ..Recorder::default()
        };
        assert!(r.write(&mut Vec::new(), json!({})).is_err());
    }
    #[test]
    fn finishing_drains_existing_records_and_preserves_incomplete_tail() {
        let root =
            std::env::temp_dir().join(format!("conduit-serial-finish-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let serial = root.join("serial");
        let output = root.join("observations");
        std::fs::write(&serial, b"ready\nnext\ntail").unwrap();
        let observer = SerialObservation::start(serial, output.clone()).unwrap();
        observer.finish().unwrap();
        observer.finish().unwrap();
        let rows: Vec<serde_json::Value> = std::fs::read_to_string(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["serial_byte_end"], 6);
        assert_eq!(rows[1]["serial_byte_end"], 11);
        assert_eq!(rows[2]["complete_records"], 2);
        assert_eq!(rows[2]["incomplete_tail_bytes"], 4);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn observer_failure_stays_failed_on_repeated_finish() {
        let root =
            std::env::temp_dir().join(format!("conduit-serial-failure-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let observer =
            SerialObservation::start(root.join("absent"), root.join("observations")).unwrap();
        assert!(observer.finish().is_err());
        assert!(observer.finish().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
