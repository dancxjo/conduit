//! Bounded Root policy projection; no bearer, issuance, reservation or effects.
use crate::native_root::NativeRoot;
use conduit_core as c;
use serde::Serialize;

#[derive(Serialize)]
struct Reply<'a> {
    event: &'static str,
    host_id: &'a c::HostId,
    boot_id: &'a c::BootId,
    offer_generation: c::OfferGeneration,
    source_identity: [u8; 32],
    count: u64,
    period_ms: u64,
    authority_grants: [c::AuthorityGrant; 2],
    capability_possession_issued: bool,
    body_installed: bool,
    native_plan_installed: bool,
}
pub fn encode(root: &NativeRoot, output: &mut [u8]) -> Result<usize, ()> {
    let grants = root.appliance_policy_grants()?;
    let (host_id, boot_id, offer_generation) = root.offer_scope();
    let (source_identity, cadence) = crate::native_software::source_facts();
    serde_json_core::to_slice(
        &Reply {
            event: "native-appliance-policy",
            host_id,
            boot_id,
            offer_generation,
            source_identity,
            count: cadence.count,
            period_ms: cadence.period_ms,
            authority_grants: grants,
            capability_possession_issued: false,
            body_installed: false,
            native_plan_installed: false,
        },
        output,
    )
    .map_err(|_| ())
}
