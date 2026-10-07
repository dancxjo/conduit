//! Mechanical observer protocol tests; these do not execute a parser/model.
#[path = "common/window8_observer.rs"]
mod observer;
use serde_json::{json, Value};
use std::io::{self, Write};

#[derive(Default)]
struct Sink {
    bytes: Vec<u8>,
    flushed: Vec<usize>,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushed.push(self.bytes.len());
        Ok(())
    }
}
#[test]
fn observer_flushes_each_exact_receipt_before_the_next_boundary() {
    let mut sink = Sink::default();
    {
        let mut observer = observer::Observer::new(&mut sink);
        observer
            .emit(
                "availability",
                json!({"source_material_bytes":"00ff", "model_invocations":0}),
            )
            .unwrap();
        observer
            .emit(
                "snapshot",
                json!({"beam_bytes":"ab01", "state_proof_bytes":["0123"], "model_invocations":4}),
            )
            .unwrap();
    }
    assert_eq!(sink.flushed.len(), 2);
    assert_eq!(sink.flushed[1], sink.bytes.len());
    assert!(sink.flushed[0] < sink.flushed[1]);
    let rows = std::str::from_utf8(&sink.bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows[0]["sequence"], 0);
    assert_eq!(rows[1]["sequence"], 1);
    assert_eq!(rows[0]["receipt"]["source_material_bytes"], "00ff");
    assert_eq!(rows[1]["receipt"]["beam_bytes"], "ab01");
    assert_eq!(rows[1]["contemporaneous_browser_observation_claim"], false);
}
