//! Finite dormant output construction from staged transfer and retained software.
//! Root must still bind native possession and complete Body start before stepping.
use alloc::vec::Vec;
use conduit_core as c;
use conduit_environment_runtime::{
    appliance::{
        assigned::ResidentSourceIdentity, assignment_transfer as t, software::ReadySoftwareBacks,
    },
    display_output::*,
};
use conduit_kernel as k;
pub type Prepared = PreparedAssignedDisplayKernel<k::HostedValueStore, k::HostedSignLog>;
pub fn prepare(
    wire: &[u8],
    profile: &AssignedDisplayProfile<'_>,
    resident: &[c::AssignedRecord<'_>],
    scope: AssignedDisplayScope<'_>,
    software: &mut ReadySoftwareBacks,
    maximum_frames: u32,
    available_bytes: usize,
) -> Result<Prepared, &'static str> {
    if profile.sign_items == 0
        || profile.sign_items > 256
        || profile.sign_bytes > 256 * core::mem::size_of::<k::KernelEvent>() as u32
    {
        return Err("display-signs");
    }
    let slots = 256u16;
    let sign_bytes = 256 * core::mem::size_of::<k::KernelEvent>() as u32;
    let needed = requirements()?;
    if available_bytes < needed {
        return Err("display-kernel-capacity");
    }
    let values = k::HostedValueStore::new(12, 1024, 12 * 1024).map_err(|_| "display-values")?;
    let signs = k::HostedSignLog::new_with_remote_storage(
        slots,
        sign_bytes,
        256,
        k::remote_sign_storage_bytes(256).ok_or("display-signs")?,
    )
    .map_err(|_| "display-signs")?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(t::MAXIMUM_RECORDS)
        .map_err(|_| "display-records")?;
    records.resize(
        t::MAXIMUM_RECORDS,
        c::AssignedRecord {
            tag: 0,
            payload: &[],
        },
    );
    let decoded = t::decode(
        wire,
        ResidentSourceIdentity::from_expanded(&profile.source.expanded_plot_id),
        resident,
        &mut records,
    )
    .map_err(|_| "display-transfer")?;
    Prepared::prepare(
        AssignedDisplayInput {
            header: decoded.header,
            records: decoded.records,
            maxima: c::AssignedPlanMaxima {
                encoded_bytes: 16 * 1024,
                runtime_state_bytes: 32 * 1024,
                counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
            },
        },
        profile,
        scope,
        software,
        maximum_frames,
        values,
        signs,
    )
    .map_err(|_| "display-kernel")
}

/// Additional backing and control headroom; already retained Source/software,
/// staged wire and tables remain accounted in Root's current heap usage.
pub fn requirements() -> Result<usize, &'static str> {
    let slots = 256u16;
    let values =
        k::HostedValueStore::canonical_storage_bytes([1024; 12]).map_err(|_| "display-values")?;
    let needed = values
        + k::HostedSignLog::canonical_storage_bytes(slots, 256).ok_or("display-signs")?
        + core::mem::size_of::<DisplayKernel<k::HostedValueStore, k::HostedSignLog>>()
        + t::MAXIMUM_RECORDS * core::mem::size_of::<c::AssignedRecord<'_>>()
        + 8192;
    Ok(needed)
}
