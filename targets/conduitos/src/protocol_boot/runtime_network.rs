//! One admitted containing-machine network poll per ordinary kernel step.
use super::*;
use crate::protocol_boot::usb_request::UsbProtocolBootRequest;

pub(super) struct RuntimeNetwork {
    deadline: arch::CandidateDeadline,
    complete: bool,
}
pub(super) fn prepare(
    request: &UsbProtocolBootRequest,
) -> Result<Option<RuntimeNetwork>, &'static str> {
    request
        .ethernet_network
        .as_ref()
        .map(|scope| {
            Ok(RuntimeNetwork {
                deadline: arch::CandidateDeadline::admit(scope.maximum_duration_millis)
                    .ok_or("protocol-network-clock-unavailable")?,
                complete: false,
            })
        })
        .transpose()
}
impl RuntimeNetwork {
    pub(super) fn complete(&self) -> bool {
        self.complete
    }
    pub(super) fn poll(&mut self, play: &mut NativePlay) -> Result<bool, &'static str> {
        let millis = self
            .deadline
            .elapsed_millis()
            .ok_or("protocol-network-clock-expired")?;
        if let Some(receipt) = play
            .poll_ethernet_network(smoltcp::time::Instant::from_millis(millis))
            .map_err(report_refusal)?
            && !self.complete
        {
            report(receipt)?;
            self.complete = true;
        }
        Ok(self.complete)
    }
}
pub(super) fn require_complete(network: Option<&RuntimeNetwork>) -> Result<(), &'static str> {
    if network.is_some_and(|network| !network.complete()) {
        return Err("protocol-network-not-complete");
    }
    Ok(())
}
fn report(receipt: crate::virtio_tcp::VirtioTcpReceipt) -> Result<(), &'static str> {
    use core::fmt::Write;
    let mut report = crate::sign_format::FixedText::new();
    writeln!(
        report,
        "CONDUIT_USB_NETWORK_RECEIPT polls={} transmitted={} received={}",
        receipt.polls, receipt.transmitted_bytes, receipt.received_bytes
    )
    .map_err(|_| "protocol-network-receipt-bound")?;
    arch::append_boot_diagnostic(report.as_bytes())
        .map_err(|_| "protocol-network-receipt-retention-refused")
}

fn report_refusal(reason: impl core::fmt::Debug) -> &'static str {
    use core::fmt::Write;
    let mut report = crate::sign_format::FixedText::new();
    if writeln!(report, "CONDUIT_USB_NETWORK_REFUSAL {reason:?}").is_ok() {
        let _ = arch::append_boot_diagnostic(report.as_bytes());
    }
    "protocol-network-poll-refused"
}
