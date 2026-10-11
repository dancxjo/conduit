//! Trusted local boot Root composition; portable Source remains inert input.
use super::{
    MAXIMUM_REQUEST_BYTES,
    usb_request::{ROOT_MODULE_COMMAND as USB_ROOT_MODULE_COMMAND, UsbProtocolBootRequest},
};
use crate::{arch, boot, protocol_source};
use conduit_core::{BootId, HostId};

/// Run a selected standalone protocol Body, retaining its biography after Lull.
///
/// # Safety
/// The caller is the sole native Root before other machine users. Its boot
/// configuration is controlled by the local administrator, who has independently
/// released firmware ownership and approved the selected electrical attachment.
/// Module presence and request flags alone cannot establish those prerequisites.
/// The runtime arena must already be initialized. This entrance never returns
/// once a protocol module is selected, preventing a second startup from replacing
/// the retained Body or reusing its native reservations.
pub unsafe fn run_if_selected(record: &boot::BootRecord) -> Result<(), &'static str> {
    let request = boot::named_module(USB_ROOT_MODULE_COMMAND, MAXIMUM_REQUEST_BYTES)
        .map_err(|_| "protocol-root-module-refused")?;
    if request.is_none() {
        return Ok(());
    }
    if boot::named_module(super::ROOT_MODULE_COMMAND, MAXIMUM_REQUEST_BYTES)
        .map_err(|_| "protocol-root-module-refused")?
        .is_some()
    {
        return Err("protocol-multiple-root-requests");
    }
    let package = boot::named_module(
        protocol_source::PROTOCOL_MODULE_COMMAND,
        protocol_source::MAXIMUM_PACKAGE_BYTES,
    )
    .map_err(|_| "protocol-source-module-refused")?;
    let (request, package) = match (request, package) {
        (None, None) => return Ok(()),
        (Some(request), Some(package)) => (request, package),
        _ => return Err("protocol-boot-module-pair-incomplete"),
    };
    let request =
        UsbProtocolBootRequest::decode(request).map_err(|_| "protocol-root-request-refused")?;
    request
        .bind_package(package)
        .map_err(|_| "protocol-source-binding-refused")?;
    let entry = protocol_source::PreparedProtocolEntry::prepare(package, &request.entry)
        .map_err(|_| "protocol-source-preparation-refused")?;
    if entry.expanded().expanded.gears.len()
        > crate::make::EMBEDDED_MAKE.operation_slot_ceiling as usize
    {
        return Err("protocol-operation-envelope-exceeded");
    }
    if !crate::make::EMBEDDED_MAKE.includes(crate::make::IMPL_USB_TRANSPORT) {
        return Err("protocol-usb-implementation-not-compiled");
    }
    #[cfg(target_arch = "x86_64")]
    arch::initialize_machine(record, boot::executable_physical_address);
    #[cfg(target_arch = "x86_64")]
    let identity_entropy = arch::boot_entropy(record.timestamp, record.image_physical_start);
    #[cfg(target_arch = "aarch64")]
    let identity_entropy = {
        let counter = arch::read_counter();
        [
            counter,
            counter.rotate_left(13),
            counter.rotate_left(29),
            counter.rotate_left(47),
        ]
    };
    let identities = crate::identity::derive(
        identity_entropy,
        record.timestamp,
        record.image_physical_start,
    );
    let host = HostId::from(crate::identity::hex(&identities.host));
    let boot = BootId::from(crate::identity::hex(&identities.boot));
    // SAFETY: the caller independently established the documented native Root
    // prerequisites. Admission permanently claims actual resources for this Boot;
    // Source checking and planning neither confer nor broaden their authority.
    let (owners, discovery) = unsafe {
        super::usb_owners::admit(
            record,
            &request,
            &host,
            &boot,
            crate::make::EMBEDDED_MAKE.build_id,
            package,
        )
    }?;
    if let Some(discovery) = discovery.as_ref() {
        discovery.validate_class(&entry)?;
    }
    let (mut play, mut outputs) = super::runtime::prepare_usb(entry, &request, host, boot, owners)?;
    match super::runtime::execute_usb(&mut play, &mut outputs, &request) {
        Ok(()) => {
            let _ = arch::append_boot_diagnostic(b"CONDUIT_PROTOCOL_RETIRED complete\n");
        }
        Err(reason) => {
            let _ = arch::append_boot_diagnostic(b"CONDUIT_PROTOCOL_REFUSAL ");
            let _ = arch::append_boot_diagnostic(reason.as_bytes());
            let _ = arch::append_boot_diagnostic(b"\n");
        }
    }
    // Keep the canonical session and quarantined owners alive. Completion of
    // this one Play does not invent an operator's conclusion or Body fulfillment.
    loop {
        core::hint::black_box((&play, &discovery));
        core::hint::spin_loop();
    }
}
