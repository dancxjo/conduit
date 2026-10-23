//! Compose bounded observations of actual Root offers for host-side planning.
//! Transport authentication/currentness is the caller's obligation. This does
//! not instantiate providers, authorize effects or establish Body membership.
use crate::NativeSoftwareSourceRecipe;
use conduit_core as c;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeOfferRefusal {
    Bounds,
    Encoding,
    Scope,
    NativeChanged,
    Source,
    Cadence,
    Installed,
    Software,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SoftwareReply {
    event: String,
    host_id: c::HostId,
    boot_id: c::BootId,
    offer_generation: c::OfferGeneration,
    source_identity: [u8; 32],
    count: u64,
    period_ms: u64,
    capabilities_bytes: usize,
    body_installed: bool,
    native_plan_installed: bool,
    capabilities: Vec<c::CapabilityOffer>,
}

impl NativeSoftwareSourceRecipe {
    /// Caller collects native offers, then software offers, then native offers
    /// again over its authenticated Root channel. Require an unchanged native
    /// snapshot and exact Source-compiled software, before composing a planning
    /// input. The returned advertisement grants no authority and creates no
    /// local Ready owner. It must not outlive the caller's currentness evidence.
    pub fn observe_native_offers(
        &self,
        before: &c::HostAdvertisement,
        software_reply: &[u8],
        after: &c::HostAdvertisement,
    ) -> Result<c::HostAdvertisement, NativeOfferRefusal> {
        use NativeOfferRefusal as E;
        if software_reply.len() > 64 * 1024 {
            return Err(E::Bounds);
        }
        let reply: SoftwareReply =
            serde_json::from_slice(software_reply).map_err(|_| E::Encoding)?;
        if before.protocol_version != c::PROTOCOL_VERSION
            || reply.event != "native-software-offer"
            || reply.host_id != before.host_id
            || reply.boot_id != before.boot_id
            || reply.offer_generation != before.offer_generation
            || after.host_id != before.host_id
            || after.boot_id != before.boot_id
            || after.offer_generation != before.offer_generation
        {
            return Err(E::Scope);
        }
        if before != after {
            return Err(E::NativeChanged);
        }
        if reply.source_identity != self.spec().source.expanded_plot_digest {
            return Err(E::Source);
        }
        if (reply.count, reply.period_ms) != (self.cadence().count, self.cadence().period_ms) {
            return Err(E::Cadence);
        }
        if reply.body_installed || reply.native_plan_installed {
            return Err(E::Installed);
        }
        let expected: Vec<c::CapabilityOffer> = self
            .offer_json()
            .map(|json| serde_json::from_slice(json).map_err(|_| E::Software))
            .collect::<Result<_, _>>()?;
        if reply.capabilities != expected
            || serde_json::to_vec(&reply.capabilities)
                .map_err(|_| E::Encoding)?
                .len()
                != reply.capabilities_bytes
            || reply.capabilities.iter().any(|software| {
                before
                    .capabilities
                    .iter()
                    .any(|native| native.capability_id == software.capability_id)
            })
        {
            return Err(E::Software);
        }
        let mut advertisement = before.clone();
        advertisement.capabilities.extend(reply.capabilities);
        Ok(advertisement)
    }
}
