//! Finite dormant output construction from staged transfer and retained software.
//! Root must still bind native possession and complete Body start before stepping.
pub use super::runtime::Prepared;
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
    let signs = DisplaySignBudget::derive(
        if cfg!(feature = "display-physical-check") {
            1
        } else {
            maximum_frames
        },
        profile.sign_items,
        profile.sign_bytes,
    )
    .map_err(|_| "display-signs")?;
    let needed = requirements(profile.maximum_value_bytes, signs)?;
    if available_bytes < needed {
        return Err("display-kernel-capacity");
    }
    let values = DisplayValueBudget::derive(signs.maximum_frames(), profile.maximum_value_bytes)
        .map_err(|_| "display-values")?
        .prepare()
        .map_err(|_| "display-values")?;
    #[cfg(feature = "display-physical-check")]
    let signs = crate::native_display_evidence::fresh_log().map_err(|_| "display-signs")?;
    #[cfg(not(feature = "display-physical-check"))]
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
    #[cfg(feature = "display-physical-check")]
    let prepare = PreparedAssignedDisplayKernel::prepare_for_session;
    #[cfg(not(feature = "display-physical-check"))]
    let prepare = PreparedAssignedDisplayKernel::prepare;
    prepare(
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
    .and_then(Prepared::from_assigned)
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
    let values = DisplayValueBudget::derive(signs.maximum_frames(), maximum_value_bytes)
        .and_then(DisplayValueBudget::storage_bytes)
        .map_err(|_| "display-values")?;
    #[cfg(feature = "display-physical-check")]
    let sign_backing = conduit_environment_runtime::display_evidence::MAXIMUM_BYTES;
    #[cfg(not(feature = "display-physical-check"))]
    let sign_backing = signs.storage_bytes().ok_or("display-signs")?;
    let needed = values
        + sign_backing
        + core::mem::size_of::<super::runtime::Raw>()
        + t::MAXIMUM_RECORDS * core::mem::size_of::<c::AssignedRecord<'_>>()
        + 8192;
    Ok(needed)
}
