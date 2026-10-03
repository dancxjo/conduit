use alloc::{format, string::String, string::ToString, vec::Vec};
use serde::{Deserialize, Serialize};

pub const MAX_GENERATED_WORDING_CLAUSES: usize = 4;
pub const FINITE_FACE_WORDING_TEMPLATE_REVISION: &str = "presentation/finite-face-wording@1";
pub const MAX_GENERATED_WORDING_BYTES: usize = 1_024;
pub const MAX_GENERATED_WORDING_PROPOSAL_BYTES: usize = 2_048;
pub const MAX_RAW_PRESENTER_OUTPUT_BYTES: usize = 2_048;

/// A finite model choice of facts and phrasing. Every quoted value must still
/// be checked against the exact source Face before this can become speech.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedWordingProposal {
    pub source_presentation_identity: String,
    pub source_presentation_revision: u64,
    pub clauses: Vec<GeneratedWordingClause>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneratedWordingStyle {
    Direct,
    Guided,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GeneratedWordingClause {
    Text {
        index: u32,
        subject: String,
        value: String,
        style: GeneratedWordingStyle,
    },
    Property {
        index: u32,
        subject: String,
        name: String,
        value: String,
        style: GeneratedWordingStyle,
    },
    Action {
        index: u32,
        identity: String,
        name: String,
        style: GeneratedWordingStyle,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedWordingRefusal {
    StaleFace,
    InvalidExtent,
    InventedClaim,
    UnavailableAction,
}

impl GeneratedWordingProposal {
    pub fn within_bounds(&self) -> bool {
        if self.clauses.len() > MAX_GENERATED_WORDING_CLAUSES {
            return false;
        }
        let mut bytes = self.source_presentation_identity.len();
        for clause in &self.clauses {
            let fields = match clause {
                GeneratedWordingClause::Text { subject, value, .. } => {
                    [subject.len(), value.len(), 0]
                }
                GeneratedWordingClause::Property {
                    subject,
                    name,
                    value,
                    ..
                } => [subject.len(), name.len(), value.len()],
                GeneratedWordingClause::Action { identity, name, .. } => {
                    [identity.len(), name.len(), 0]
                }
            };
            for field in fields {
                let Some(next) = bytes.checked_add(field) else {
                    return false;
                };
                bytes = next;
            }
        }
        bytes <= MAX_GENERATED_WORDING_PROPOSAL_BYTES
    }

    pub fn render_exact(
        &self,
        presentation: &crate::Presentation,
    ) -> Result<String, GeneratedWordingRefusal> {
        if self.source_presentation_identity != presentation.identity.as_str()
            || self.source_presentation_revision != presentation.revision
        {
            return Err(GeneratedWordingRefusal::StaleFace);
        }
        if self.clauses.is_empty() || !self.within_bounds() {
            return Err(GeneratedWordingRefusal::InvalidExtent);
        }
        let mut output = String::new();
        for clause in &self.clauses {
            let sentence = match clause {
                GeneratedWordingClause::Text {
                    index,
                    subject,
                    value,
                    style,
                } => {
                    let text = presentation
                        .text
                        .get(*index as usize)
                        .ok_or(GeneratedWordingRefusal::InventedClaim)?;
                    if &text.subject != subject || &text.text != value {
                        return Err(GeneratedWordingRefusal::InventedClaim);
                    }
                    match style {
                        GeneratedWordingStyle::Direct => value.clone(),
                        GeneratedWordingStyle::Guided => format!("Current message: {value}"),
                    }
                }
                GeneratedWordingClause::Property {
                    index,
                    subject,
                    name,
                    value,
                    style,
                } => {
                    let property = presentation
                        .properties
                        .get(*index as usize)
                        .ok_or(GeneratedWordingRefusal::InventedClaim)?;
                    let exact_value = match &property.value {
                        crate::PresentationPropertyValue::Text(value)
                        | crate::PresentationPropertyValue::Identity(value) => value.clone(),
                        crate::PresentationPropertyValue::Count(value) => value.to_string(),
                        crate::PresentationPropertyValue::Signed(value) => value.to_string(),
                        crate::PresentationPropertyValue::Flag(value) => value.to_string(),
                        _ => return Err(GeneratedWordingRefusal::InventedClaim),
                    };
                    if &property.subject != subject
                        || &property.name != name
                        || &exact_value != value
                    {
                        return Err(GeneratedWordingRefusal::InventedClaim);
                    }
                    let subject_name = presentation
                        .subjects
                        .iter()
                        .find(|item| &item.identity == subject)
                        .ok_or(GeneratedWordingRefusal::InventedClaim)?
                        .name
                        .as_str();
                    match style {
                        GeneratedWordingStyle::Direct => {
                            format!("{subject_name}: {name} is {value}")
                        }
                        GeneratedWordingStyle::Guided => {
                            format!("For {subject_name}, {name} is {value}")
                        }
                    }
                }
                GeneratedWordingClause::Action {
                    index,
                    identity,
                    name,
                    style,
                } => {
                    let action = presentation
                        .actions
                        .get(*index as usize)
                        .ok_or(GeneratedWordingRefusal::InventedClaim)?;
                    if &action.identity != identity || &action.name != name {
                        return Err(GeneratedWordingRefusal::InventedClaim);
                    }
                    if !action.availability.is_available() {
                        return Err(GeneratedWordingRefusal::UnavailableAction);
                    }
                    match style {
                        GeneratedWordingStyle::Direct => format!("You can {name}"),
                        GeneratedWordingStyle::Guided => format!("Next, you can {name}"),
                    }
                }
            };
            if !output.is_empty() {
                output.push(' ');
            }
            output.push_str(&sentence);
            if !sentence.ends_with(['.', '!', '?']) {
                output.push('.');
            }
            if output.len() > MAX_GENERATED_WORDING_BYTES {
                return Err(GeneratedWordingRefusal::InvalidExtent);
            }
        }
        Ok(output)
    }
}
