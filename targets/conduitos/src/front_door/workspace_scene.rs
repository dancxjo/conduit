//! Host/reference workspace rendering uses the same canonical Mask layout.
use super::{Error, application_layout::ApplicationViewport};
use crate::{display::PixelTarget, product_journey::JourneyProjection};
#[cfg(test)]
use conduit_face::{ApplicationComponent, LayoutRect};
use conduit_face::{ApplicationView, GraphicsScene};
#[cfg(test)]
use conduitos_mask_core::scene::application_text;

pub(super) fn scene(
    journey: &JourneyProjection,
    foreground_title: &str,
    application_view: Option<&ApplicationView>,
    viewport: ApplicationViewport,
    patchbay_graph_available: bool,
    refusal: Option<(&str, &str)>,
    display: &impl PixelTarget,
) -> Result<GraphicsScene, Error> {
    let format = display.format().validate().map_err(Error::Display)?;
    let facts = super::workspace_mask::facts(
        journey,
        foreground_title,
        patchbay_graph_available,
        refusal,
        false,
    );
    conduitos_mask_core::scene::scene(
        &facts,
        foreground_title,
        application_view,
        viewport,
        patchbay_graph_available,
        refusal,
        u16::try_from(format.width).map_err(|_| Error::Scene)?,
        u16::try_from(format.height).map_err(|_| Error::Scene)?,
    )
    .map_err(|_| Error::Scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec};
    use conduit_face::{
        ApplicationAction, ApplicationEventKind, ApplicationNodeState, ApplicationViewNode,
    };

    #[test]
    fn application_projection_keeps_exact_identity_and_action_on_native_surface() {
        let exact = "expanded/plot/exact";
        let view = ApplicationView {
            revision: 1,
            nodes: vec![
                ApplicationViewNode {
                    parent: None,
                    component: ApplicationComponent::Shell,
                    key: "shell".into(),
                    text: "Patchbay".into(),
                    value: String::new(),
                    value_capacity: 0,
                    action: None,
                    state: ApplicationNodeState::Ready,
                },
                ApplicationViewNode {
                    parent: Some(0),
                    component: ApplicationComponent::Definition,
                    key: "subject".into(),
                    text: "Plot".into(),
                    value: exact.into(),
                    value_capacity: 64,
                    action: None,
                    state: ApplicationNodeState::Ready,
                },
                ApplicationViewNode {
                    parent: Some(0),
                    component: ApplicationComponent::Button,
                    key: "inspect".into(),
                    text: "Inspect next".into(),
                    value: String::new(),
                    value_capacity: 0,
                    action: Some(0),
                    state: ApplicationNodeState::Ready,
                },
            ],
            actions: vec![ApplicationAction {
                id: "patchbay.inspect.next".into(),
                event: ApplicationEventKind::Activate,
            }],
        };
        let mut scene = GraphicsScene::empty();
        application_text(
            &mut scene,
            LayoutRect {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
            },
            100,
            &view,
            ApplicationViewport::default(),
        )
        .unwrap();
        assert!(
            scene
                .commands()
                .iter()
                .any(|command| command.payload() == exact)
        );
        assert!(
            scene
                .commands()
                .iter()
                .any(|command| command.payload() == "Inspect next")
        );
    }
}
