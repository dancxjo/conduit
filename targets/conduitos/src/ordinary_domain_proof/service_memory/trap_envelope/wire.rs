//! Fixed record projection. Invalid observations remain explicit evidence.
use crate::sign_format::FixedText;
use core::fmt::{self, Write};

pub(super) fn position(row: usize) -> Option<(usize, usize, usize)> {
    match row {
        0..18 => Some(((row % 6) / 3, row / 6, row % 3)),
        18..20 => Some((0, row - 15, 0)),
        _ => None,
    }
}

fn invalid(raw: &[u64; 14], value: u64, origin: u64, case: usize) -> Option<&'static str> {
    if raw[0] == 0 {
        return Some("missing");
    }
    if raw[0] != 2 {
        return Some("incomplete");
    }
    let expected = match case {
        3 => (4, 2, 0),
        4 => (0x10d, 1, 2),
        _ => (0, 0, 1),
    };
    if (value, origin, raw[1]) != expected {
        return Some("path-mismatch");
    }
    if raw[12] != raw[13] {
        return Some("flags-mismatch");
    }
    let ordered = [
        raw[8], raw[9], raw[2], raw[3], raw[10], raw[11], raw[6], raw[7],
    ];
    if ordered.windows(2).any(|pair| pair[1] < pair[0])
        || raw[5] != raw[7]
        || (origin != 2 && (raw[4] < raw[3] || raw[4] > raw[10]))
        || (origin == 2 && raw[4] != 0)
    {
        return Some("clock-regressed");
    }
    None
}

fn nullable(output: &mut FixedText, ticks: Option<u64>) -> fmt::Result {
    match ticks {
        Some(ticks) => write!(output, "{ticks}"),
        None => output.write_str("null"),
    }
}

pub(super) fn sample(
    row: usize,
    instance: u64,
    value: u64,
    origin: u64,
    raw: [u64; 14],
) -> Result<FixedText, fmt::Error> {
    let (mode, case, trial) = position(row).ok_or(fmt::Error)?;
    let invalid = invalid(&raw, value, origin, case);
    let mut output = FixedText::new();
    write!(
        output,
        "CONDUIT_SERVICE_TRAP_ENVELOPE {{\"schema\":\"conduit.conduitos/service-trap-envelope@1\",\"units\":\"raw-tsc\",\"calibrated\":false,\"counter_overhead_included\":true,\"overhead_subtracted\":false,\"pure_hardware_trap\":false,\"full_irq_preservation\":false,\"row\":{row},\"mode\":{mode},\"case\":{case},\"trial\":{trial},\"instance\":{instance},\"value\":{value},\"origin\":{origin},\"root_flags_preserved\":{},\"valid\":{},\"invalid\":",
        raw[0] == 2 && raw[12] == raw[13],
        invalid.is_none()
    )?;
    match invalid {
        Some(reason) => write!(output, "\"{reason}\""),
        None => output.write_str("null"),
    }?;
    for (name, start, end, present) in [
        ("entry_state_ticks", 2, 3, true),
        ("terminal_return_ticks", 4, 5, origin != 2),
        ("root_restore_ticks", 6, 7, true),
        ("entry_control_ticks", 8, 9, true),
        ("return_control_ticks", 10, 11, true),
    ] {
        write!(output, ",\"{name}\":")?;
        nullable(
            &mut output,
            if invalid.is_none() && present {
                raw[end].checked_sub(raw[start])
            } else {
                None
            },
        )?;
    }
    output.write_str("}\n")?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn raw(origin: u64) -> [u64; 14] {
        [
            2,
            match origin {
                0 => 1,
                1 => 2,
                _ => 0,
            },
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
        ]
    }
    fn json(row: usize, value: u64, origin: u64, raw: [u64; 14]) -> serde_json::Value {
        let sign = sample(row, u64::MAX, value, origin, raw).unwrap();
        let text = core::str::from_utf8(sign.as_bytes()).unwrap();
        assert_eq!(text.bytes().filter(|byte| *byte == b'\n').count(), 1);
        serde_json::from_str(text.strip_prefix("CONDUIT_SERVICE_TRAP_ENVELOPE ").unwrap()).unwrap()
    }
    #[test]
    fn finite_matrix_and_actual_paths_preserve_explicit_preempt_absence() {
        for row in 0..20 {
            let (_, case, _) = position(row).unwrap();
            let (value, origin) = match case {
                3 => (4, 2),
                4 => (0x10d, 1),
                _ => (0, 0),
            };
            let record = json(row, value, origin, raw(origin));
            assert_eq!(record["valid"], true);
            assert_eq!(record["instance"].as_u64(), Some(u64::MAX));
            assert_eq!(record["root_flags_preserved"], true);
            assert_eq!(record["terminal_return_ticks"].is_null(), origin == 2);
            assert_eq!(record["root_restore_ticks"], 3);
        }
        assert!(position(20).is_none());
        assert!(sample(usize::MAX, 0, 0, 0, raw(0)).is_err());
    }
    #[test]
    fn full_width_intervals_and_wrapping_clock_do_not_saturate_or_fabricate_zero() {
        let mut data = [0; 14];
        data[0] = 2;
        data[1] = 1;
        data[5] = u64::MAX;
        data[7] = u64::MAX;
        data[12] = 2;
        data[13] = 2;
        let record = json(0, 0, 0, data);
        assert_eq!(record["root_restore_ticks"].as_u64(), Some(u64::MAX));
        assert_eq!(record["terminal_return_ticks"].as_u64(), Some(u64::MAX));
        data[6] = u64::MAX;
        data[7] = 0;
        data[5] = 0;
        let record = json(0, 0, 0, data);
        assert_eq!(record["invalid"], "clock-regressed");
        assert!(record["root_restore_ticks"].is_null());
    }
    #[test]
    fn absent_partial_reversed_flags_and_path_changes_remain_invalid_null_evidence() {
        for (index, replacement, expected) in [
            (0, 0, "missing"),
            (0, 1, "incomplete"),
            (3, 2, "clock-regressed"),
            (13, 0x203, "flags-mismatch"),
            (1, 2, "path-mismatch"),
        ] {
            let mut data = raw(0);
            data[index] = replacement;
            let record = json(0, 0, 0, data);
            assert_eq!(record["valid"], false);
            assert_eq!(record["invalid"], expected);
            for field in [
                "entry_state_ticks",
                "terminal_return_ticks",
                "root_restore_ticks",
                "entry_control_ticks",
                "return_control_ticks",
            ] {
                assert!(record[field].is_null());
            }
        }
    }
}
