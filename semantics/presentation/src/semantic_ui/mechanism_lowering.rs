use super::{
    ActionAvailability, ChoiceMultiplicity, ChoiceOption, FieldKind, FormField, SemanticAction,
    SemanticPresentationRefusal,
};
use crate::{
    ApplicationAction, ApplicationComponent, ApplicationNodeState, ApplicationViewRefusal,
};
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

type LoweredNode = (
    ApplicationComponent,
    String,
    String,
    u32,
    Option<u8>,
    ApplicationNodeState,
);

pub(super) fn action_node(
    action: &SemanticAction,
    component: ApplicationComponent,
    actions: &mut Vec<ApplicationAction>,
) -> Result<LoweredNode, SemanticPresentationRefusal> {
    let (label, state) = availability_label(action)?;
    let index = (state == ApplicationNodeState::Ready)
        .then(|| admit_action(action, actions))
        .transpose()?;
    Ok((component, label, String::new(), 0, index, state))
}

pub(super) fn field_node(
    field: &FormField,
    actions: &mut Vec<ApplicationAction>,
    device: bool,
) -> Result<LoweredNode, SemanticPresentationRefusal> {
    let component = match &field.kind {
        FieldKind::Text if !device => ApplicationComponent::TextInput,
        FieldKind::TextArea if !device => ApplicationComponent::TextArea,
        FieldKind::Select { options } if !options.is_empty() => ApplicationComponent::Select,
        _ if device => return Err(SemanticPresentationRefusal::InvalidDeviceChoice),
        _ => return Err(SemanticPresentationRefusal::InvalidField),
    };
    let (label, state) = availability_label(&field.input_action)?;
    let text = titled(&field.label, &label);
    let index = (state == ApplicationNodeState::Ready)
        .then(|| admit_action(&field.input_action, actions))
        .transpose()?;
    Ok((
        component,
        text,
        field.value.clone(),
        field.value_capacity,
        index,
        state,
    ))
}

pub(super) fn choice_node(
    option: &ChoiceOption,
    multiplicity: ChoiceMultiplicity,
    actions: &mut Vec<ApplicationAction>,
) -> Result<LoweredNode, SemanticPresentationRefusal> {
    if option.change_action.event != crate::ApplicationEventKind::Change {
        return Err(SemanticPresentationRefusal::InvalidChoiceGroup);
    }
    let (_, state) = availability_label(&option.change_action)?;
    let action = (state == ApplicationNodeState::Ready)
        .then(|| admit_action(&option.change_action, actions))
        .transpose()?;
    Ok((
        match multiplicity {
            ChoiceMultiplicity::Independent => ApplicationComponent::IndependentChoice,
            ChoiceMultiplicity::Exclusive => ApplicationComponent::ExclusiveChoice,
        },
        option.identity.clone(),
        option.selected.to_string(),
        5,
        action,
        state,
    ))
}

fn availability_label(
    action: &SemanticAction,
) -> Result<(String, ApplicationNodeState), SemanticPresentationRefusal> {
    match &action.availability {
        ActionAvailability::Available => Ok((action.label.clone(), ApplicationNodeState::Ready)),
        ActionAvailability::Busy { detail } if !detail.is_empty() => Ok((
            format!("{} — busy: {detail}", action.label),
            ApplicationNodeState::Busy,
        )),
        ActionAvailability::Unavailable { detail } if !detail.is_empty() => Ok((
            format!("{} — unavailable: {detail}", action.label),
            ApplicationNodeState::Unavailable,
        )),
        _ => Err(SemanticPresentationRefusal::InvalidActionAvailability),
    }
}

fn admit_action(
    action: &SemanticAction,
    actions: &mut Vec<ApplicationAction>,
) -> Result<u8, SemanticPresentationRefusal> {
    if let Some(index) = actions
        .iter()
        .position(|candidate| candidate.id == action.identity && candidate.event == action.event)
    {
        return Ok(index as u8);
    }
    let index = u8::try_from(actions.len()).map_err(|_| {
        SemanticPresentationRefusal::ApplicationView(ApplicationViewRefusal::TooManyActions)
    })?;
    actions.push(ApplicationAction {
        id: action.identity.clone(),
        event: action.event,
    });
    Ok(index)
}

pub(super) fn titled(title: &str, detail: &str) -> String {
    if title.is_empty() {
        detail.into()
    } else if detail.is_empty() {
        title.into()
    } else {
        format!("{title} — {detail}")
    }
}
