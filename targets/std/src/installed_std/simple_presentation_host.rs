//! Existing text/scalar Host presentation effects shared by execution paths.
//! This module has no scheduler, Plan state, or portable-value policy.
use conduit_core::KindId;
use std::io::Write;

/// Returns false without writing when this is not an installed simple target.
pub(super) fn present<W: Write>(
    target: Option<&KindId>,
    input: &[u8],
    output: &mut W,
) -> Result<bool, String> {
    match target.map(KindId::as_str) {
        Some("presentation/stdout-text") => {
            let text = std::str::from_utf8(input)
                .map_err(|_| "text presentation input is not valid UTF-8".to_string())?;
            write!(output, "PRESENTATION-TEXT bytes={} hex=", input.len())
                .map_err(|error| error.to_string())?;
            for byte in input {
                write!(output, "{byte:02x}").map_err(|error| error.to_string())?;
            }
            writeln!(output).map_err(|error| error.to_string())?;
            writeln!(output, "{text}").map_err(|error| error.to_string())?;
        }
        Some(conduit_std_offers::TICK_PRESENTATION_TARGET) => {
            let tick = super::contract::decode_tick(input).map_err(|error| error.to_string())?;
            writeln!(output, "tick sequence={tick}").map_err(|error| error.to_string())?;
        }
        Some(conduit_std_offers::COUNT_PRESENTATION_TARGET) => {
            let count = super::count_operations::decode_count(input)?;
            writeln!(output, "count value={count}").map_err(|error| error.to_string())?;
        }
        Some(conduit_std_offers::BOOL_PRESENTATION_TARGET) => {
            let value = conduit_core::InfoBool::decode(input)
                .map_err(|error| format!("Boolean presentation input is invalid: {error:?}"))?;
            writeln!(output, "bool value={}", value.get()).map_err(|error| error.to_string())?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_output_retains_its_exact_wire_marker_and_human_line() {
        let mut output = Vec::new();
        assert!(present(
            Some(&conduit_core::kind_id("presentation/stdout-text")),
            b"CALLING",
            &mut output
        )
        .unwrap());
        assert_eq!(
            output,
            b"PRESENTATION-TEXT bytes=7 hex=43414c4c494e47\nCALLING\n"
        );
    }

    #[test]
    fn unsupported_and_malformed_inputs_never_write_success_output() {
        let mut output = Vec::new();
        assert!(!present(None, b"anything", &mut output).unwrap());
        assert!(!present(
            Some(&conduit_core::kind_id("presentation/not-installed")),
            b"anything",
            &mut output
        )
        .unwrap());
        assert!(present(
            Some(&conduit_core::kind_id("presentation/stdout-text")),
            &[0xff],
            &mut output
        )
        .is_err());
        assert!(present(
            Some(&conduit_core::kind_id(
                conduit_std_offers::TICK_PRESENTATION_TARGET
            )),
            &[0],
            &mut output
        )
        .is_err());
        assert!(output.is_empty());
    }
}
