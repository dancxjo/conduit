//! Fresh local network planning from checked meaning and independently supplied
//! current offers/grants. Shared by host Make and constrained Root preparation.
//! No provider, authority, Host, Boot, assignment or Play is created here.
use alloc::collections::BTreeMap;
use conduit_core as c;
use conduit_network_join_call::{CheckedNetworkLowering, PreparedNetworkKernelParts};
use conduit_plan_lowering::lowering::{LoweredPlanFragment, LoweringError};

#[derive(Debug)]
pub enum NetworkPlanningRefusal {
    SourceIdentity,
    Placement(conduit_planner::PlannerError),
    Planning(conduit_planner::PlannerError),
    ForeignOrMultipleFragments,
    Lowering(LoweringError),
}

impl NetworkPlanningRefusal {
    pub fn code(&self) -> &'static str {
        use conduit_planner::PlannerError as P;
        match self {
            Self::SourceIdentity => "source-mismatch",
            Self::Placement(error) | Self::Planning(error) => match error {
                P::UnknownCapability(_) => "capability-unavailable",
                P::AuthorityGrantMissing(_) => "authority-missing",
                P::UnavailableResource(_) | P::ResourceCapacityExceeded(_) => {
                    "resource-unavailable"
                }
                P::UnavailableBaseImplementationId(_) => "base-unavailable",
                _ => "planner-refused",
            },
            Self::ForeignOrMultipleFragments => "nonlocal-fragment",
            Self::Lowering(error) => {
                let _ = error;
                "lowering-refused"
            }
        }
    }
}

pub struct PreparedNetworkSetup {
    pub(super) plan: c::Plan,
    checked: CheckedNetworkLowering,
}
impl PreparedNetworkSetup {
    /// Root may release inert construction metadata between preparations. A
    /// freshly loaded, independently trusted Source must match all three pins;
    /// identities alone establish no checking, offers, grants or possession.
    pub fn from_checked_for_identity(
        expected: &c::PlotIdentity,
        source: &conduit_plot::ExpandedCanonicalPlot,
        host: &c::HostAdvertisement,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
    ) -> Result<Self, NetworkPlanningRefusal> {
        if source.source_document_id != expected.source_document_id
            || source.checked_plot_id != expected.checked_plot_id
            || source.expanded_plot_id != expected.expanded_plot_id
        {
            return Err(NetworkPlanningRefusal::SourceIdentity);
        }
        Self::from_checked(source, host, bases, authority)
    }
    /// Plan fresh bindings from the exact linked, Make-checked task image.
    /// This retains the ordinary planner and all current-offer checks while
    /// avoiding reconstruction/validation of a complete authoring expansion.
    pub fn from_indexed_for_identity(
        expected: &c::PlotIdentity,
        source: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        host: &c::HostAdvertisement,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
    ) -> Result<Self, NetworkPlanningRefusal> {
        Self::plan_indexed_for_identity(expected, source, host, bases, authority)?.lower()
    }
    /// Produce only the current ordinary Plan, allowing offers/grants to retire
    /// before canonical lowering allocates its numeric metadata.
    pub fn plan_indexed_for_identity(
        expected: &c::PlotIdentity,
        source: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        host: &c::HostAdvertisement,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
    ) -> Result<NetworkPlanForLowering, NetworkPlanningRefusal> {
        use NetworkPlanningRefusal as E;
        // This preliminary fence needs only the three pinned identities. Both
        // indexed placement and planning below validate the complete planning
        // metadata bounds; retaining the rich provenance decoder here would
        // reconstruct unrelated construction data on constrained native Roots.
        if &source.identity().map_err(|_| E::SourceIdentity)? != expected {
            return Err(E::SourceIdentity);
        }
        let hosts = core::slice::from_ref(host);
        let placements = conduit_planner::indexed_source::default_placements(source, hosts)
            .map_err(E::Placement)?;
        let plan = conduit_planner::indexed_source::plan_with_connection_limits(
            source,
            hosts,
            &placements,
            bases,
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
                authority_grants: authority,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &BTreeMap::new(),
        )
        .map_err(E::Planning)?;
        let [fragment] = plan.fragments.as_slice() else {
            return Err(E::ForeignOrMultipleFragments);
        };
        if fragment.host_id != host.host_id || fragment.boot_id != host.boot_id {
            return Err(E::ForeignOrMultipleFragments);
        }
        Ok(NetworkPlanForLowering { plan })
    }
    pub fn from_checked(
        source: &conduit_plot::ExpandedCanonicalPlot,
        host: &c::HostAdvertisement,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
    ) -> Result<Self, NetworkPlanningRefusal> {
        use NetworkPlanningRefusal as E;
        let hosts = core::slice::from_ref(host);
        let placements =
            conduit_planner::default_expanded_placements(source, hosts).map_err(E::Placement)?;
        let plan = conduit_planner::plan_expanded_canonical_with_options(
            source,
            hosts,
            &placements,
            bases,
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
                authority_grants: authority,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(E::Planning)?;
        Self::from_plan(plan, host)
    }
    fn from_plan(
        plan: c::Plan,
        host: &c::HostAdvertisement,
    ) -> Result<Self, NetworkPlanningRefusal> {
        use NetworkPlanningRefusal as E;
        let [fragment] = plan.fragments.as_slice() else {
            return Err(E::ForeignOrMultipleFragments);
        };
        if fragment.host_id != host.host_id || fragment.boot_id != host.boot_id {
            return Err(E::ForeignOrMultipleFragments);
        }
        let checked = CheckedNetworkLowering::prepare_lowering(fragment).map_err(E::Lowering)?;
        Ok(Self { plan, checked })
    }
    pub fn plan(&self) -> &c::Plan {
        &self.plan
    }
    pub fn lowered(&self) -> &LoweredPlanFragment {
        self.checked.lowered()
    }
    pub fn kernel_parts(
        &self,
    ) -> Result<PreparedNetworkKernelParts, conduit_network_join_call::ControlKernelRefusal> {
        self.checked.kernel_parts(&self.plan.fragments[0])
    }
}
/// Opaque current Plan awaiting canonical lowering; grants no effect authority.
pub struct NetworkPlanForLowering {
    plan: c::Plan,
}
impl NetworkPlanForLowering {
    pub fn lower(self) -> Result<PreparedNetworkSetup, NetworkPlanningRefusal> {
        let checked = CheckedNetworkLowering::prepare_lowering(&self.plan.fragments[0])
            .map_err(NetworkPlanningRefusal::Lowering)?;
        Ok(PreparedNetworkSetup {
            plan: self.plan,
            checked,
        })
    }
}
