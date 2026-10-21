//! Exact Source-to-Plan admission for a separately installed canonical ingress.
use conduit_core::{ConnectionTrack, HostAdvertisement, Plan, PortDirection};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredForePort, LoweredPlanFragment};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};
use std::collections::BTreeMap;

pub struct PlannedPatchbayIngress {
    plan: Plan,
    lowered: LoweredPlanFragment,
}

impl PlannedPatchbayIngress {
    /// The host must already offer the exact Back. This function neither adds
    /// an offer nor borrows an owner route from a legacy application placement.
    pub fn prepare(host: &HostAdvertisement) -> Result<Self, String> {
        let mut startup = StartupCatalog::new();
        conduit_face::install_mask_plot_value_aliases(&mut startup).map_err(debug)?;
        startup
            .insert(KindSignature {
                kind: patchbay_application::PATCHBAY_INTERACTION_KIND.into(),
                startup_parameters: vec![],
            })
            .map_err(debug)?;
        let mut profile = ProfileCatalog::new();
        profile
            .insert_kind(patchbay_application::patchbay_interaction_contract())
            .map_err(debug)?;
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document(include_str!(
                "../../../../../plots/patchbay/interaction.conduit"
            )),
            &startup,
        )
        .map_err(debug)?;
        let authored = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "patchbay/interaction",
            &profile,
        )
        .map_err(debug)?;
        let hosts = core::slice::from_ref(host);
        let placements = conduit_planner::default_expanded_placements(&authored.expanded, hosts)
            .map_err(debug)?;
        let limits = BTreeMap::from([(
            conduit_planner::ForeBoundaryKey {
                direction: PortDirection::Input,
                front_port_id: conduit_core::port_id("interaction"),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: super::BYTES,
            },
        )]);
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &authored,
            hosts,
            &placements,
            &[],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: super::BYTES,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &limits,
        )
        .map_err(debug)?;
        let [fragment] = plan.fragments.as_slice() else {
            return Err("Patchbay ingress requires one exact local fragment".into());
        };
        let [placement] = fragment.placements.as_slice() else {
            return Err("Patchbay ingress requires one exact owner sink".into());
        };
        super::prepare(placement)?;
        let lowered = lower_plan_fragment(fragment).map_err(debug)?;
        let [input] = lowered.fore_ports.as_slice() else {
            return Err("Patchbay ingress requires one exact input boundary".into());
        };
        if input.direction != PortDirection::Input
            || input.value_kind.as_str() != conduit_face::FACE_INTERACTION_VALUE_KIND
            || input.temporal != (conduit_core::PortTemporal::Flow { closes: true })
            || input.item_capacity != 1
            || input.byte_capacity != super::BYTES
            || input.selected_line.is_some()
        {
            return Err("Patchbay ingress has a different typed finite boundary".into());
        }
        Ok(Self { plan, lowered })
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    pub fn lowered(&self) -> &LoweredPlanFragment {
        &self.lowered
    }
    pub fn input(&self) -> &LoweredForePort {
        &self.lowered.fore_ports[0]
    }
}

fn debug(error: impl core::fmt::Debug) -> String {
    format!("Patchbay ingress preparation: {error:?}")
}
