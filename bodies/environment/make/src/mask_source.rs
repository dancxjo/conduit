//! Ordinary seven-segment Mask Source and exact planning against supplied offers.
//! This initializes no renderer, admits no provider and performs no output.
use conduit_core as c;
use conduit_face as f;
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot as p;
use std::collections::BTreeMap;
#[path = "mask_source_material.rs"]
mod material;
pub use material::SevenSegmentMaskMaterial;

pub struct SevenSegmentMaskSource {
    pub plot: p::ExpandedAuthoringPlot,
    pub role: f::MaskPlot,
}
impl SevenSegmentMaskSource {
    pub fn compile() -> Result<Self, String> {
        let mut startup = p::StartupCatalog::new();
        let mut profiles = p::ProfileCatalog::new();
        f::install_mask_plot_value_aliases(&mut startup)?;
        f::install_mask_mechanism_catalog(&mut startup, &mut profiles)?;
        let syntax = p::parse_syntax_document(include_str!(
            "../../../../plots/seven-segment-mask/main.conduit"
        ));
        if !syntax.diagnostics.is_empty() {
            return Err(format!("Mask syntax: {:?}", syntax.diagnostics));
        }
        let checked = p::check_syntax_document(&syntax, &startup).map_err(debug)?;
        let plot = p::expand_canonical_plot_for_authoring(&checked, "seven-segment", &profiles)
            .map_err(debug)?;
        let role = f::MaskPlot::admit(&plot).map_err(debug)?;
        Ok(Self { plot, role })
    }
    /// Caller-owned Host truth must contain independently initialized Backs,
    /// resources and grants. Metadata supplied here never establishes possession.
    /// `maximum_bytes` is a finite realization ceiling, not a new Face Type.
    pub fn plan(
        &self,
        host: &c::HostAdvertisement,
        maximum_bytes: u32,
        grants: &[c::AuthorityGrant],
    ) -> Result<f::PlannedMaskPlot, String> {
        if host.protocol_version != c::PROTOCOL_VERSION
            || host.host_id.as_str().is_empty()
            || host.boot_id.as_str().is_empty()
            || host.offer_generation.0 == 0
            || maximum_bytes == 0
        {
            return Err("invalid Mask Host or finite realization bound".into());
        }
        let hosts = [host.clone()];
        let placements = default_expanded_placements(&self.plot.expanded, &hosts).map_err(debug)?;
        let boundaries = [
            (c::PortDirection::Input, "face"),
            (c::PortDirection::Output, "show"),
            (c::PortDirection::Output, "interaction"),
        ]
        .into_iter()
        .map(|(direction, name)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: c::port_id(name),
                    track: c::ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: maximum_bytes,
                },
            )
        })
        .collect();
        let empty = BTreeMap::new();
        let plan = plan_expanded_authoring_with_options(
            &self.plot,
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            PlanningOptions {
                connection_bases: &empty,
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: maximum_bytes,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .map_err(debug)?;
        f::PlannedMaskPlot::admit(&self.role, &plan).map_err(debug)
    }
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
