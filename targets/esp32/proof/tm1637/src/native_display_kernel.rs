//! Finite dormant output construction from staged transfer and retained software.
//! Root must still bind native possession and complete Body start before stepping.
use alloc::vec::Vec;
use conduit_core as c;
use conduit_environment_runtime::{
    appliance::{
        assigned::ResidentSourceIdentity, assignment_transfer as t,
        software_compact::CompactOutputSoftware,
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
    software: &mut CompactOutputSoftware<'_>,
    maximum_frames: u32,
    available_bytes: usize,
) -> Result<Prepared, &'static str> {
    if profile.sign_items == 0
        || profile.sign_items > 256
        || profile.sign_bytes > 256 * core::mem::size_of::<k::KernelEvent>() as u32
    {
        return Err("display-signs");
    }
    let signs = DisplaySignBudget::derive(maximum_frames, profile.sign_items, profile.sign_bytes)
        .map_err(|_| "display-signs")?;
    let needed = requirements(profile.maximum_value_bytes, signs)?;
    if available_bytes < needed {
        return Err("display-kernel-capacity");
    }
    let values = k::HostedValueStore::new(
        12,
        profile.maximum_value_bytes,
        12 * profile.maximum_value_bytes,
    )
    .map_err(|_| "display-values")?;
    let signs = k::HostedSignLog::new_with_remote_storage(
        signs.ordinary_items(),
        signs.ordinary_bytes(),
        signs.remote_items(),
        signs.remote_bytes(),
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
pub fn requirements(
    maximum_value_bytes: u32,
    signs: DisplaySignBudget,
) -> Result<usize, &'static str> {
    if maximum_value_bytes == 0 || maximum_value_bytes > 1024 {
        return Err("display-value-extent");
    }
    let values = k::HostedValueStore::canonical_storage_bytes([maximum_value_bytes; 12])
        .map_err(|_| "display-values")?;
    let needed = values
        + signs.storage_bytes().ok_or("display-signs")?
        + core::mem::size_of::<DisplayKernel<k::HostedValueStore, k::HostedSignLog>>()
        + t::MAXIMUM_RECORDS * core::mem::size_of::<c::AssignedRecord<'_>>()
        + 8192;
    Ok(needed)
}
