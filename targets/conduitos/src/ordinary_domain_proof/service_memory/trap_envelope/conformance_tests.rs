//! Actual bounded producer and strict reader protocol; no native execution claim.
use crate::{service_trap_reader_tests as reader, service_trap_wire_tests as wire};
use std::string::String;

#[test]
fn actual_wire_twenty_rows_satisfy_reader_and_invalid_flags_refuse() {
    let mut transcript = String::new();
    let mut instance = 0;
    for row in 0..20 {
        let (mode, case, trial) = wire::position(row).unwrap();
        if mode == 0 || trial == 0 {
            instance += 1;
        }
        let (value, origin, path) = match case {
            3 => (4, 2, 0),
            4 => (0x10d, 1, 2),
            _ => (0, 0, 1),
        };
        let raw = [
            2,
            path,
            3,
            5,
            if origin == 2 { 0 } else { 7 },
            15,
            12,
            15,
            1,
            2,
            9,
            10,
            0x202,
            0x202,
        ];
        let record = wire::sample(row, instance, value, origin, raw).unwrap();
        transcript.push_str(core::str::from_utf8(record.as_bytes()).unwrap());
    }
    let accepted = reader::validate(&transcript).unwrap();
    assert_eq!(accepted["rows"].as_array().unwrap().len(), 20);
    assert_eq!(accepted["full_irq_preservation"], false);
    let bad = wire::sample(
        0,
        1,
        0,
        0,
        [2, 1, 3, 5, 7, 15, 12, 15, 1, 2, 9, 10, 0x202, 0x203],
    )
    .unwrap();
    let first_end = transcript.find('\n').unwrap() + 1;
    let mut invalid = String::from(core::str::from_utf8(bad.as_bytes()).unwrap());
    invalid.push_str(&transcript[first_end..]);
    assert!(reader::validate(&invalid).is_err());
}
