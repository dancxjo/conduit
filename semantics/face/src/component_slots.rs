//! Required recipe slots borrow semantic truth from the original Face.
//! Preparation checks references; it never creates an action or dispatch path.
use crate::{
    ComponentBindingRole, ComponentRecipeRefusal as Refusal, ComponentSubjectSelectorBound, Face,
    FacePropertyValue, FaceRole, FaceSubject,
};
use alloc::{collections::BTreeSet, format};
use sha2::{Digest, Sha256};

pub(crate) fn validate_binding(
    face: &Face,
    subject: &FaceSubject,
    binding: &ComponentSubjectSelectorBound,
) -> Result<(), Refusal> {
    let role_matches = match (&subject.role, binding.role()) {
        (FaceRole::Semantic(actual), ComponentBindingRole::Semantic(expected)) => {
            actual.as_str() == expected
        }
        (FaceRole::Semantic(_), _) | (_, ComponentBindingRole::Semantic(_)) => false,
        (actual, ComponentBindingRole::Role(expected)) => {
            crate::component_recipe::existing_role_name(actual) == Some(expected.as_str())
        }
    };
    if !role_matches {
        return Err(Refusal::SlotRole);
    }
    let mut names = BTreeSet::new();
    for expected in binding.properties().iter() {
        if !names.insert(expected.name()) {
            return Err(Refusal::DuplicateBinding);
        }
        let mut properties = face.properties.iter().filter(|property| {
            property.subject == subject.identity && property.name == *expected.name()
        });
        let property = properties.next().ok_or(Refusal::SlotProperty)?;
        if properties.next().is_some() {
            return Err(Refusal::SlotProperty);
        }
        let matches = match (&property.value, expected.kind().as_str()) {
            (FacePropertyValue::Flag(_), "flag")
            | (FacePropertyValue::Count(_), "count")
            | (FacePropertyValue::Signed(_), "signed")
            | (FacePropertyValue::Text(_), "text")
            | (FacePropertyValue::Identity(_), "identity") => expected.value_kind().is_empty(),
            (FacePropertyValue::TypedValue { contract, .. }, "typed") => {
                contract.value_kind.as_str() == expected.value_kind()
            }
            _ => false,
        };
        if !matches {
            return Err(Refusal::SlotProperty);
        }
    }
    let mut actions = BTreeSet::new();
    for expected in binding.actions().iter() {
        if !actions.insert(expected.identity()) {
            return Err(Refusal::DuplicateBinding);
        }
        let action = face
            .actions
            .iter()
            .find(|action| {
                action.identity == *expected.identity()
                    && action.target == subject.identity
                    && action.intent == *expected.intent()
            })
            .ok_or(Refusal::SlotAction)?;
        if action.arguments.len() != expected.arguments().len() {
            return Err(Refusal::SlotAction);
        }
        for (actual, argument) in action.arguments.iter().zip(expected.arguments().iter()) {
            let digest = Sha256::digest(actual.contract.identity_bytes());
            if actual.name != *argument.name()
                || actual.contract.value_kind.as_str() != argument.value_kind()
                || format!("{digest:x}") != *argument.contract_digest()
            {
                return Err(Refusal::SlotAction);
            }
        }
    }
    Ok(())
}
