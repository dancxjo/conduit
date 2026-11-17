//! Back-specific preparation owners layered on the same generic lowering.
use super::*;
pub(super) fn backing_bytes(
    gear: &conduit_core::PlannedGear,
    budget: &super::super::super::factory::BackBudget,
    selected: Option<&crate::hosted_audio::HostedPlaybackSelection>,
) -> Result<usize, String> {
    match gear.implementation_id.as_str() {
        conduit_std_offers::TICK_IMPLEMENTATION => array::<ValueRef>(budget.host_requests)?
            .checked_mul(2)
            .ok_or_else(|| "Tick backing overflow".into()),
        conduit_std_offers::BODY_WAKE_IMPLEMENTATION => {
            super::super::super::startup_source_back::StartupGate::backing_bytes()
                .and_then(|n| {
                    n.checked_add(size_of::<
                        super::super::super::startup_source_back::StartupGate,
                    >())
                })
                .ok_or_else(|| "startup gate backing overflow".into())
        }
        conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION => {
            let selected = selected.ok_or("startup chime has no selected speaker")?;
            let mut bytes =
                crate::hosted_audio::startup_chime::layout::preparation_bytes(selected)?;
            add(
                &mut bytes,
                super::super::super::startup_chime_back::PCM_BYTES,
            )?;
            add(
                &mut bytes,
                size_of::<(
                    conduit_kernel::NodeId,
                    crate::hosted_audio::startup_chime::PreparedChimeOutput,
                )>(),
            )?;
            add(
                &mut bytes,
                size_of::<crate::body_execution::StartupChimeCallReceipt>(),
            )?;
            Ok(bytes)
        }
        _ => Err("selected IMAGE receipt has no owned accounting for this Back".into()),
    }
}
