//! Shared actual-kernel memory measurement and original IMAGE reservation.
//! Both preparation entrances use this boundary before consuming a Play sequence
//! or exposing Started. The caller owns rollback/release of returned resources.
use super::*;
use crate::kernel_preparation::{
    ImageMemoryLease, KernelResourceLedger, KernelResourceReservation,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn reserve_prepared_kernel<T: TimerAdapter>(
    kernel: &mut BodyKernel<'_>,
    timer: &mut T,
    host: &conduit_core::HostAdvertisement,
    ledger: &mut KernelResourceLedger,
    plans: &[(&conduit_core::Plan, bool)],
    lifecycle_bytes: usize,
    clock_identity_bytes: usize,
    lease: &ImageMemoryLease,
) -> Result<Vec<KernelResourceReservation>, String> {
    let receipt = kernel.prepare_memory(
        timer,
        &host.host_id,
        &host.boot_id,
        lifecycle_bytes,
        clock_identity_bytes,
    )?;
    let reservations = ledger.reserve_funded_body(host, plans, &receipt, lease)?;
    kernel.attach_image_memory(lease);
    Ok(reservations)
}
