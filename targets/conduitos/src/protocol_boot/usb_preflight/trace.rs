//! Bounded independent discovery evidence; these records grant no authority.
use super::{DiscoveryVerdict, RetiredUsbDiscovery};
use conduit_core::{PortId, ValuePayload};
use core::fmt::Write;

fn append(bytes: &[u8]) -> Result<(), &'static str> {
    #[cfg(target_os = "none")]
    crate::arch::append_boot_diagnostic(bytes).map_err(|_| "protocol-usb-preflight-diagnostic")?;
    #[cfg(not(target_os = "none"))]
    let _ = bytes; // Deterministic tests do not access native face MMIO.
    Ok(())
}
fn hex(bytes: &[u8]) -> Result<(), &'static str> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        append(&[DIGITS[(byte >> 4) as usize], DIGITS[(byte & 15) as usize]])?;
    }
    append(b"\n")
}
pub(super) fn prepared(
    discovery: &RetiredUsbDiscovery,
    observation: &ValuePayload,
) -> Result<(), &'static str> {
    let plan = &discovery._play.kernel().definition().internal_plan;
    let mut text = crate::sign_format::FixedText::new();
    writeln!(text, "CONDUIT_USB_PREFLIGHT_PLAN source={} checked={} expanded={} plan={} class_source={} class_checked={}",
        plan.source_document_id.as_str(), plan.checked_plot_id.as_str(), plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str(), discovery.class_resident.source_document_id.as_str(), discovery.class_resident.checked_plot_id.as_str())
        .map_err(|_| "protocol-usb-preflight-diagnostic-envelope")?;
    append(text.as_bytes())?;
    append(b"CONDUIT_USB_PREFLIGHT_PACKAGE sha256=\n")?;
    hex(&discovery.package_digest)?;
    let mut text = crate::sign_format::FixedText::new();
    write!(
        text,
        "CONDUIT_USB_PREFLIGHT_ROOT bytes={} sha256=",
        discovery.root_bytes.len()
    )
    .map_err(|_| "protocol-usb-preflight-diagnostic-envelope")?;
    for byte in &discovery.root_digest {
        write!(text, "{byte:02x}").map_err(|_| "protocol-usb-preflight-diagnostic-envelope")?;
    }
    writeln!(text).map_err(|_| "protocol-usb-preflight-diagnostic-envelope")?;
    append(text.as_bytes())?;
    hex(&discovery.root_bytes)?;
    let facts = discovery.snapshot.facts();
    let mut text = crate::sign_format::FixedText::new();
    writeln!(text, "CONDUIT_USB_PREFLIGHT_ATTACHMENT segment={} bus={} device={} function={} vendor={} device_id={} bar={} port={} slot={} dma_slot={} epoch={} requested={} actual={}",
        facts[0],facts[1],facts[2],facts[3],facts[4],facts[5],facts[6],facts[7],facts[8],facts[9],facts[10],
        discovery.snapshot.requested(),discovery.snapshot.bytes().len())
        .map_err(|_| "protocol-usb-preflight-diagnostic-envelope")?;
    append(text.as_bytes())?;
    let mut text = crate::sign_format::FixedText::new();
    writeln!(
        text,
        "CONDUIT_USB_PREFLIGHT_OBSERVATION kind={} bytes={}",
        observation.value_kind.as_str(),
        observation.encoded.len()
    )
    .map_err(|_| "protocol-usb-preflight-diagnostic-envelope")?;
    append(text.as_bytes())?;
    hex(&observation.encoded)
}
pub(super) fn output(port: &PortId, value: &ValuePayload) -> Result<(), &'static str> {
    let mut text = crate::sign_format::FixedText::new();
    writeln!(
        text,
        "CONDUIT_USB_PREFLIGHT_OUTPUT port={} kind={} bytes={}",
        port.as_str(),
        value.value_kind.as_str(),
        value.encoded.len()
    )
    .map_err(|_| "protocol-usb-preflight-diagnostic-envelope")?;
    append(text.as_bytes())?;
    hex(&value.encoded)
}
pub(super) fn retired(verdict: &DiscoveryVerdict) -> Result<(), &'static str> {
    append(match verdict {
        DiscoveryVerdict::Supported => b"CONDUIT_USB_PREFLIGHT_RETIRED supported\n",
        DiscoveryVerdict::Unsupported => b"CONDUIT_USB_PREFLIGHT_RETIRED unsupported\n",
    })
}
