//! Atomic text replacement and source-owned suggestions over an immutable Face.
//! Document range editing remains the separate revisioned face/editor contract.
use crate::{
    ChoiceMultiplicity, Face, FaceAction, FaceCollectionRefusal, FaceInteraction,
    FaceInteractionArgument, FaceInteractionRefusal, FacePropertyValue, FaceRole, MaskShow,
    PreparedFaceCollection, UTF8_TEXT_VALUE_KIND,
};
use alloc::{collections::BTreeSet, vec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceTextEntryRefusal {
    InvalidFace,
    Binding,
    UnsupportedPolicy,
    Suggestion,
    InvalidText,
    StaleFace,
    Collection(FaceCollectionRefusal),
    Interaction(FaceInteractionRefusal),
}
use FaceTextEntryRefusal as Refusal;

/// Availability stays separate: a disabled field may still disclose its value.
/// This borrows source bytes, never a Mask-local draft or inferred prose value.
pub fn text_entry_value(face: &Face, action_index: usize) -> Option<&str> {
    let action = face.actions.get(action_index)?;
    let [argument] = action.arguments.as_slice() else {
        return None;
    };
    if argument.contract.value_kind.as_str() != UTF8_TEXT_VALUE_KIND
        || !face
            .subjects
            .iter()
            .any(|s| s.identity == action.target && s.role == FaceRole::TextEntry)
    {
        return None;
    }
    let FacePropertyValue::Identity(binding) =
        crate::collections::property(face, &action.target, "value-change-action").ok()?
    else {
        return None;
    };
    let FacePropertyValue::TypedValue { contract, bytes } =
        crate::collections::property(face, &action.target, "current-value").ok()?
    else {
        return None;
    };
    if binding != &action.identity
        || contract != &argument.contract
        || contract.validate(bytes).is_err()
    {
        return None;
    }
    core::str::from_utf8(bytes).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSuggestionPolicy {
    Fixed,
    Free,
}

pub struct PreparedFaceTextEntry<'a> {
    face: &'a Face,
    action: &'a FaceAction,
    current: &'a str,
    suggestions: Option<PreparedFaceCollection<'a>>,
    policy: Option<TextSuggestionPolicy>,
}

pub fn prepare_face_text_entry<'a>(
    face: &'a Face,
    subject: &str,
    maximum_suggestions: usize,
) -> Result<PreparedFaceTextEntry<'a>, Refusal> {
    face.validate().map_err(|_| Refusal::InvalidFace)?;
    prepare_validated_text_entry(face, subject, maximum_suggestions)
}

pub(crate) fn prepare_validated_text_entry<'a>(
    face: &'a Face,
    subject: &str,
    maximum_suggestions: usize,
) -> Result<PreparedFaceTextEntry<'a>, Refusal> {
    let FacePropertyValue::Identity(action_id) =
        crate::collections::property(face, subject, "value-change-action")
            .map_err(|_| Refusal::Binding)?
    else {
        return Err(Refusal::Binding);
    };
    let (index, action) = face
        .actions
        .iter()
        .enumerate()
        .find(|(_, action)| &action.identity == action_id && action.target == subject)
        .ok_or(Refusal::Binding)?;
    let current = text_entry_value(face, index).ok_or(Refusal::Binding)?;
    let source = crate::collections::optional_property(face, subject, "suggestion-source")
        .map_err(|_| Refusal::Binding)?;
    let policy = crate::collections::optional_property(face, subject, "suggestion-policy")
        .map_err(|_| Refusal::Binding)?;
    let (suggestions, policy) = match (source, policy) {
        (None, None) => (None, None),
        (Some(FacePropertyValue::Identity(source)), Some(FacePropertyValue::Text(policy))) => {
            let collection =
                crate::collections::prepare_validated_collection(face, source, maximum_suggestions)
                    .map_err(Refusal::Collection)?;
            let policy = match policy.as_str() {
                "fixed" if collection.multiplicity() == Some(ChoiceMultiplicity::Exclusive) => {
                    if crate::collections::identity(face, source, "value-source")
                        .map_err(|_| Refusal::Binding)?
                        != subject
                        || crate::collections::identity(face, source, "selection-action")
                            .map_err(|_| Refusal::Binding)?
                            != action.identity
                    {
                        return Err(Refusal::Binding);
                    }
                    TextSuggestionPolicy::Fixed
                }
                "free" if collection.multiplicity().is_none() => TextSuggestionPolicy::Free,
                _ => return Err(Refusal::UnsupportedPolicy),
            };
            let mut values = BTreeSet::new();
            for item in collection.items() {
                let (contract, bytes) =
                    crate::collections::value(face, &item.identity, "choice-value")
                        .map_err(|_| Refusal::Suggestion)?;
                if contract.value_kind != action.arguments[0].contract.value_kind
                    || core::str::from_utf8(bytes).is_err()
                    || action.arguments[0].contract.validate(bytes).is_err()
                    || !values.insert(bytes)
                {
                    return Err(Refusal::Suggestion);
                }
                if policy == TextSuggestionPolicy::Free
                    && face.actions.iter().any(|a| a.target == item.identity)
                {
                    return Err(Refusal::Suggestion);
                }
            }
            (Some(collection), Some(policy))
        }
        _ => return Err(Refusal::Binding),
    };
    Ok(PreparedFaceTextEntry {
        face,
        action,
        current,
        suggestions,
        policy,
    })
}

impl<'a> PreparedFaceTextEntry<'a> {
    pub fn current(&self) -> &'a str {
        self.current
    }
    pub fn action(&self) -> &'a FaceAction {
        self.action
    }
    pub fn suggestions(&self) -> Option<&PreparedFaceCollection<'a>> {
        self.suggestions.as_ref()
    }
    pub fn policy(&self) -> Option<TextSuggestionPolicy> {
        self.policy
    }

    /// Inspecting/highlighting a candidate returns source bytes only; no proposal.
    pub fn suggestion(&self, item: &str) -> Result<&'a [u8], Refusal> {
        let collection = self.suggestions.as_ref().ok_or(Refusal::Suggestion)?;
        if !collection
            .items()
            .iter()
            .any(|candidate| candidate.identity == item)
        {
            return Err(Refusal::Suggestion);
        }
        crate::collections::value(self.face, item, "choice-value")
            .map(|(_, bytes)| bytes)
            .map_err(|_| Refusal::Suggestion)
    }

    /// Explicit submission constructs a proposal. The source owner still commits.
    pub fn submit(
        &self,
        current: &Face,
        show: &MaskShow,
        bytes: &[u8],
        sequence: u64,
    ) -> Result<FaceInteraction, Refusal> {
        if current.identity != self.face.identity || current.revision != self.face.revision {
            return Err(Refusal::StaleFace);
        }
        let argument = &self.action.arguments[0];
        if core::str::from_utf8(bytes).is_err() || argument.contract.validate(bytes).is_err() {
            return Err(Refusal::InvalidText);
        }
        if self.policy == Some(TextSuggestionPolicy::Fixed)
            && !self
                .suggestions
                .as_ref()
                .expect("admitted suggestions")
                .items()
                .iter()
                .any(|item| {
                    self.suggestion(&item.identity)
                        .is_ok_and(|value| value == bytes)
                })
        {
            return Err(Refusal::Suggestion);
        }
        FaceInteraction::new(
            current,
            show,
            &self.action.identity,
            &self.action.target,
            vec![FaceInteractionArgument {
                name: argument.name.clone(),
                value_kind: argument.contract.value_kind.as_str().into(),
                value: bytes.to_vec(),
            }],
            sequence,
        )
        .map_err(Refusal::Interaction)
    }
}
