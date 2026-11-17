//! Explicit selection for one complete Body workset on one admitted member.
//! The descriptor is private carrier input, never authored Source or authority.
use super::{debug, Owner};
use crate::durable_host::owner::MemberPresentationBasis;
use conduit_body::{BodyPlotPlan, BodyState, RunningHostRendezvousDescriptor};
use conduit_core::{
    AuthorityGrant, BaseImplementationId, HostAdvertisement, ProtectedResourceGrant,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read, path::Path};

#[path = "startup_member/carrier.rs"]
mod carrier;
#[path = "startup_member/execution.rs"]
mod execution;
pub(super) use execution::MemberWorker;
#[path = "startup_member/identity.rs"]
mod identity;
#[path = "startup_member/terminal_retention.rs"]
mod terminal_retention;

const MAXIMUM_SELECTION_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StartupMemberSelection {
    pub(crate) member: MemberPresentationBasis,
    pub(crate) rendezvous: RunningHostRendezvousDescriptor,
    pub(crate) candidate_id: String,
    pub(crate) authority_grants: Vec<AuthorityGrant>,
    pub(crate) protected_resource_grants: Vec<ProtectedResourceGrant>,
}

impl StartupMemberSelection {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path)
            .map_err(|_| "startup member selection could not be opened")?;
        let mut bytes = Vec::new();
        if file
            .take(MAXIMUM_SELECTION_BYTES + 1)
            .read_to_end(&mut bytes)
            .is_err()
        {
            bytes.fill(0);
            return Err("startup member selection could not be read".into());
        }
        if bytes.len() as u64 > MAXIMUM_SELECTION_BYTES {
            bytes.fill(0);
            return Err("startup member selection exceeds its finite input bound".into());
        }
        let selection = serde_json::from_slice(&bytes)
            .map_err(|_| "startup member selection is malformed".into());
        bytes.fill(0);
        selection
    }

    pub(crate) fn validate(
        &self,
        actual: &HostAdvertisement,
        now_millis: u64,
    ) -> Result<(), String> {
        self.member.validate()?;
        if &self.member.host_advertisement != actual {
            return Err("startup member current offer changed".into());
        }
        self.rendezvous
            .validate(now_millis)
            .map_err(|_| "startup member rendezvous is invalid or expired")?;
        let mut selected = self
            .rendezvous
            .candidates
            .iter()
            .filter(|candidate| candidate.candidate_id == self.candidate_id);
        let candidate = selected
            .next()
            .ok_or("startup member selected candidate is absent")?;
        if selected.next().is_some()
            || candidate.line_family != conduit_body::RendezvousLineFamily::AuthenticatedTlsStream
            || candidate.maximum_attempts != 1
            || self
                .authority_grants
                .iter()
                .any(|grant| grant.host_id != actual.host_id || grant.boot_id != actual.boot_id)
            || self
                .protected_resource_grants
                .iter()
                .any(|grant| grant.host_id != actual.host_id || grant.boot_id != actual.boot_id)
        {
            return Err("startup member selection is not one exact authenticated attempt".into());
        }
        Ok(())
    }
}

impl Owner {
    /// Recheck original owner custody before planning from actual current offers.
    /// This selects every retained Plot, or returns without publishing a proposal.
    pub(super) fn plan_startup_member_workset(
        &mut self,
        sources: &[(
            &crate::plot_source::CanonicalSource,
            &conduit_plot::ExpandedAuthoringPlot,
        )],
        selection: &StartupMemberSelection,
        actual: &HostAdvertisement,
        now_millis: u64,
    ) -> Result<(), String> {
        self.require_workset_recovered()?;
        self.require_current_native_member(&selection.member)?;
        selection.validate(actual, now_millis)?;
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.session.evidence().body.state != BodyState::Lulled
            || actual.host_id == self.host.advertisement().host_id
        {
            return Err("startup member planning requires the idle original owner".into());
        }
        let supplied = conduit_body::BodyWorkset::from_plots(sources.iter().map(|(_, plot)| {
            conduit_body::ResidentPlot::new(
                plot.expanded.source_document_id.clone(),
                plot.expanded.checked_plot_id.clone(),
            )
        }))
        .map_err(debug)?;
        if supplied != self.session.evidence().body.workset {
            return Err("startup member Sources must cover the complete retained workset".into());
        }
        let hosts = std::slice::from_ref(actual);
        let mut partitions = Vec::with_capacity(sources.len());
        for (source, expected) in sources {
            let checked = source.expand_named_for_authoring(&expected.expanded.name)?;
            if &checked != *expected || !checked.expanded.activations.is_empty() {
                return Err("startup member requires exact ordinary checked Source".into());
            }
            let placements = conduit_planner::default_expanded_placements(&checked.expanded, hosts)
                .map_err(debug)?;
            let plan = conduit_planner::plan_expanded_authoring_with_options(
                &checked,
                hosts,
                &placements,
                &[BaseImplementationId::from("conduit.base/local@1")],
                conduit_planner::PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
                    connection_byte_capacity: conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
                    authority_grants: &selection.authority_grants,
                    protected_resource_grants: &selection.protected_resource_grants,
                    line_offers: &[],
                },
                &BTreeMap::new(),
            )
            .map_err(debug)?;
            if plan.fragments.len() != 1 || plan.fragments[0].host_id != actual.host_id {
                return Err("complete startup workset must realize on the selected member".into());
            }
            partitions.push(BodyPlotPlan {
                plot: conduit_body::ResidentPlot::new(
                    checked.expanded.source_document_id.clone(),
                    checked.expanded.checked_plot_id.clone(),
                ),
                plan,
            });
        }
        let owner = self.host.advertisement();
        self.session
            .propose(partitions, &owner.host_id, &owner.boot_id)
            .map_err(debug)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "startup_member/tests.rs"]
mod tests;
