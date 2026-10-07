//! Owned append-only observation of actual execution boundaries; no replay scheduler.
use serde_json::{json, Value};
use std::io::{self, Write};
use std::time::Instant;

pub struct Observer<W: Write> {
    writer: W,
    started: Instant,
    next_sequence: u64,
}
impl<W: Write> Observer<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            started: Instant::now(),
            next_sequence: 0,
        }
    }
    /// Call only at an actual acquisition/execution boundary. Native custody remains
    /// with the producer; this serializes exact canonical receipt bytes unchanged.
    pub fn emit(&mut self, event: &str, receipt: Value) -> io::Result<()> {
        let row = json!({
            "schema": "language/window8-execution-observer@1",
            "sequence": self.next_sequence,
            "actual_elapsed_nanos": self.started.elapsed().as_nanos().to_string(),
            "actual_elapsed_ms": self.started.elapsed().as_millis().to_string(),
            "event": event,
            "receipt": receipt,
            "contemporaneous_browser_observation_claim": false
        });
        serde_json::to_writer(&mut self.writer, &row)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| io::Error::other("observer sequence overflow"))?;
        Ok(())
    }
}
