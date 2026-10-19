//! Opt-in post-drain evidence for the existing two-Host transport entrance.
use conduit_kernel::{KernelEvent, RemoteLifecycleIdentity, SignQuery, SignRetentionGap};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use conduit_std_host::InstalledRemoteFragment;

/// Owned reconstruction after execution. Its copying cost is separate from
/// kernel Play and must be included when measuring the observed entrance.
#[derive(Debug)]
pub struct RemoteKernelEvidence {
    pub identity: KernelIdentityMap,
    pub events: Vec<KernelEvent>,
    pub remote_identities: Vec<(u32, RemoteLifecycleIdentity)>,
    pub retention_gap: Option<SignRetentionGap>,
}

#[derive(Debug)]
pub struct TwoHostKernelEvidence {
    pub source: RemoteKernelEvidence,
    pub sink: RemoteKernelEvidence,
}

pub(super) fn retain(runtime: &InstalledRemoteFragment) -> RemoteKernelEvidence {
    let signs = runtime.kernel_signs();
    let events: Vec<_> = signs.events().collect();
    let remote_identities = events
        .iter()
        .filter_map(|event| {
            signs
                .remote_identity(event.sequence)
                .map(|identity| (event.sequence, identity))
        })
        .collect();
    RemoteKernelEvidence {
        identity: runtime.kernel_identity().clone(),
        events,
        remote_identities,
        retention_gap: signs.retention_gap(),
    }
}
