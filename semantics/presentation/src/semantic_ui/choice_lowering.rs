use super::mechanism_lowering::choice_node;
use super::{ChoiceMultiplicity, ChoiceOption, SemanticPresentationRefusal};
use crate::{
    ApplicationAction, ApplicationComponent, ApplicationNodeState, ApplicationViewNode,
    ApplicationViewRefusal, MAX_APPLICATION_VIEW_NODES,
};
use alloc::{format, string::String, vec::Vec};

pub(super) fn lower_choice_group(
    parent: u8,
    source_key: &str,
    label: &str,
    multiplicity: ChoiceMultiplicity,
    options: &[ChoiceOption],
    nodes: &mut Vec<ApplicationViewNode>,
    actions: &mut Vec<ApplicationAction>,
) -> Result<(), SemanticPresentationRefusal> {
    let required = 1usize.saturating_add(options.len().saturating_mul(2));
    if nodes.len().saturating_add(required) > MAX_APPLICATION_VIEW_NODES {
        return Err(SemanticPresentationRefusal::ApplicationView(
            ApplicationViewRefusal::TooManyNodes,
        ));
    }
    if label.is_empty()
        || options.is_empty()
        || options
            .iter()
            .any(|option| option.identity.is_empty() || option.label.is_empty())
        || options.iter().enumerate().any(|(index, option)| {
            options[..index]
                .iter()
                .any(|prior| prior.identity == option.identity)
        })
        || (multiplicity == ChoiceMultiplicity::Exclusive
            && options.iter().filter(|option| option.selected).count() > 1)
    {
        return Err(SemanticPresentationRefusal::InvalidChoiceGroup);
    }

    nodes[parent as usize].text = source_key.into();
    nodes.push(ApplicationViewNode {
        parent: Some(parent),
        component: ApplicationComponent::ChoiceGroupLabel,
        key: generated_key(parent, nodes.len(), "label"),
        text: label.into(),
        value: String::new(),
        value_capacity: 0,
        action: None,
        state: ApplicationNodeState::Ready,
    });
    for (option_index, option) in options.iter().enumerate() {
        let option_parent = next_index(nodes)?;
        nodes.push(ApplicationViewNode {
            parent: Some(parent),
            component: ApplicationComponent::ChoiceOptionLabel,
            key: generated_key(parent, option_index, "option"),
            text: option.label.clone(),
            value: String::new(),
            value_capacity: 0,
            action: None,
            state: ApplicationNodeState::Ready,
        });
        let (component, text, value, value_capacity, action, state) =
            choice_node(option, multiplicity, actions)?;
        nodes.push(ApplicationViewNode {
            parent: Some(option_parent),
            component,
            key: generated_key(parent, option_index, "choice"),
            text,
            value,
            value_capacity,
            action,
            state,
        });
    }
    Ok(())
}

fn next_index(nodes: &[ApplicationViewNode]) -> Result<u8, SemanticPresentationRefusal> {
    u8::try_from(nodes.len()).map_err(|_| {
        SemanticPresentationRefusal::ApplicationView(ApplicationViewRefusal::TooManyNodes)
    })
}

fn generated_key(parent: u8, index: usize, role: &str) -> String {
    format!("n{parent}-{role}-{index}")
}
