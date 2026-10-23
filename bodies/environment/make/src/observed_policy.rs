//! Root policy observed over the caller's authenticated/current channel.
//! Projection is planning permission; it never creates local bearer possession.
use crate::{NativeOfferRefusal as E, NativeSoftwareSourceRecipe};
use conduit_core as c;
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    event: String,
    host_id: c::HostId,
    boot_id: c::BootId,
    offer_generation: c::OfferGeneration,
    source_identity: [u8; 32],
    count: u64,
    period_ms: u64,
    authority_grants: [c::AuthorityGrant; 2],
    capability_possession_issued: bool,
    body_installed: bool,
    native_plan_installed: bool,
}
impl NativeSoftwareSourceRecipe {
    /// Validate this Root policy against the same scoped, composed native
    /// offers and checked Source. The caller retains transport/currentness
    /// evidence. Capability issuance and Body/Play admission stay on Root.
    pub fn observe_appliance_policy(
        &self,
        host: &c::HostAdvertisement,
        bytes: &[u8],
    ) -> Result<[c::AuthorityGrant; 2], E> {
        if bytes.len() > 8192 {
            return Err(E::Bounds);
        }
        let reply: Reply = serde_json::from_slice(bytes).map_err(|_| E::Encoding)?;
        if reply.event != "native-appliance-policy"
            || reply.host_id != host.host_id
            || reply.boot_id != host.boot_id
            || reply.offer_generation != host.offer_generation
        {
            return Err(E::Scope);
        }
        if reply.source_identity != self.spec().source.expanded_plot_digest {
            return Err(E::Source);
        }
        if (reply.count, reply.period_ms) != (self.cadence().count, self.cadence().period_ms) {
            return Err(E::Cadence);
        }
        if reply.body_installed || reply.native_plan_installed || reply.capability_possession_issued
        {
            return Err(E::Installed);
        }
        use conduit_monotonic_wait::{WAIT_AUTHORITY, WAIT_CAPABILITY};
        use conduit_pulse_capture::{installation::*, PULSE_READ_CALL, PULSE_READ_KIND};
        let expected = [
            (
                PULSE_READ_KIND,
                PULSE_READ_CALL,
                PULSE_AUTHORITY,
                PULSE_CAPABILITY,
            ),
            (
                conduit_time::TICK_KIND,
                c::WAIT_HOST_CALL_CONTRACT,
                WAIT_AUTHORITY,
                WAIT_CAPABILITY,
            ),
        ];
        for (grant, (kind, call, authority, cap)) in reply.authority_grants.iter().zip(expected) {
            if grant.grant_id.as_str().is_empty()
                || grant.host_id != host.host_id
                || grant.boot_id != host.boot_id
                || grant.subject_kind.as_str() != kind
                || grant.host_call_contract_id.as_str() != call
                || grant.contract_id.as_str() != authority
                || grant.capability_id.as_str() != cap
            {
                return Err(E::Authority);
            }
            let mut offers = host
                .capabilities
                .iter()
                .filter(|offer| offer.capability_id == grant.capability_id);
            let offer = offers.next().ok_or(E::Authority)?;
            if offers.next().is_some()
                || offer.kind_id.as_str() != kind
                || host
                    .bases
                    .iter()
                    .filter(|base| base.capability_ids.contains(&grant.capability_id))
                    .count()
                    != 1
                || !offer.authority_requirements.iter().any(|required| {
                    required.contract_id == grant.contract_id
                        && required.host_call_contract_id == grant.host_call_contract_id
                        && required.subject_kind == grant.subject_kind
                })
            {
                return Err(E::Authority);
            }
        }
        if reply.authority_grants[0].grant_id == reply.authority_grants[1].grant_id {
            return Err(E::Authority);
        }
        Ok(reply.authority_grants)
    }
}
