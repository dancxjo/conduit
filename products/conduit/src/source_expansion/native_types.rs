//! Authored declarations correlated with the existing checked Type representation.
use super::SourceSpan;
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape as Shape};
use conduit_plot::{CheckedSyntaxDocument, SyntaxDocument};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct NativeTypeView<'a> {
    pub name: &'a str,
    pub authored: &'a str,
    pub source_span: SourceSpan,
    pub checked_identity: &'a str,
    pub representation: Representation<'a>,
}

#[derive(Debug, Serialize)]
pub(super) struct FamilyView<'a> {
    pub name: &'a str,
    pub authored: &'a str,
    pub source_span: SourceSpan,
    pub parameters: Vec<ParameterView<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct ParameterView<'a> {
    name: &'a str,
    kind: &'static str,
    source_span: SourceSpan,
}

#[derive(Debug, Serialize)]
pub(super) struct ImportView<'a> {
    pub path: &'a str,
    pub alias: &'a str,
    pub source_span: SourceSpan,
    pub owner_sources: Vec<OwnerSourceView<'a>>,
}

#[derive(Debug, Serialize)]
pub(super) struct OwnerSourceView<'a> {
    package_content_digest: String,
    module_path: &'a str,
    source_document_id: &'a str,
    declaration_name: &'a str,
    declaration_span: SourceSpan,
}

#[derive(Debug, Serialize)]
#[serde(tag = "constraint", rename_all = "snake_case")]
pub(super) enum Representation<'a> {
    Leaf {
        kind: &'a str,
    },
    Nominal {
        identity: &'a str,
        representation: Box<Self>,
    },
    Collection {
        exact_items: u16,
        element: Box<Self>,
    },
    Sequence {
        minimum_items: u16,
        maximum_items: u16,
        element: Box<Self>,
    },
    Record {
        identity: &'a str,
        fields: Vec<Member<'a>>,
    },
    Variant {
        identity: &'a str,
        cases: Vec<Member<'a>>,
    },
}

#[derive(Debug, Serialize)]
pub(super) struct Member<'a> {
    name: &'a str,
    representation: Representation<'a>,
}

impl<'a> From<&'a StructuredInfoType> for Representation<'a> {
    fn from(value: &'a StructuredInfoType) -> Self {
        match value.shape() {
            Shape::Leaf(kind) => Self::Leaf {
                kind: kind.as_str(),
            },
            Shape::Nominal {
                schema,
                representation,
            } => Self::Nominal {
                identity: schema.as_str(),
                representation: Box::new(representation.into()),
            },
            Shape::Collection { element, length } => Self::Collection {
                exact_items: length,
                element: Box::new(element.into()),
            },
            Shape::Sequence {
                element,
                minimum_items,
                maximum_items,
            } => Self::Sequence {
                minimum_items,
                maximum_items,
                element: Box::new(element.into()),
            },
            Shape::Record { schema, fields } => Self::Record {
                identity: schema.as_str(),
                fields: fields
                    .iter()
                    .map(|field| Member {
                        name: field.name(),
                        representation: field.value_type().into(),
                    })
                    .collect(),
            },
            Shape::Variant { schema, cases } => Self::Variant {
                identity: schema.as_str(),
                cases: cases
                    .iter()
                    .map(|case| Member {
                        name: case.tag(),
                        representation: case.payload_type().into(),
                    })
                    .collect(),
            },
        }
    }
}

pub(super) fn types<'a>(
    syntax: &'a SyntaxDocument,
    checked: &'a CheckedSyntaxDocument,
) -> Result<Vec<NativeTypeView<'a>>, String> {
    let mut budget = InspectionBudget {
        nodes: 16_000,
        bytes: 1024 * 1024,
    };
    syntax
        .types
        .iter()
        .filter(|declaration| declaration.parameters.is_empty())
        .map(|declaration| {
            let value = checked
                .native_types
                .iter()
                .find(|value| value.name == declaration.name.text)
                .expect("checked authored Type");
            budget.visit(&value.value_type)?;
            Ok(NativeTypeView {
                name: &declaration.name.text,
                authored: &syntax.round_trip()[declaration.span.start..declaration.span.end],
                source_span: declaration.span.into(),
                checked_identity: value.identity.as_str(),
                representation: (&value.value_type).into(),
            })
        })
        .collect()
}

pub(super) fn families(syntax: &SyntaxDocument) -> Vec<FamilyView<'_>> {
    syntax
        .types
        .iter()
        .filter(|declaration| !declaration.parameters.is_empty())
        .map(|declaration| FamilyView {
            name: &declaration.name.text,
            authored: &syntax.round_trip()[declaration.span.start..declaration.span.end],
            source_span: declaration.span.into(),
            parameters: declaration
                .parameters
                .iter()
                .map(|parameter| ParameterView {
                    name: &parameter.name.text,
                    kind: if parameter.value_type.is_some() {
                        "Info"
                    } else {
                        "Type"
                    },
                    source_span: parameter.name.span.into(),
                })
                .collect(),
        })
        .collect()
}

pub(super) fn imports<'a>(
    syntax: &'a SyntaxDocument,
    catalog: &'a conduit_plot::StartupCatalog,
) -> Vec<ImportView<'a>> {
    syntax
        .uses
        .iter()
        .map(|declaration| ImportView {
            path: &declaration.path,
            alias: &declaration.alias.text,
            source_span: declaration.span.into(),
            owner_sources: catalog
                .native_family_sources(&declaration.path)
                .into_iter()
                .flatten()
                .map(|origin| OwnerSourceView {
                    package_content_digest: origin
                        .package_content_digest
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                    module_path: &origin.module_path,
                    source_document_id: origin.source_document_id.as_str(),
                    declaration_name: &origin.declaration_name,
                    declaration_span: origin.declaration_span.into(),
                })
                .collect(),
        })
        .collect()
}

// Bound the expanded tree before allocating each view, including repeated references.
struct InspectionBudget {
    nodes: usize,
    bytes: usize,
}
impl InspectionBudget {
    fn text(&mut self, value: &str) -> Result<(), String> {
        self.bytes = self
            .bytes
            .checked_sub(value.len())
            .ok_or_else(Self::refusal)?;
        Ok(())
    }
    fn refusal() -> String {
        "Source Type inspection exceeds its 16000-node or 1 MiB metadata profile".into()
    }
    fn visit(&mut self, value: &StructuredInfoType) -> Result<(), String> {
        self.nodes = self.nodes.checked_sub(1).ok_or_else(Self::refusal)?;
        match value.shape() {
            Shape::Leaf(kind) => self.text(kind.as_str())?,
            Shape::Nominal {
                schema,
                representation,
            } => {
                self.text(schema.as_str())?;
                self.visit(representation)?;
            }
            Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
                self.visit(element)?
            }
            Shape::Record { schema, fields } => {
                self.text(schema.as_str())?;
                for field in fields {
                    self.text(field.name())?;
                    self.visit(field.value_type())?;
                }
            }
            Shape::Variant { schema, cases } => {
                self.text(schema.as_str())?;
                for case in cases {
                    self.text(case.tag())?;
                    self.visit(case.payload_type())?;
                }
            }
        }
        Ok(())
    }
}
