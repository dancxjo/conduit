//! Opt-in bounded diagnostic evidence; never a linguistic policy or runtime owner.
use std::{fs::OpenOptions, io::Write};
const MAXIMUM_TRACE_BYTES: u64 = 64 * 1024 * 1024;
pub fn record(entry: &str, sequence: u64, input: &[u8], output: &[u8]) {
    let Some(path) = std::env::var_os("CONDUIT_PARSER_SOURCE_TRACE") else {
        return;
    };
    // Caller creates a fresh private file. Refuse missing files rather than
    // silently creating a competing diagnostic artifact or overwriting evidence.
    let mut file = OpenOptions::new()
        .append(true)
        .open(path)
        .expect("existing fresh trace file");
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut line = serde_json::to_vec(&serde_json::json!({
        "entry": entry, "sequence": sequence,
        "input_canonical_hex": hex(input), "output_canonical_hex": hex(output)
    }))
    .unwrap();
    line.push(b'\n');
    assert!(
        file.metadata()
            .unwrap()
            .len()
            .checked_add(line.len() as u64)
            .is_some_and(|end| end <= MAXIMUM_TRACE_BYTES),
        "diagnostic trace capacity"
    );
    file.write_all(&line)
        .expect("retain complete canonical Source observation");
}
