//! Resident application view derived from the authoritative selection.
use super::*;
impl PatchbayApplicationPort {
    pub(super) fn view(&self) -> Result<ApplicationView, PatchbayApplicationRefusal> {
        let mut actions = Vec::with_capacity(self.plots.len() + 3);
        for index in 0..self.plots.len() {
            actions.push(ApplicationAction {
                id: format!("{SELECT_PLOT_ACTION_PREFIX}{index}"),
                event: ApplicationEventKind::Activate,
            });
        }
        let inspect_action = actions.len();
        actions.push(ApplicationAction {
            id: INSPECT_NEXT_ACTION_ID.into(),
            event: ApplicationEventKind::Activate,
        });
        let edit_action = actions.len();
        actions.push(ApplicationAction {
            id: EDIT_CURRENT_ACTION_ID.into(),
            event: ApplicationEventKind::Activate,
        });
        let mask_action = actions.len();
        if self.mask_topology.is_some() {
            actions.push(mask::action());
        }
        let canvas_action = actions.len();
        actions.push(ApplicationAction {
            id: resident_canvas::SELECT_SUBJECT.into(),
            event: ApplicationEventKind::Change,
        });
        let mut nodes = vec![
            node(
                None,
                ApplicationComponent::Shell,
                "patchbay",
                "Patchbay",
                "",
                0,
                None,
            ),
            node(
                Some(0),
                ApplicationComponent::Main,
                "active-plots",
                "Active plots on this body",
                "",
                0,
                None,
            ),
        ];
        let mut evidence = Vec::new();
        if let Some(active_play_id) = &self.active_play_id {
            evidence.push(node(
                Some(1),
                ApplicationComponent::Status,
                "body-play",
                &format!(
                    "Body Plan {} · Play {}",
                    self.body_plan_id.as_str(),
                    active_play_id.as_str()
                ),
                "",
                0,
                None,
            ));
        }
        for (index, plot) in self.plots.iter().enumerate() {
            let state = match plot.state {
                PatchbayActivePlotState::Playing => "Playing",
                PatchbayActivePlotState::Lulled => "Lulled",
            };
            let focus = if plot.focused { " · foreground" } else { "" };
            nodes.push(node(
                Some(1),
                ApplicationComponent::Button,
                &format!("active-plot-{index}"),
                &plot.title,
                "",
                0,
                Some(index as u8),
            ));
            nodes.push(node(
                Some(1),
                ApplicationComponent::Status,
                &format!("active-plot-status-{index}"),
                &format!("{state}{focus}"),
                "",
                0,
                None,
            ));
            evidence.push(node(
                Some(1),
                ApplicationComponent::Status,
                &format!("checked-plot-{index}"),
                &format!("{} · checked {}", plot.title, plot.checked_plot_id),
                "",
                0,
                None,
            ));
        }
        if let Some(index) = self.selected_plot {
            let plot = &self.plots[index];
            let inspection = self.inspection()?;
            nodes.push(node(
                Some(1),
                ApplicationComponent::PatchbayCanvas,
                "resident-graph",
                "Plot connections",
                &self.canvas()?,
                65_536,
                Some(canvas_action as u8),
            ));
            evidence.push(node(
                Some(1),
                ApplicationComponent::Status,
                "identity",
                &format!(
                    "Expanded Plot {} · Plan {} · Body Plan {}",
                    plot.graph.expanded_plot_id.as_str(),
                    plot.plan_id.as_str(),
                    self.body_plan_id.as_str()
                ),
                "",
                0,
                None,
            ));
            nodes.extend([
                node(
                    Some(1),
                    ApplicationComponent::Heading,
                    "plot",
                    &plot.title,
                    "",
                    0,
                    None,
                ),
                node(
                    Some(1),
                    ApplicationComponent::Definition,
                    "subject",
                    &format!("{:?}", inspection.subject_kind),
                    &inspection.subject_identity,
                    768,
                    None,
                ),
                node(
                    Some(1),
                    ApplicationComponent::Paragraph,
                    "facts",
                    &inspection.exact_facts.join(" · "),
                    "",
                    0,
                    None,
                ),
                node(
                    Some(1),
                    ApplicationComponent::Button,
                    "inspect-next",
                    "Inspect next",
                    "",
                    0,
                    Some(inspect_action as u8),
                ),
                node(
                    Some(1),
                    ApplicationComponent::Button,
                    "edit-current",
                    "Edit current",
                    "",
                    0,
                    Some(edit_action as u8),
                ),
            ]);
        }
        if self.edit_requested {
            nodes.push(node(
                Some(1),
                ApplicationComponent::Status,
                "edit-request",
                "Edit requested against the exact inspected subject; no authority was inferred.",
                "",
                0,
                None,
            ));
        }
        if let Some(topology) = &self.mask_topology {
            mask::append_nodes(topology, &mut nodes, mask_action as u8);
        }
        let disclosure = nodes.len() as u8;
        nodes.push(node(
            Some(1),
            ApplicationComponent::Disclosure,
            "exact-evidence",
            "Inspect exact evidence",
            "",
            0,
            None,
        ));
        nodes.push(node(
            Some(disclosure),
            ApplicationComponent::Summary,
            "exact-evidence-title",
            "Inspect exact evidence",
            "",
            0,
            None,
        ));
        for mut item in evidence {
            item.parent = Some(disclosure);
            nodes.push(item);
        }
        let view = ApplicationView {
            revision: self.revision,
            nodes,
            actions,
        };
        view.validate().map_err(PatchbayApplicationRefusal::Event)?;
        Ok(view)
    }
}

fn node(
    parent: Option<u8>,
    component: ApplicationComponent,
    key: &str,
    text: &str,
    value: &str,
    value_capacity: u32,
    action: Option<u8>,
) -> ApplicationViewNode {
    ApplicationViewNode {
        parent,
        component,
        key: key.into(),
        text: text.into(),
        value: value.into(),
        value_capacity,
        action,
        state: ApplicationNodeState::Ready,
    }
}
