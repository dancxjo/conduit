//! Bounded baseline receipts shared by the appliance and real host API tests.
use crate::sign_format::FixedText;
use core::fmt::{self, Write};

pub(crate) const TRIALS: usize = 3;
pub(crate) const RECORDS: usize = 3 * 2 * TRIALS + 2;

#[derive(Clone, Copy, Default)]
pub(crate) struct Sample {
    pub mode: u8,
    pub case: u8,
    pub trial: usize,
    pub instance: u64,
    pub setup: u64,
    pub setup_clearing: u64,
    pub request: u64,
    pub entry: u64,
    pub response: u64,
    pub teardown: u64,
    pub interrupts: u64,
    pub preemptions: u64,
}

pub(crate) fn header(
    cpu: [u32; 3],
    invariant_tsc: bool,
    overhead: [u64; TRIALS],
) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    writeln!(
        output,
        "CONDUIT_SERVICE_BASELINE {{\"schema\":\"conduit.conduitos/service-baseline@1\",\"units\":\"raw-tsc\",\"calibrated\":false,\"component_attribution\":false,\"setup_component_records\":14,\"fixture_capacity\":256,\"records\":{RECORDS},\"cpuid1_eax\":{},\"cpuid1_ecx\":{},\"cpuid1_edx\":{},\"invariant_tsc\":{},\"counter_pair_ticks\":[{},{},{}]}}",
        cpu[0], cpu[1], cpu[2], invariant_tsc, overhead[0], overhead[1], overhead[2]
    )?;
    Ok(output)
}

pub(crate) fn setup_component(row: Sample) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    writeln!(
        output,
        "CONDUIT_SERVICE_SETUP_COMPONENT {{\"schema\":\"conduit.conduitos/service-setup-component@1\",\"units\":\"raw-tsc\",\"component\":\"slot-zeroing\",\"instance\":{},\"setup\":{},\"clearing\":{},\"counter_overhead_included\":true}}",
        row.instance, row.setup, row.setup_clearing
    )?;
    Ok(output)
}

pub(crate) fn sample(row: Sample) -> Result<FixedText, fmt::Error> {
    let mut output = FixedText::new();
    writeln!(
        output,
        "CONDUIT_SERVICE_SAMPLE {{\"mode\":{},\"case\":{},\"trial\":{},\"instance\":{},\"setup\":{},\"request\":{},\"entry\":{},\"response\":{},\"teardown\":{},\"interrupts\":{},\"preemptions\":{}}}",
        row.mode,
        row.case,
        row.trial,
        row.instance,
        row.setup,
        row.request,
        row.entry,
        row.response,
        row.teardown,
        row.interrupts,
        row.preemptions
    )?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(record: &FixedText, prefix: &str) -> serde_json::Value {
        let text = core::str::from_utf8(record.as_bytes()).unwrap();
        assert_eq!(text.bytes().filter(|byte| *byte == b'\n').count(), 1);
        serde_json::from_str(
            text.strip_prefix(prefix)
                .unwrap()
                .strip_suffix('\n')
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn header_preserves_raw_counter_boundary_at_maximum_width() {
        let record = header([u32::MAX; 3], true, [u64::MAX; TRIALS]).unwrap();
        let value = json(&record, "CONDUIT_SERVICE_BASELINE ");
        assert_eq!(value["records"], 20);
        assert_eq!(value["units"], "raw-tsc");
        assert_eq!(value["calibrated"], false);
        assert_eq!(value["component_attribution"], false);
        assert_eq!(value["fixture_capacity"], 256);
        assert_eq!(value["cpuid1_eax"], u32::MAX);
        assert_eq!(value["counter_pair_ticks"][2], u64::MAX);
    }

    #[test]
    fn sample_fits_real_fixed_text_without_truncating_wide_counters() {
        let record = sample(Sample {
            mode: u8::MAX,
            case: u8::MAX,
            trial: usize::MAX,
            instance: u64::MAX,
            setup: u64::MAX,
            setup_clearing: u64::MAX,
            request: u64::MAX,
            entry: u64::MAX,
            response: u64::MAX,
            teardown: u64::MAX,
            interrupts: u64::MAX,
            preemptions: u64::MAX,
        })
        .unwrap();
        let value = json(&record, "CONDUIT_SERVICE_SAMPLE ");
        assert_eq!(value["trial"].as_u64(), Some(usize::MAX as u64));
        for field in [
            "instance",
            "setup",
            "request",
            "entry",
            "response",
            "teardown",
            "interrupts",
            "preemptions",
        ] {
            assert_eq!(value[field].as_u64(), Some(u64::MAX), "{field}");
        }
    }

    #[test]
    fn component_preserves_its_exact_instance_and_raw_overhead_boundary() {
        let record = setup_component(Sample {
            instance: u64::MAX,
            setup: u64::MAX,
            setup_clearing: u64::MAX,
            ..Sample::default()
        })
        .unwrap();
        let value = json(&record, "CONDUIT_SERVICE_SETUP_COMPONENT ");
        assert_eq!(value["instance"], u64::MAX);
        assert_eq!(value["clearing"], u64::MAX);
        assert_eq!(value["counter_overhead_included"], true);
        assert_eq!(value["units"], "raw-tsc");
        assert_eq!(value["component"], "slot-zeroing");
    }
}
