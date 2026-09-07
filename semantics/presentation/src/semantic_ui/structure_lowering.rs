use super::PresentationMechanism;
use crate::{ApplicationComponent, ApplicationNodeState};
use alloc::string::String;

type StructuralNode = (
    ApplicationComponent,
    String,
    String,
    u32,
    Option<u8>,
    ApplicationNodeState,
);

pub(super) fn structural_node(mechanism: &PresentationMechanism) -> Option<StructuralNode> {
    let (component, text) = match mechanism {
        PresentationMechanism::Shell => (ApplicationComponent::Shell, String::new()),
        PresentationMechanism::Workbench | PresentationMechanism::Grid => {
            (ApplicationComponent::Grid, String::new())
        }
        PresentationMechanism::Panel { title } => (ApplicationComponent::Panel, title.clone()),
        PresentationMechanism::Heading { text } => (ApplicationComponent::Heading, text.clone()),
        _ => return None,
    };
    Some((
        component,
        text,
        String::new(),
        0,
        None,
        ApplicationNodeState::Ready,
    ))
}
