//! Trusted local boot Root composition; portable Source remains inert input.
use super::{MAXIMUM_REQUEST_BYTES, ProtocolBootRequest, ROOT_MODULE_COMMAND};
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
    let request = boot::named_module(ROOT_MODULE_COMMAND, MAXIMUM_REQUEST_BYTES)
        .map_err(|_| "protocol-root-module-refused")?;
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
        ProtocolBootRequest::decode(request).map_err(|_| "protocol-root-request-refused")?;
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
    let required = crate::make::IMPL_I2C_TRANSACTION | crate::make::IMPL_MONOTONIC_DEADLINE;
    if crate::make::EMBEDDED_MAKE.implementations & required != required {
        return Err("protocol-native-implementations-not-compiled");
    }
    arch::initialize_machine(record, boot::executable_physical_address);
    let identities = crate::identity::derive(
        arch::boot_entropy(record.timestamp, record.image_physical_start),
        record.timestamp,
        record.image_physical_start,
    );
    let host = HostId::from(crate::identity::hex(&identities.host));
    let boot = BootId::from(crate::identity::hex(&identities.boot));
    // SAFETY: the caller independently established the documented native Root
    // prerequisites. Admission permanently claims actual resources for this Boot;
    // Source checking and planning neither confer nor broaden their authority.
    let owners = unsafe {
        super::owners::admit(&request, &host, &boot, crate::make::EMBEDDED_MAKE.build_id)
    }?;
    let (mut play, mut outputs) = super::runtime::prepare(entry, &request, host, boot, owners)?;
    match super::runtime::execute(&mut play, &mut outputs, &request) {
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
        core::hint::black_box(&play);
        core::hint::spin_loop();
    }
}
