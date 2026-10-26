//! Checked ordinary Mask graph and Fore; no current Host, renderer or authority.
use super::*;
use std::fmt::{self, Write};
pub struct SevenSegmentMaskMaterial {
    image: Vec<u8>,
    identity: c::PlotIdentity,
    role: f::MaskPlot,
    front: c::CheckedFront,
    inputs: Vec<p::AuthoringFrontBinding>,
    outputs: Vec<p::AuthoringFrontBinding>,
}
impl SevenSegmentMaskMaterial {
    pub fn prepare(source: &SevenSegmentMaskSource) -> Result<Self, String> {
        source.plot.validate_boundary_provenance().map_err(debug)?;
        let role = f::MaskPlot::admit(&source.plot).map_err(debug)?;
        if role != source.role {
            return Err("ordinary Mask role differs from checked Source".into());
        }
        Ok(Self {
            image: p::flat_image::indexed::encode(&source.plot.expanded).map_err(debug)?,
            identity: role.plot_identity.clone(),
            role,
            front: source.plot.front.clone(),
            inputs: source.plot.input_bindings.clone(),
            outputs: source.plot.output_bindings.clone(),
        })
    }
    pub fn planning_image(&self) -> &[u8] {
        &self.image
    }
    pub fn role(&self) -> &f::MaskPlot {
        &self.role
    }
    pub fn planning_boundary(
        &self,
    ) -> Result<
        conduit_planner::indexed_source::IndexedAuthoringBoundary,
        conduit_planner::PlannerError,
    > {
        // SAFETY: private graph and Fore captured together after exact original
        // authoring provenance and ordinary Mask-role admission.
        unsafe {
            conduit_planner::indexed_source::IndexedAuthoringBoundary::from_make(
                self.identity.clone(),
                self.front.clone(),
                self.inputs.clone(),
                self.outputs.clone(),
            )
        }
    }
    /// Root authenticates the original digest before installing callbacks and
    /// independently admits actual retained renderer/routing owners, Body,
    /// grants, Fore and Mask role. These immutable records create no readiness.
    pub fn write_rust(&self, out: &mut impl Write) -> fmt::Result {
        writeln!(out, "use conduit_core as c;")?;
        writeln!(out, "pub fn source() -> c::PlotIdentity {{ c::PlotIdentity {{ source_document_id:{:?}.into(), checked_plot_id:{:?}.into(), expanded_plot_id:{:?}.into() }} }}", self.identity.source_document_id.as_str(), self.identity.checked_plot_id.as_str(), self.identity.expanded_plot_id.as_str())?;
        writeln!(
            out,
            "pub fn planning_source_image() -> &'static [u8] {{ &{:?} }}",
            self.image
        )?;
        writeln!(
            out,
            "pub fn planning_source_sha256() -> [u8;32] {{ {:?} }}",
            c::assigned_plan_payload_digest(&self.image)
        )?;
        let role = serde_json::to_vec(&self.role).map_err(|_| fmt::Error)?;
        writeln!(out, "pub const ROLE: &[u8] = &{role:?};")?;
        writeln!(out, "pub fn role() -> Result<conduit_face::MaskPlot, ()> {{ serde_json::from_slice(ROLE).map_err(|_| ()) }}")?;
        // SAFETY: this immutable original is the private validated Make graph.
        let image =
            unsafe { p::flat_image::indexed::IndexedSourceImage::from_make_image(&self.image) }
                .map_err(|_| fmt::Error)?;
        for index in 0..image.gear_count() {
            let gear = image.gear(index).map_err(|_| fmt::Error)?;
            crate::source_recipe::linked_text::write_gear(
                out,
                &format!("mask_gear_{index}"),
                &gear,
            )?;
            crate::source_recipe::linked_text::write_configuration(
                out,
                &format!("mask_configuration_{index}"),
                &gear,
            )?;
        }
        for (name, output, prefix) in [
            (
                "decode_planning_gear",
                "conduit_plot::CheckedGear",
                "mask_gear",
            ),
            (
                "decode_planning_configuration",
                "conduit_plot::flat_image::indexed::IndexedGearConfiguration",
                "mask_configuration",
            ),
        ] {
            writeln!(out, "pub fn {name}(index:usize) -> Result<{output}, conduit_plot::flat_image::ImageRefusal> {{ match index {{")?;
            for index in 0..image.gear_count() {
                writeln!(out, "{index} => {prefix}_{index}(),")?;
            }
            writeln!(
                out,
                "_ => Err(conduit_plot::flat_image::ImageRefusal::Representation) }} }}"
            )?;
        }
        let front = serde_json::to_vec(&self.front).map_err(|_| fmt::Error)?;
        writeln!(out, "pub const FRONT: &[u8] = &{front:?};")?;
        for (name, rows) in [
            ("planning_inputs", &self.inputs),
            ("planning_outputs", &self.outputs),
        ] {
            writeln!(out, "pub fn {name}() -> alloc::vec::Vec<conduit_plot::AuthoringFrontBinding> {{ alloc::vec![")?;
            for row in rows {
                writeln!(out, "conduit_plot::AuthoringFrontBinding {{ front_port_id:{:?}.into(), gear_id:{:?}.into(), gear_port_id:{:?}.into(), track:c::ConnectionTrack::{:?},", row.front_port_id.as_str(), row.gear_id.as_str(), row.gear_port_id.as_str(), row.track)?;
                match &row.source_origin {
                    Some(origin) => {
                        let span = origin.source_span;
                        writeln!(out, "source_origin:Some(conduit_plot::AuthoringCordOrigin {{ origin:conduit_plot::ExpandedConnectionOrigin::{:?}, source_span:conduit_plot::Span {{start:{},end:{},line:{},column:{},end_line:{},end_column:{}}} }}),", origin.origin, span.start, span.end, span.line, span.column, span.end_line, span.end_column)?;
                    }
                    None => writeln!(out, "source_origin:None,")?,
                }
                writeln!(out, "}},")?;
            }
            writeln!(out, "] }}")?;
        }
        writeln!(out, "pub fn planning_boundary() -> Result<conduit_planner::indexed_source::IndexedAuthoringBoundary, ()> {{")?;
        writeln!(out, "let front=serde_json::from_slice(FRONT).map_err(|_|())?; let inputs=planning_inputs(); let outputs=planning_outputs();")?;
        writeln!(
            out,
            "// SAFETY: exact private checked Make graph/Fore/provenance, never ingress metadata."
        )?;
        writeln!(out, "unsafe {{ conduit_planner::indexed_source::IndexedAuthoringBoundary::from_make(source(),front,inputs,outputs) }}.map_err(|_|()) }}")
    }
}
