//! Six register pushes/pops and user-origin Root IRQ handler envelopes only.
use crate::{ordinary_domain_baseline_format::Sample, sign_format::FixedText};
use core::fmt::{self, Write};
pub(super) fn sample(row: Sample, ticks: [u64; 9]) -> Result<FixedText, fmt::Error> {
    if ticks[4] != row.interrupts {
        return Err(fmt::Error);
    }
    let mut output = FixedText::new();
    write!(output,"CONDUIT_SERVICE_ROOT_STATE {{\"schema\":\"conduit.conduitos/service-root-state@1\",\"units\":\"raw-tsc\",\"calibrated\":false,\"registers\":6,\"flags_cost_included\":false,\"stack_switch_cost_included\":false,\"pure_trap_attribution\":false,\"counter_overhead_included\":true,\"overhead_subtracted\":false,\"irq_scope\":\"user-origin-root-handler\",\"mode\":{},\"case\":{},\"trial\":{},\"instance\":{},\"push_control\":{},\"pushes\":{},\"pop_control\":{},\"pops\":{},\"irq_count\":{},\"irq_total\":{},\"irq_control_total\":{},\"irq_min\":",
        row.mode,row.case,row.trial,row.instance,ticks[0],ticks[1],ticks[2],ticks[3],ticks[4],ticks[5],ticks[8])?;
    if ticks[4] == 0 {
        writeln!(output, "null,\"irq_max\":null}}")?;
    } else {
        writeln!(output, "{},\"irq_max\":{}}}", ticks[6], ticks[7])?;
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn json(text: &FixedText) -> serde_json::Value {
        let text = core::str::from_utf8(text.as_bytes()).unwrap();
        assert_eq!(text.bytes().filter(|b| *b == b'\n').count(), 1);
        serde_json::from_str(text.strip_prefix("CONDUIT_SERVICE_ROOT_STATE ").unwrap()).unwrap()
    }
    #[test]
    fn actual_fixed_text_preserves_wide_intervals_and_absent_irq_minimum() {
        let row = Sample {
            instance: u64::MAX,
            trial: usize::MAX,
            interrupts: 1,
            ..Sample::default()
        };
        let value = json(
            &sample(
                row,
                [
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    1,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                ],
            )
            .unwrap(),
        );
        for key in [
            "instance",
            "pushes",
            "pops",
            "irq_total",
            "irq_min",
            "irq_max",
            "irq_control_total",
        ] {
            assert_eq!(value[key].as_u64(), Some(u64::MAX));
        }
        let value = json(&sample(Sample::default(), [0; 9]).unwrap());
        assert!(value["irq_min"].is_null());
        assert!(value["irq_max"].is_null());
        assert!(sample(row, [0; 9]).is_err());
    }
}
