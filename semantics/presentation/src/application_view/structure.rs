use super::{ApplicationComponent, ApplicationViewNode, ApplicationViewRefusal};
use alloc::vec::Vec;

pub(super) fn validate(nodes: &[ApplicationViewNode]) -> Result<(), ApplicationViewRefusal> {
    for (index, node) in nodes.iter().enumerate() {
        let children = nodes
            .iter()
            .filter(|child| child.parent == Some(index as u8));
        match node.component {
            ApplicationComponent::FormField => {
                let children = children.collect::<Vec<_>>();
                let count = |component| {
                    children
                        .iter()
                        .filter(|child| child.component == component)
                        .count()
                };
                if count(ApplicationComponent::FieldLabel) != 1
                    || count(ApplicationComponent::FieldHelp) != 1
                    || count(ApplicationComponent::FieldError) > 1
                    || count(ApplicationComponent::TextInput)
                        + count(ApplicationComponent::Select)
                        + count(ApplicationComponent::TextArea)
                        != 1
                {
                    return Err(ApplicationViewRefusal::InvalidControlValue);
                }
            }
            ApplicationComponent::FieldLabel
            | ApplicationComponent::FieldHelp
            | ApplicationComponent::FieldError => {
                if node.parent.is_none_or(|parent| {
                    nodes[usize::from(parent)].component != ApplicationComponent::FormField
                }) {
                    return Err(ApplicationViewRefusal::InvalidControlValue);
                }
            }
            ApplicationComponent::Option => {
                if node.parent.is_none_or(|parent| {
                    nodes[usize::from(parent)].component != ApplicationComponent::Select
                }) {
                    return Err(ApplicationViewRefusal::InvalidControlValue);
                }
            }
            ApplicationComponent::Stepper => {
                let total = node
                    .value
                    .split_once('/')
                    .and_then(|(_, total)| total.parse::<usize>().ok())
                    .unwrap_or(0);
                if children
                    .filter(|child| child.component == ApplicationComponent::Button)
                    .count()
                    != total
                {
                    return Err(ApplicationViewRefusal::InvalidControlValue);
                }
            }
            ApplicationComponent::ChoiceGroup => {
                if children
                    .clone()
                    .filter(|child| child.component == ApplicationComponent::ChoiceLegend)
                    .count()
                    != 1
                    || !children
                        .clone()
                        .any(|child| child.component == ApplicationComponent::ChoiceLabel)
                {
                    return Err(ApplicationViewRefusal::InvalidControlValue);
                }
            }
            ApplicationComponent::ChoiceLegend => {
                if node.parent.is_none_or(|parent| {
                    nodes[usize::from(parent)].component != ApplicationComponent::ChoiceGroup
                }) {
                    return Err(ApplicationViewRefusal::InvalidControlValue);
                }
            }
            ApplicationComponent::ChoiceLabel => {
                if node.parent.is_none_or(|parent| {
                    nodes[usize::from(parent)].component != ApplicationComponent::ChoiceGroup
                }) || children
                    .filter(|child| child.component == ApplicationComponent::Checkbox)
                    .count()
                    != 1
                {
                    return Err(ApplicationViewRefusal::InvalidControlValue);
                }
            }
            ApplicationComponent::Checkbox => {
                if !matches!(node.value.as_str(), "true" | "false")
                    || node.parent.is_none_or(|parent| {
                        nodes[usize::from(parent)].component != ApplicationComponent::ChoiceLabel
                    })
                {
                    return Err(ApplicationViewRefusal::InvalidControlValue);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn valid_progress(value: &str) -> bool {
    value.split_once('/').is_some_and(|(current, total)| {
        current
            .parse::<u16>()
            .ok()
            .zip(total.parse::<u16>().ok())
            .is_some_and(|(current, total)| total > 0 && current <= total)
    })
}
