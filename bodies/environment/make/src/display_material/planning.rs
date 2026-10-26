//! Linked checked display graph and Fore remain separate from initialized
//! computation owners. No current offer, native grant or Plan is serialized.
use super::*;
impl DisplaySourceMaterial {
    pub fn planning_image(&self) -> &[u8] {
        &self.planning_image
    }
    pub fn planning_boundary(
        &self,
    ) -> Result<
        conduit_planner::indexed_source::IndexedAuthoringBoundary,
        conduit_planner::PlannerError,
    > {
        let bindings = |rows: &[conduit_plot::AuthoringFrontBinding]| {
            rows.iter()
                .map(|row| {
                    let mut row = row.clone();
                    row.source_origin = None;
                    row
                })
                .collect()
        };
        // SAFETY: private fields were captured together only after validating
        // the original checked authoring graph and its boundary provenance.
        unsafe {
            conduit_planner::indexed_source::IndexedAuthoringBoundary::from_make(
                self.identity.clone(),
                self.front.clone(),
                bindings(&self.inputs),
                bindings(&self.outputs),
            )
        }
    }
    pub(super) fn write_planning_rust(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(out,"#[cfg(feature = \"target-memory-check\")]\npub fn planning_source_image() -> &'static [u8] {{ &{:?} }}",self.planning_image)?;
        writeln!(out,"#[cfg(feature = \"target-memory-check\")]\npub fn planning_source_sha256() -> [u8;32] {{ {:?} }}",c::assigned_plan_payload_digest(&self.planning_image))?;
        writeln!(out,"#[cfg(feature = \"target-memory-check\")]\npub fn planning_boundary() -> Result<conduit_planner::indexed_source::IndexedAuthoringBoundary, ()> {{")?;
        writeln!(out,"let source=c::PlotIdentity {{ source_document_id:{:?}.into(),checked_plot_id:{:?}.into(),expanded_plot_id:{:?}.into() }};",self.identity.source_document_id.as_str(),self.identity.checked_plot_id.as_str(),self.identity.expanded_plot_id.as_str())?;
        let front = serde_json::to_vec(&self.front).map_err(|_| fmt::Error)?;
        writeln!(
            out,
            "let front:c::CheckedFront=serde_json::from_slice(&{:?}).map_err(|_|())?;",
            front
        )?;
        for (name, bindings) in [("inputs", &self.inputs), ("outputs", &self.outputs)] {
            writeln!(out, "let {name}=alloc::vec![")?;
            for binding in bindings {
                writeln!(out,"conduit_plot::AuthoringFrontBinding {{ source_origin:None,front_port_id:{:?}.into(),gear_id:{:?}.into(),gear_port_id:{:?}.into(),track:c::ConnectionTrack::{:?} }},",binding.front_port_id.as_str(),binding.gear_id.as_str(),binding.gear_port_id.as_str(),binding.track)?;
            }
            writeln!(out, "];")?;
        }
        writeln!(out,"// SAFETY: the complete graph/Fore pair is independently checked Make output linked into Root, never received descriptors.")?;
        writeln!(out,"unsafe {{ conduit_planner::indexed_source::IndexedAuthoringBoundary::from_make(source,front,inputs,outputs) }}.map_err(|_|()) }}")
    }
}
