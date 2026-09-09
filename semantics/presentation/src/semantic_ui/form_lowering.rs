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
    let option_count = match &field.kind {
        FieldKind::Select { options } => options.len(),
        FieldKind::NamedSelect { options } => options.len(),
        _ => 0,
    };
    let event_is_exact = matches!(
        (&field.kind, field.input_action.event),
        (
            FieldKind::Text | FieldKind::TextArea,
            ApplicationEventKind::Input
        ) | (
            FieldKind::Select { .. } | FieldKind::NamedSelect { .. },
            ApplicationEventKind::Change
        )
    );
    let required = option_count.saturating_add(3 + usize::from(field.error.is_some()));
    if nodes.len().saturating_add(required) > MAX_APPLICATION_VIEW_NODES {
        return Err(SemanticPresentationRefusal::ApplicationView(
            ApplicationViewRefusal::TooManyNodes,
        ));
    }
    let options = match &field.kind {
        FieldKind::Select { options } => options
            .iter()
            .map(|value| (value.as_str(), value.as_str()))
            .collect::<Vec<_>>(),
        FieldKind::NamedSelect { options } => options
            .iter()
            .map(|option| (option.identity.as_str(), option.label.as_str()))
            .collect(),
        _ => Vec::new(),
    };
    if field.label.is_empty()
        || field.help.is_empty()
        || field.error.as_ref().is_some_and(String::is_empty)
        || field.value_capacity == 0
        || !event_is_exact
        || field.value.len() > usize::try_from(field.value_capacity).unwrap_or(0)
        || options
            .iter()
            .any(|(value, label)| value.is_empty() || label.is_empty())
        || options
            .iter()
            .enumerate()
            .any(|(index, (value, _))| options[..index].iter().any(|(prior, _)| prior == value))
        || matches!(&field.kind, FieldKind::Select { .. } | FieldKind::NamedSelect { .. } if !options.iter().any(|(value, _)| *value == field.value))
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
    for (value, label) in options {
        let index = next_index(nodes)?;
        nodes.push(ApplicationViewNode {
            parent: Some(control_index),
            component: ApplicationComponent::Option,
            key: generated_key(parent, index),
            text: label.into(),
            value: value.into(),
            value_capacity: u32::try_from(value.len()).unwrap_or(u32::MAX).max(1),
            action: None,
            state: ApplicationNodeState::Ready,
        });
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
