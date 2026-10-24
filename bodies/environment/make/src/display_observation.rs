//! Observe preparation facts only. Caller owns channel authentication/currentness;
//! an accepted descriptor or projected grant creates no local native possession.
use crate::{DisplayOutputSource, NativeOfferRefusal as E};
use conduit_core as c;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    event: String,
    host_id: c::HostId,
    boot_id: c::BootId,
    offer_generation: c::OfferGeneration,
    source_document_id: c::SourceDocumentId,
    checked_plot_id: c::CheckedPlotId,
    expanded_plot_id: c::ExpandedPlotId,
    capabilities_bytes: usize,
    authority_grants: [c::AuthorityGrant; 1],
    capability_possession_issued: bool,
    body_installed: bool,
    native_plan_installed: bool,
    capabilities: Vec<c::CapabilityOffer>,
}
impl DisplayOutputSource {
    pub fn observe_native_offers(
        &self,
        before: &c::HostAdvertisement,
        bytes: &[u8],
        after: &c::HostAdvertisement,
    ) -> Result<(c::HostAdvertisement, [c::AuthorityGrant; 1]), E> {
        if bytes.len() > 64 * 1024 {
            return Err(E::Bounds);
        }
        let reply: Reply = serde_json::from_slice(bytes).map_err(|_| E::Encoding)?;
        if before.protocol_version != c::PROTOCOL_VERSION
            || reply.event != "native-display-software-offer"
            || reply.host_id != before.host_id
            || reply.boot_id != before.boot_id
            || reply.offer_generation != before.offer_generation
        {
            return Err(E::Scope);
        }
        if before != after {
            return Err(E::NativeChanged);
        }
        if reply.source_document_id != self.plot.expanded.source_document_id
            || reply.checked_plot_id != self.plot.expanded.checked_plot_id
            || reply.expanded_plot_id != self.plot.expanded.expanded_plot_id
        {
            return Err(E::Source);
        }
        if reply.body_installed || reply.native_plan_installed {
            return Err(E::Installed);
        }
        if reply.capability_possession_issued {
            return Err(E::Authority);
        }
        // Temporary local owners independently validate the checked constructor.
        // They are not the remote owners and establish no remote readiness.
        let mut expected = before.clone();
        expected.capabilities.clear();
        self.prepare_software(before.host_id.clone(), before.boot_id.clone())
            .map_err(|_| E::Software)?
            .append_to_advertisement(&mut expected)
            .map_err(|_| E::Software)?;
        if reply.capabilities != expected.capabilities
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
        use conduit_tm1637_wire::{installation::*, WRITE_CALL, WRITE_KIND};
        let grant = &reply.authority_grants[0];
        if grant.grant_id.as_str().is_empty()
            || grant.host_id != before.host_id
            || grant.boot_id != before.boot_id
            || grant.contract_id.as_str() != WRITE_AUTHORITY
            || grant.host_call_contract_id.as_str() != WRITE_CALL
            || grant.subject_kind.as_str() != WRITE_KIND
            || grant.capability_id.as_str() != WRITE_CAPABILITY
        {
            return Err(E::Authority);
        }
        let matching: Vec<_> = before
            .capabilities
            .iter()
            .filter(|offer| offer.capability_id == grant.capability_id)
            .collect();
        let [offer] = matching.as_slice() else {
            return Err(E::Authority);
        };
        if offer.kind_id.as_str() != WRITE_KIND
            || before
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
        let mut observed = before.clone();
        observed.capabilities.extend(reply.capabilities);
        Ok((observed, reply.authority_grants))
    }
}
