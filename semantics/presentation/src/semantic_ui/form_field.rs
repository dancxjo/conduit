//! Portable field values and labels. Renderers preserve exact option identities.

use super::SemanticAction;
use alloc::{string::String, vec::Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectOption {
    pub identity: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Text,
    Select { options: Vec<String> },
    NamedSelect { options: Vec<SelectOption> },
    TextArea,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormField {
    pub label: String,
    pub help: String,
    pub error: Option<String>,
    pub value: String,
    pub value_capacity: u32,
    pub input_action: SemanticAction,
    pub kind: FieldKind,
}
