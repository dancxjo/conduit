//! Existing native Host inventory/health observation, never pool selection.
use conduit_core::{PoolRealizationEnvelope, PoolRealizationObservation};
use conduit_std_host::StdHost;

pub(super) fn observe(
    host: &StdHost,
    realization: &PoolRealizationEnvelope,
    command: usize,
) -> Result<PoolRealizationObservation, String> {
    if realization.resources.len() > 32 || command >= 64 {
        return Err("pool observation storage or command bound refused".into());
    }
    // Keep health/resource Signs distinct from the sampler's 1..=64 sequence.
    // These are producing identities; they create no capacity or authorization.
    let owner = host.advertisement();
    let first = 1024 + (command as u64) * 33;
    let sign =
        |sequence| conduit_core::bind_sign(&owner.host_id, &owner.boot_id, None, sequence).sign_id;
    let resources: Vec<_> = (0..realization.resources.len())
        .map(|index| sign(first + 1 + index as u64))
        .collect();
    host.observe_local_model_pool_realization(realization, sign(first), &resources)
}
