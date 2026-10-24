//! Ordinary Mask construction. Checking definitions do not advertise readiness.
use conduit_face::{MaskPlot, PlannedMaskPlot};
use conduit_plot as p;

pub struct DisplayMaskSource {
    plot: p::ExpandedAuthoringPlot,
    role: MaskPlot,
}

#[derive(Debug)]
pub enum MaskPlanningRefusal {
    Placement(conduit_planner::PlannerError),
    Planning(conduit_planner::PlannerError),
    Role(conduit_face::MaskPlotError),
}

impl DisplayMaskSource {
    pub fn compile() -> Result<Self, String> {
        let mut startup = p::StartupCatalog::new();
        let mut profiles = p::ProfileCatalog::new();
        conduit_face::install_mask_plot_value_aliases(&mut startup)?;
        conduit_face::install_mask_mechanism_catalog(&mut startup, &mut profiles)?;
        let syntax = p::parse_syntax_document(include_str!(
            "../../../../plots/seven-segment-mask/main.conduit"
        ));
        if !syntax.diagnostics.is_empty() {
            return Err(format!("Mask Source: {:?}", syntax.diagnostics));
        }
        let checked = p::check_syntax_document(&syntax, &startup)
            .map_err(|reason| format!("Mask checking: {reason:?}"))?;
        let plot = p::expand_canonical_plot_for_authoring(&checked, "seven-segment", &profiles)
            .map_err(|reason| format!("Mask expansion: {reason:?}"))?;
        let role = MaskPlot::admit(&plot).map_err(|reason| reason.to_string())?;
        Ok(Self { plot, role })
    }

    pub fn plot(&self) -> &p::ExpandedAuthoringPlot {
        &self.plot
    }

    /// The caller supplies current authenticated offers, possession/authority,
    /// and every finite queue bound. This starts no Play and claims no Face
    /// reader, clock, renderer, output caller, Body membership or electrical IO.
    pub fn plan(
        &self,
        hosts: &[conduit_core::HostAdvertisement],
        bases: &[conduit_core::BaseImplementationId],
        options: conduit_planner::PlanningOptions<'_>,
        boundaries: &std::collections::BTreeMap<
            conduit_planner::ForeBoundaryKey,
            conduit_planner::ConnectionQueueLimits,
        >,
    ) -> Result<PlannedMaskPlot, MaskPlanningRefusal> {
        use MaskPlanningRefusal as E;
        let placements = conduit_planner::default_expanded_placements(&self.plot.expanded, hosts)
            .map_err(E::Placement)?;
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &self.plot,
            hosts,
            &placements,
            bases,
            options,
            boundaries,
        )
        .map_err(E::Planning)?;
        PlannedMaskPlot::admit(&self.role, &plan).map_err(E::Role)
    }
}
