//! Existing text/scalar Host face effects shared by execution paths.
//! This module has no scheduler, Plan state, or portable-value policy.
use conduit_core::KindId;
use std::io::Write;

/// Returns false without writing when this is not an installed simple target.
#[cfg(test)]
pub(super) fn present<W: Write>(
    target: Option<&KindId>,
    input: &[u8],
    output: &mut W,
) -> Result<bool, String> {
    present_named(target, input, output, None)
}

/// Render the declared data name with each value; preparation admits this finite Text.
pub(super) fn present_named<W: Write>(
    target: Option<&KindId>,
    input: &[u8],
    output: &mut W,
    label: Option<&str>,
) -> Result<bool, String> {
    match target.map(KindId::as_str) {
        Some("face/stdout-text") => {
            let text = std::str::from_utf8(input)
                .map_err(|_| "text face input is not valid UTF-8".to_string())?;
            write!(output, "FACE-TEXT bytes={} hex=", input.len())
                .map_err(|error| error.to_string())?;
            for byte in input {
                write!(output, "{byte:02x}").map_err(|error| error.to_string())?;
            }
            writeln!(output).map_err(|error| error.to_string())?;
            write_label(output, label)?;
            writeln!(output, "{text}").map_err(|error| error.to_string())?;
        }
        Some(conduit_std_offers::TICK_FACE_TARGET) => {
            let tick = super::contract::decode_tick(input).map_err(|error| error.to_string())?;
            write_label(output, label)?;
            writeln!(output, "tick sequence={tick}").map_err(|error| error.to_string())?;
        }
        Some(conduit_std_offers::COUNT_FACE_TARGET) => {
            let count = super::count_backs::decode_count(input)?;
            write_label(output, label)?;
            writeln!(output, "count value={count}").map_err(|error| error.to_string())?;
        }
        Some(conduit_std_offers::BOOL_FACE_TARGET) => {
            conduit_core::InfoBool::decode(input)
                .map_err(|error| format!("Boolean Face input is invalid: {error:?}"))?;
            write_label(output, label)?;
            super::bool_face::present_stdout(output, input)?;
        }
        Some(conduit_std_offers::INDICATOR_FACE_TARGET) => {
            let pattern = conduit_text::MorsePattern::decode(input)
                .map_err(|error| format!("decode indicator Morse pattern: {error:?}"))?;
            write_label(output, label)?;
            writeln!(
                output,
                "indicator unit-ms={} segments={}",
                pattern.unit_millis,
                pattern.segments.len()
            )
            .map_err(|error| error.to_string())?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn write_label(output: &mut impl Write, label: Option<&str>) -> Result<(), String> {
    if let Some(label) = label {
        write!(output, "{label}: ").map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_output_retains_its_exact_wire_marker_and_human_line() {
        let mut output = Vec::new();
        assert!(present(
            Some(&conduit_core::kind_id("face/stdout-text")),
            b"CALLING",
            &mut output
        )
        .unwrap());
        assert_eq!(output, b"FACE-TEXT bytes=7 hex=43414c4c494e47\nCALLING\n");
    }

    #[test]
    fn declared_name_is_shown_with_each_bounded_text_value() {
        let mut output = Vec::new();
        for text in [b"warm".as_slice(), b"cool".as_slice()] {
            assert!(present_named(
                Some(&conduit_core::kind_id("face/stdout-text")),
                text,
                &mut output,
                Some("Room condition")
            )
            .unwrap());
        }
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Room condition: warm"));
        assert!(output.contains("Room condition: cool"));
    }

    #[test]
    fn unsupported_and_malformed_inputs_never_write_success_output() {
        let mut output = Vec::new();
        assert!(!present(None, b"anything", &mut output).unwrap());
        assert!(!present(
            Some(&conduit_core::kind_id("face/not-installed")),
            b"anything",
            &mut output
        )
        .unwrap());
        assert!(present(
            Some(&conduit_core::kind_id("face/stdout-text")),
            &[0xff],
            &mut output
        )
        .is_err());
        assert!(present(
            Some(&conduit_core::kind_id(conduit_std_offers::TICK_FACE_TARGET)),
            &[0],
            &mut output
        )
        .is_err());
        assert!(output.is_empty());
    }
}
