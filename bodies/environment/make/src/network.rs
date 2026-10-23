//! Authored control meaning, separate from any initialized radio, authority or
//! page submission. Checking this Source makes no network availability claim.
pub fn compile_network_setup() -> Result<conduit_plot::ExpandedCanonicalPlot, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_net::install_network_bootstrap_catalogs(&mut startup, &mut profiles)?;
    let syntax =
        conduit_plot::parse_syntax_document(include_str!("../../profiles/network.conduit"));
    if !syntax.diagnostics.is_empty() {
        return Err(format!("network setup Source: {:?}", syntax.diagnostics));
    }
    let checked = conduit_plot::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("{error:?}"))?;
    conduit_plot::expand_canonical_plot(&checked, "environment-network-setup", &profiles)
        .map_err(|error| format!("{error:?}"))
}

/// Host-side preparation of the authored control Plot against caller-supplied
/// current offers and grants. It initializes no provider and starts no Play.
pub struct PreparedNetworkSetup {
    plan: conduit_core::Plan,
    lowered: conduit_plan_lowering::lowering::LoweredPlanFragment,
}
impl PreparedNetworkSetup {
    pub fn prepare(
        host: &conduit_core::HostAdvertisement,
        bases: &[conduit_core::BaseImplementationId],
        authority: &[conduit_core::AuthorityGrant],
    ) -> Result<Self, String> {
        let source = compile_network_setup()?;
        let hosts = core::slice::from_ref(host);
        let placements = conduit_planner::default_expanded_placements(&source, hosts)
            .map_err(|error| format!("network placements: {error:?}"))?;
        let plan = conduit_planner::plan_expanded_canonical_with_options(
            &source,
            hosts,
            &placements,
            bases,
            conduit_planner::PlanningOptions {
                connection_bases: &std::collections::BTreeMap::new(),
                line_candidates: &std::collections::BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
                authority_grants: authority,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|error| format!("network planning: {error:?}"))?;
        let [fragment] = plan.fragments.as_slice() else {
            return Err("network setup must have one local fragment".into());
        };
        if fragment.host_id != host.host_id || fragment.boot_id != host.boot_id {
            return Err("network setup selected a foreign Host/Boot".into());
        }
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
            .map_err(|error| format!("network lowering: {error:?}"))?;
        Ok(Self { plan, lowered })
    }
    pub fn plan(&self) -> &conduit_core::Plan {
        &self.plan
    }
    pub fn lowered(&self) -> &conduit_plan_lowering::lowering::LoweredPlanFragment {
        &self.lowered
    }
    /// Ordinary Body workset material, not a proposal, Wake or start receipt.
    pub fn body_plot_plan(&self) -> conduit_body::BodyPlotPlan {
        conduit_body::BodyPlotPlan {
            plot: conduit_body::ResidentPlot::new(
                self.plan.source_document_id.clone(),
                self.plan.checked_plot_id.clone(),
            ),
            plan: self.plan.clone(),
        }
    }
}
