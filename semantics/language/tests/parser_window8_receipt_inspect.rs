//! Read-only inspection of retained raw proposals; no stable fact authority.
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[test]
#[ignore = "explicit retained receipt path"]
fn inspect_exact_raw_beam_receipts() {
    let path = std::env::var("WINDOW8_RECEIPTS").unwrap();
    let rows: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for row in rows.as_array().unwrap() {
        let hex = row["epochs"].as_array().unwrap().last().unwrap()["beam_bytes"]
            .as_str()
            .unwrap();
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let beam = LanguageParserWindow8RawBeam::decode(&bytes).unwrap();
        for (slot, hyp) in [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ]
        .into_iter()
        .enumerate()
        {
            eprintln!(
                "{} raw slot{slot}: active={} score={} choices={:?} heads={:?}",
                row["id"],
                hyp.active(),
                hyp.score(),
                hyp.choices(),
                hyp.state().heads()
            );
        }
    }
}
