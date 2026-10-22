//! Separate bounded staged-IPC receipt; original baseline schemas are unchanged.
use crate::sign_format::FixedText;
use core::fmt::{self, Write};
pub const LENGTHS: [usize; 5] = [0, 256, 4096, 16384, 65536];
pub const RECORDS: usize = 30;
pub const ROOT_WORKSPACE: usize = 131072;
#[derive(Clone, Copy, Default)]
pub struct Row {
    pub mode: u8,
    pub case: usize,
    pub trial: usize,
    pub length: usize,
    pub instance: u64,
    pub request: u64,
    pub stage: u64,
    pub entry: u64,
    pub returned: u64,
}
pub fn header(reserved: u32) -> Result<FixedText, fmt::Error> {
    let mut text = FixedText::new();
    writeln!(
        text,
        "CONDUIT_SERVICE_STAGED_HEADER {{\"schema\":\"conduit.conduitos/service-staged-ipc@1\",\"fixture\":true,\"privilege\":\"ring3\",\"units\":\"raw-tsc\",\"calibrated\":false,\"counter_overhead_included\":true,\"component_attribution\":false,\"records\":30,\"refusals\":8,\"lengths\":[0,256,4096,16384,65536],\"root_workspace_bytes\":131072,\"domain_reserved_bytes\":{},\"entry_work_grant\":1,\"tail_check_outside_interval\":true,\"preconditioned\":true,\"total_transaction\":false,\"guest_independent_identity_authority\":false,\"production_protocol\":false}}",
        reserved
    )?;
    Ok(text)
}
pub fn row(value: Row) -> Result<FixedText, fmt::Error> {
    let mut text = FixedText::new();
    writeln!(
        text,
        "CONDUIT_SERVICE_STAGED_SAMPLE {{\"mode\":{},\"case\":{},\"trial\":{},\"length\":{},\"instance\":{},\"request\":{},\"stage_admission_copy\":{},\"entry_and_guest\":{},\"return_copy_validation\":{},\"validated_bytes\":{},\"tail_unchanged\":true}}",
        value.mode,
        value.case,
        value.trial,
        value.length,
        value.instance,
        value.request,
        value.stage,
        value.entry,
        value.returned,
        value.length
    )?;
    Ok(text)
}
pub fn refusal(
    case: usize,
    instance: u64,
    request: u64,
    descriptor_instance: u64,
    descriptor_request: u64,
    reason: u32,
) -> Result<FixedText, fmt::Error> {
    let mut text = FixedText::new();
    writeln!(
        text,
        "CONDUIT_SERVICE_STAGED_REFUSAL {{\"case\":{},\"instance\":{},\"request\":{},\"descriptor_instance\":{},\"descriptor_request\":{},\"reason\":{},\"guest_entered\":{},\"successful_output\":false}}",
        case,
        instance,
        request,
        descriptor_instance,
        descriptor_request,
        reason,
        case == 6
    )?;
    Ok(text)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn value(text: FixedText, prefix: &str) -> serde_json::Value {
        let raw = core::str::from_utf8(text.as_bytes()).unwrap();
        assert_eq!(raw.bytes().filter(|b| *b == b'\n').count(), 1);
        serde_json::from_str(
            raw.strip_prefix(prefix)
                .unwrap()
                .strip_suffix('\n')
                .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn fixed_receipts_preserve_max_width_and_workspace_boundary() {
        let header = value(header(u32::MAX).unwrap(), "CONDUIT_SERVICE_STAGED_HEADER ");
        assert_eq!(header["root_workspace_bytes"], 131072);
        assert_eq!(header["root_workspace_bytes"], ROOT_WORKSPACE);
        assert_eq!(header["records"], RECORDS);
        assert_eq!(header["lengths"], serde_json::json!(LENGTHS));
        assert_eq!(header["domain_reserved_bytes"], u32::MAX);
        let row = value(
            row(Row {
                mode: u8::MAX,
                case: usize::MAX,
                trial: usize::MAX,
                length: usize::MAX,
                instance: u64::MAX,
                request: u64::MAX,
                stage: u64::MAX,
                entry: u64::MAX,
                returned: u64::MAX,
            })
            .unwrap(),
            "CONDUIT_SERVICE_STAGED_SAMPLE ",
        );
        for key in [
            "instance",
            "request",
            "stage_admission_copy",
            "entry_and_guest",
            "return_copy_validation",
        ] {
            assert_eq!(row[key], u64::MAX);
        }
        assert_eq!(row["validated_bytes"].as_u64(), Some(usize::MAX as u64));
        let denied = value(
            refusal(6, u64::MAX, u64::MAX, u64::MAX, u64::MAX, 4).unwrap(),
            "CONDUIT_SERVICE_STAGED_REFUSAL ",
        );
        assert_eq!(denied["guest_entered"], true);
        assert_eq!(denied["successful_output"], false);
    }
}
