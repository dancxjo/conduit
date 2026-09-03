use super::{FieldKind, FormField, SemanticPresentationRefusal};
use crate::{
    ApplicationAction, ApplicationComponent, ApplicationEventKind, ApplicationNodeState,
    ApplicationViewNode, ApplicationViewRefusal, MAX_APPLICATION_VIEW_NODES,
};
use alloc::{format, string::String, vec::Vec};

use super::mechanism_lowering::field_node;

type LoweredNode = (
    ApplicationComponent,
    String,
    String,
    u32,
    Option<u8>,
    ApplicationNodeState,
);

pub(super) fn lower_form_field(
    parent: u8,
    source_key: &str,
    field: &FormField,
    nodes: &mut Vec<ApplicationViewNode>,
    actions: &mut Vec<ApplicationAction>,
    device: bool,
) -> Result<(), SemanticPresentationRefusal> {
    let options = match &field.kind {
        FieldKind::Select { options } => options.as_slice(),
        _ => &[],
    };
    let event_is_exact = matches!(
        (&field.kind, field.input_action.event),
        (
            FieldKind::Text | FieldKind::TextArea,
            ApplicationEventKind::Input
        ) | (FieldKind::Select { .. }, ApplicationEventKind::Change)
    );
    let required = 3 + usize::from(field.error.is_some()) + options.len();
    if nodes.len().saturating_add(required) > MAX_APPLICATION_VIEW_NODES {
        return Err(SemanticPresentationRefusal::ApplicationView(
            ApplicationViewRefusal::TooManyNodes,
        ));
    }
    if field.label.is_empty()
        || field.help.is_empty()
        || field.error.as_ref().is_some_and(String::is_empty)
        || field.value_capacity == 0
        || !event_is_exact
        || field.value.len() > usize::try_from(field.value_capacity).unwrap_or(0)
        || options.iter().any(String::is_empty)
        || options
            .iter()
            .enumerate()
            .any(|(index, option)| options[..index].contains(option))
        || matches!(&field.kind, FieldKind::Select { .. } if !options.iter().any(|option| option == &field.value))
    {
        return Err(if device {
            SemanticPresentationRefusal::InvalidDeviceChoice
        } else {
            SemanticPresentationRefusal::InvalidField
        });
    }

    push_text(
        parent,
        nodes,
        ApplicationComponent::FieldLabel,
        &field.label,
    )?;
    let control_index = push_control(parent, source_key, field, nodes, actions, device)?;
    push_text(parent, nodes, ApplicationComponent::FieldHelp, &field.help)?;
    if let Some(error) = &field.error {
        push_text(parent, nodes, ApplicationComponent::FieldError, error)?;
    }
    if let FieldKind::Select { options } = &field.kind {
        for option in options {
            let index = next_index(nodes)?;
            nodes.push(ApplicationViewNode {
                parent: Some(control_index),
                component: ApplicationComponent::Option,
                key: generated_key(parent, index),
                text: option.clone(),
                value: option.clone(),
                value_capacity: u32::try_from(option.len()).unwrap_or(u32::MAX).max(1),
                action: None,
                state: ApplicationNodeState::Ready,
            });
        }
    }
    Ok(())
}

pub(super) fn progress_node(
    component: ApplicationComponent,
    title: &str,
    current: u16,
    total: u16,
) -> LoweredNode {
    let value = format!("{current}/{total}");
    (
        component,
        title.into(),
        value,
        11,
        None,
        ApplicationNodeState::Ready,
    )
}

fn push_control(
    parent: u8,
    source_key: &str,
    field: &FormField,
    nodes: &mut Vec<ApplicationViewNode>,
    actions: &mut Vec<ApplicationAction>,
    device: bool,
) -> Result<u8, SemanticPresentationRefusal> {
    let index = next_index(nodes)?;
    let (component, text, value, value_capacity, action, state) =
        field_node(field, actions, device)?;
    nodes.push(ApplicationViewNode {
        parent: Some(parent),
        component,
        key: if source_key.len() <= 24 {
            format!("{source_key}-control")
        } else {
            generated_key(parent, index)
        },
        text,
        value,
        value_capacity,
        action,
        state,
    });
    Ok(index)
}

fn push_text(
    parent: u8,
    nodes: &mut Vec<ApplicationViewNode>,
    component: ApplicationComponent,
    text: &str,
) -> Result<(), SemanticPresentationRefusal> {
    let index = next_index(nodes)?;
    nodes.push(ApplicationViewNode {
        parent: Some(parent),
        component,
        key: generated_key(parent, index),
        text: text.into(),
        value: String::new(),
        value_capacity: 0,
        action: None,
        state: ApplicationNodeState::Ready,
    });
    Ok(())
}

fn next_index(nodes: &[ApplicationViewNode]) -> Result<u8, SemanticPresentationRefusal> {
    u8::try_from(nodes.len()).map_err(|_| {
        SemanticPresentationRefusal::ApplicationView(ApplicationViewRefusal::TooManyNodes)
    })
}

fn generated_key(parent: u8, index: u8) -> String {
    format!("n{parent}-f{index}")
}
