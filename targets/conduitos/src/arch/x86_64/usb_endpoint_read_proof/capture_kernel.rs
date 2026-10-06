//! Mechanical kernel admission and servicing shared by native capture appliances.
use super::capture_preparation::PreparedNativeCapture;
use super::*;
use conduit_composite::AdmittedKernelCompositeHostRequest;
use conduit_kernel::NodeId;
use conduit_kernel::scheduler::RemoteIngressOutcome;

pub(super) fn admit_unit<const N: usize>(
    capture: &mut PreparedNativeCapture<'_, N>,
    port: &PortId,
    sequence: u64,
    unit: &ValuePayload,
    close: bool,
) -> Result<(), &'static str> {
    if !matches!(
        capture
            .play
            .kernel_mut()
            .admit_input(port, sequence, unit)
            .map_err(|_| "usb-hid-capture-input")?,
        RemoteIngressOutcome::Accepted { .. }
    ) {
        return Err("usb-hid-capture-input-pressure");
    }
    if close {
        capture
            .play
            .kernel_mut()
            .close_input(port)
            .map_err(|_| "usb-hid-capture-close")?;
    }
    Ok(())
}

pub(super) fn publish_call<const N: usize>(
    capture: &mut PreparedNativeCapture<'_, N>,
) -> Result<(NodeId, AdmittedKernelCompositeHostRequest), &'static str> {
    for _ in 0..8192 {
        capture
            .play
            .kernel_mut()
            .step()
            .map_err(|_| "usb-hid-capture-step")?;
        if let Some(request) = capture.play.kernel_mut().next_host_request()
            && !capture
                .play
                .service_pure_call(&request)
                .map_err(|_| "usb-hid-capture-pure")?
        {
            let kernel = capture.play.kernel_mut();
            let obligation = kernel
                .host_request_obligation(&request)
                .map_err(|_| "usb-hid-capture-obligation")?;
            let admitted = kernel
                .admit_host_request(
                    &request,
                    &obligation.host,
                    &obligation.resources,
                    &obligation.authorities,
                )
                .map_err(|_| "usb-hid-capture-admission")?;
            let call = *kernel
                .admitted_host_request_view(&admitted)
                .map_err(|_| "usb-hid-capture-call")?
                .request;
            capture
                .owner
                .begin(
                    call.node,
                    call.call,
                    call.request,
                    kernel
                        .host_request_input(&admitted)
                        .map_err(|_| "usb-hid-capture-call-input")?,
                )
                .map_err(|_| "usb-hid-capture-submit")?;
            return Ok((call.node, admitted));
        }
    }
    Err("usb-hid-capture-call-budget")
}

pub(super) fn step_pure<const N: usize>(
    capture: &mut PreparedNativeCapture<'_, N>,
) -> Result<KernelCompositeStatus, &'static str> {
    let status = capture
        .play
        .kernel_mut()
        .step()
        .map_err(|_| "usb-hid-capture-step")?;
    if let Some(request) = capture.play.kernel_mut().next_host_request()
        && !capture
            .play
            .service_pure_call(&request)
            .map_err(|_| "usb-hid-capture-pure")?
    {
        return Err("usb-hid-capture-unexpected-call");
    }
    Ok(status)
}
