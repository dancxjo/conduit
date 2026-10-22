//! Exact per-entry Root CR3 write envelopes, not trap or calibrated timing.
use crate::{ordinary_domain_baseline_format::Sample, sign_format::FixedText};
use core::fmt::{self, Write};
pub(super) fn sample(row: Sample, ticks: [u64; 4]) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    writeln!(output,
        "CONDUIT_SERVICE_CR3 {{\"schema\":\"conduit.conduitos/service-cr3@1\",\"units\":\"raw-tsc\",\"calibrated\":false,\"trap_attribution\":false,\"counter_overhead_included\":true,\"overhead_subtracted\":false,\"guest_clock_authority_added\":false,\"mode\":{},\"case\":{},\"trial\":{},\"instance\":{},\"entry_control\":{},\"entry_write\":{},\"return_control\":{},\"return_write\":{}}}",
        row.mode, row.case, row.trial, row.instance, ticks[0], ticks[1], ticks[2], ticks[3])?;
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_fixed_text_keeps_all_wide_counters_and_exact_pair_identity() {
        let text = sample(
            Sample {
                mode: 1,
                case: 4,
                trial: usize::MAX,
                instance: u64::MAX,
                ..Sample::default()
            },
            [u64::MAX; 4],
        )
        .unwrap();
        let text = core::str::from_utf8(text.as_bytes()).unwrap();
        assert_eq!(text.bytes().filter(|b| *b == b'\n').count(), 1);
        let value: serde_json::Value =
            serde_json::from_str(text.strip_prefix("CONDUIT_SERVICE_CR3 ").unwrap()).unwrap();
        assert_eq!(value["instance"].as_u64(), Some(u64::MAX));
        assert_eq!(value["trial"].as_u64(), Some(usize::MAX as u64));
        for key in [
            "entry_control",
            "entry_write",
            "return_control",
            "return_write",
        ] {
            assert_eq!(value[key].as_u64(), Some(u64::MAX));
        }
        assert_eq!(value["calibrated"], false);
        assert_eq!(value["trap_attribution"], false);
        assert_eq!(value["overhead_subtracted"], false);
    }
}
