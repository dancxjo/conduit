//! The selected resident graph projected into the existing finite canvas slot.
use super::*;
use conduit_presentation::application_canvas::{
    ApplicationCanvas, CanvasCord, CanvasNode, CanvasPort,
};

pub(super) const SELECT_SUBJECT: &str = "patchbay.subject.select";
impl PatchbayApplicationPort {
    pub(super) fn canvas(&self) -> Result<String, PatchbayApplicationRefusal> {
        let plot = self.selected_active_plot()?;
        let graph = &plot.graph;
        let mut canvas = ApplicationCanvas {
            checked_plot: plot.checked_plot_id.clone(),
            expanded_plot: graph.expanded_plot_id.as_str().into(),
            plan: plot.plan_id.as_str().into(),
            body_plan: self.body_plan_id.as_str().into(),
            play: self
                .active_play_id
                .as_ref()
                .map(|id| id.as_str().into())
                .unwrap_or_default(),
            selected: self.inspection()?.subject_identity,
            nodes: Vec::new(),
            ports: Vec::new(),
            cords: Vec::new(),
        };
        for gear in &graph.gears {
            canvas.nodes.push(CanvasNode {
                identity: gear.identity.clone(),
                label: gear.gear_id.as_str().into(),
                kind: gear.kind_id.as_str().into(),
            });
            for port in gear.inputs.iter().chain(&gear.outputs) {
                canvas.ports.push(port_record(
                    &port.identity,
                    &gear.identity,
                    &port.descriptor,
                ));
            }
        }
        for composition in &graph.compositions {
            canvas.nodes.push(CanvasNode {
                identity: composition.identity.clone(),
                label: composition.gear_name.clone(),
                kind: composition.back_name.clone(),
            });
            for port in composition.inputs.iter().chain(&composition.outputs) {
                canvas.ports.push(port_record(
                    &port.identity,
                    &composition.identity,
                    &port.descriptor,
                ));
            }
        }
        for port in graph.front_inputs.iter().chain(&graph.front_outputs) {
            canvas
                .ports
                .push(port_record(&port.identity, "", &port.descriptor));
        }
        for cord in &graph.cords {
            canvas.cords.push(CanvasCord {
                identity: cord.identity.clone(),
                source: cord.source_port.clone(),
                sink: cord.sink_port.clone(),
            });
        }
        canvas.encode().map_err(PatchbayApplicationRefusal::Event)
    }
    pub(super) fn select_canvas_subject(
        &mut self,
        value: &[u8],
    ) -> Result<(), PatchbayApplicationRefusal> {
        let identity =
            core::str::from_utf8(value).map_err(|_| PatchbayApplicationRefusal::NoSubject)?;
        let selected = self
            .selected_active_plot()?
            .graph
            .subject_identities()
            .position(|subject| subject == identity)
            .ok_or(PatchbayApplicationRefusal::NoSubject)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
        self.selected_subject = selected;
        self.revision = revision;
        Ok(())
    }
}
fn port_record(
    identity: &str,
    owner: &str,
    descriptor: &conduit_core::PortDescriptor,
) -> CanvasPort {
    CanvasPort {
        identity: identity.into(),
        owner: owner.into(),
        label: descriptor.port_id.as_str().into(),
        direction: match descriptor.direction {
            conduit_core::PortDirection::Input => "input",
            conduit_core::PortDirection::Output => "output",
        }
        .into(),
        info: descriptor.value_kind.as_str().into(),
        temporal: format!("{:?}", descriptor.temporal),
    }
}
