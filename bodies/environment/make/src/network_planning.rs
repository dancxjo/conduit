//! Fresh local network planning from checked meaning and independently supplied
//! current offers/grants. Shared by host Make and constrained Root preparation.
//! No provider, authority, Host, Boot, assignment or Play is created here.
use alloc::collections::BTreeMap;
use conduit_core as c;
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment, LoweringError};

#[derive(Debug)]
pub enum NetworkPlanningRefusal {
    Placement(conduit_planner::PlannerError),
    Planning(conduit_planner::PlannerError),
    ForeignOrMultipleFragments,
    Lowering(LoweringError),
}

impl NetworkPlanningRefusal {
    pub fn code(&self) -> &'static str {
        use conduit_planner::PlannerError as P;
        match self {
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
    pub(super) lowered: LoweredPlanFragment,
}
impl PreparedNetworkSetup {
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
        let [fragment] = plan.fragments.as_slice() else {
            return Err(E::ForeignOrMultipleFragments);
        };
        if fragment.host_id != host.host_id || fragment.boot_id != host.boot_id {
            return Err(E::ForeignOrMultipleFragments);
        }
        let lowered = lower_plan_fragment(fragment).map_err(E::Lowering)?;
        Ok(Self { plan, lowered })
    }
    pub fn plan(&self) -> &c::Plan {
        &self.plan
    }
    pub fn lowered(&self) -> &LoweredPlanFragment {
        &self.lowered
    }
}
