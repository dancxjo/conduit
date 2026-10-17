use super::FaceMechanism;
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

pub(super) fn structural_node(mechanism: &FaceMechanism) -> Option<StructuralNode> {
    let (component, text) = match mechanism {
        FaceMechanism::Shell => (ApplicationComponent::Shell, String::new()),
        FaceMechanism::Workbench | FaceMechanism::Grid => {
            (ApplicationComponent::Grid, String::new())
        }
        FaceMechanism::Panel { title } => (ApplicationComponent::Panel, title.clone()),
        FaceMechanism::Heading { text } => (ApplicationComponent::Heading, text.clone()),
        FaceMechanism::Separator { label } => (ApplicationComponent::Separator, label.clone()),
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
