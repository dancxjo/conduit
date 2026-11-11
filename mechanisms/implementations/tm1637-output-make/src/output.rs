//! Checked output meaning, with no initialized provider or implied authority.
use conduit_core as c;
use conduit_plot as p;
pub struct DisplayOutputSource {
    pub plot: p::ExpandedAuthoringPlot,
    value_bounds: Vec<(c::KindId, u32)>,
}
impl DisplayOutputSource {
    pub fn compile() -> Result<Self, String> {
        let contract = conduit_tm1637_wire::WriteContract::prepare().map_err(debug)?;
        let kind = contract.kind();
        let mut startup = p::StartupCatalog::new();
        let mut profiles = p::ProfileCatalog::new();
        startup
            .insert(p::KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .map_err(debug)?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profiles.insert_kind(kind.clone()).map_err(debug)?;
        let absence = c::StructuredInfoType::leaf(c::kind_id(c::EMPTY_INFO_ID))
            .and_then(|ty| ty.profile())
            .map_err(debug)?;
        startup.insert_value_kind_alias("Tm1637Absence", absence.value_kind().clone())?;
        let source = format!(
            "{}\n{}",
            include_str!("../../../../plots/device-protocols/tm1637.conduit"),
            include_str!("../../profiles/display-output.conduit")
        );
        let syntax = p::parse_syntax_document(&source);
        if !syntax.diagnostics.is_empty() {
            return Err(format!("display Source: {:?}", syntax.diagnostics));
        }
        let checked = p::check_syntax_document(&syntax, &startup).map_err(debug)?;
        for plot in &checked.plots {
            for cord in &plot.cords {
                for stage in &cord.stages {
                    if let p::CheckedCordStage::StructuredSelector { selector, .. } = stage {
                        profiles
                            .insert(p::structured_selector_definition(
                                selector,
                                c::PortTemporal::Flow { closes: true },
                            ))
                            .map_err(debug)?;
                    }
                }
            }
        }
        let mut value_bounds = Vec::new();
        for ty in checked.native_types.iter().map(|ty| &ty.value_type) {
            value_bounds.push((
                ty.profile().map_err(debug)?.value_kind().clone(),
                p::maximum_prepared_canonical_value_bytes(ty).map_err(debug)?,
            ));
        }
        let absence = c::StructuredInfoType::leaf(c::kind_id(c::EMPTY_INFO_ID)).map_err(debug)?;
        value_bounds.push((
            absence.profile().map_err(debug)?.value_kind().clone(),
            p::maximum_prepared_canonical_value_bytes(&absence).map_err(debug)?,
        ));
        let plot = p::expand_canonical_plot_for_authoring(
            &checked,
            "environment-display-output",
            &profiles,
        )
        .map_err(debug)?;
        let source = Self { plot, value_bounds };
        source.maximum_value_bytes()?;
        Ok(source)
    }
    /// Conservative canonical extent from every exact Type at a payload port.
    /// Refuse unknown/abnormal Types instead of guessing from observed values.
    pub fn maximum_value_bytes(&self) -> Result<u32, String> {
        self.plot
            .expanded
            .gears
            .iter()
            .flat_map(|gear| gear.inputs.iter().chain(&gear.outputs))
            .try_fold(1, |maximum, port| {
                if port.abnormal_kind.is_some() {
                    return Err("output abnormal Type needs its own extent proof".into());
                }
                let bytes = self
                    .value_bounds
                    .iter()
                    .find(|(kind, _)| kind == &port.value_kind)
                    .ok_or("output payload Type is not independently bounded")?
                    .1;
                if bytes > conduit_tm1637_wire::MAXIMUM_BYTES {
                    return Err("output Type exceeds native profile".into());
                }
                Ok(maximum.max(bytes))
            })
    }
}
fn debug(reason: impl core::fmt::Debug) -> String {
    format!("{reason:?}")
}

#[derive(Debug)]
pub enum DisplayPlanningRefusal {
    Placement(conduit_planner::PlannerError),
    Planning(conduit_planner::PlannerError),
    NonlocalFragment,
    Lowering(conduit_plan_lowering::lowering::LoweringError),
    ValueExtent(String),
}
pub struct PreparedDisplayOutput {
    plan: c::Plan,
    lowered: conduit_plan_lowering::lowering::LoweredPlanFragment,
    maximum_value_bytes: u32,
}
impl PreparedDisplayOutput {
    /// Current authenticated offers, actual retained software/providers and
    /// grants remain Root custody. This constructor creates no implementation,
    /// membership, capability possession or permission to start a Play.
    pub fn prepare(
        source: &DisplayOutputSource,
        host: &c::HostAdvertisement,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
    ) -> Result<Self, DisplayPlanningRefusal> {
        use std::collections::BTreeMap;
        use DisplayPlanningRefusal as E;
        let hosts = core::slice::from_ref(host);
        let choices = conduit_planner::default_expanded_placements(&source.plot.expanded, hosts)
            .map_err(E::Placement)?;
        let mut boundaries = BTreeMap::new();
        for (direction, bindings, bytes) in [
            (
                c::PortDirection::Input,
                &source.plot.input_bindings,
                conduit_tm1637_wire::MAXIMUM_BYTES,
            ),
            (c::PortDirection::Output, &source.plot.output_bindings, 64),
        ] {
            for binding in bindings {
                boundaries.insert(
                    conduit_planner::ForeBoundaryKey {
                        direction,
                        front_port_id: binding.front_port_id.clone(),
                        track: binding.track,
                    },
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: bytes,
                    },
                );
            }
        }
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &source.plot,
            hosts,
            &choices,
            bases,
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: conduit_tm1637_wire::MAXIMUM_BYTES,
                authority_grants: authority,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .map_err(E::Planning)?;
        let [fragment] = plan.fragments.as_slice() else {
            return Err(E::NonlocalFragment);
        };
        if fragment.host_id != host.host_id || fragment.boot_id != host.boot_id {
            return Err(E::NonlocalFragment);
        }
        let (lowered, activations) =
            conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
                &plan,
                &fragment.fragment_id,
            )
            .map_err(E::Lowering)?;
        if !activations.entries.is_empty() {
            return Err(E::NonlocalFragment);
        }
        Ok(Self {
            plan,
            lowered,
            maximum_value_bytes: source.maximum_value_bytes().map_err(E::ValueExtent)?,
        })
    }
    pub fn maximum_value_bytes(&self) -> u32 {
        self.maximum_value_bytes
    }
    pub fn plan(&self) -> &c::Plan {
        &self.plan
    }
    pub fn lowered(&self) -> &conduit_plan_lowering::lowering::LoweredPlanFragment {
        &self.lowered
    }
}
