use crate::{StartupCatalog, StructuredSelectorSyntax, SyntaxCheckDiagnostic};
use conduit_core::{StructuredSelector, StructuredSelectorRefusal, UnmatchedVariantDisposition};

pub(super) fn check(
    syntax: &StructuredSelectorSyntax,
    catalog: &StartupCatalog,
) -> Result<StructuredSelector, SyntaxCheckDiagnostic> {
    let value_type_name = match syntax {
        StructuredSelectorSyntax::Field { value_type, .. }
        | StructuredSelectorSyntax::Index { value_type, .. }
        | StructuredSelectorSyntax::Variant { value_type, .. } => value_type,
    };
    let value_type = catalog
        .structured_type(&value_type_name.text)
        .cloned()
        .ok_or_else(|| diagnostic(value_type_name.span, "unknown structured selector type"))?;
    let checked = match syntax {
        StructuredSelectorSyntax::Field { field, .. } => {
            StructuredSelector::field(value_type, field.text.clone())
        }
        StructuredSelectorSyntax::Index { index, .. } => {
            let index = index.text.parse::<u16>().map_err(|_| {
                diagnostic(
                    index.span,
                    "structured selector index exceeds the finite u16 range",
                )
            })?;
            StructuredSelector::index(value_type, index)
        }
        StructuredSelectorSyntax::Variant { tag, unmatched, .. } => StructuredSelector::variant(
            value_type,
            tag.text.clone(),
            match unmatched.text.as_str() {
                "drop" => UnmatchedVariantDisposition::Drop,
                "refuse" => UnmatchedVariantDisposition::Refuse,
                _ => unreachable!("surface parser admits only exact unmatched dispositions"),
            },
        ),
    };
    checked.map_err(|refusal| diagnostic(syntax.span(), refusal_message(&refusal)))
}

fn refusal_message(refusal: &StructuredSelectorRefusal) -> &'static str {
    match refusal {
        StructuredSelectorRefusal::NotARecord => "field projection requires a record type",
        StructuredSelectorRefusal::NotACollection => "index selection requires a collection type",
        StructuredSelectorRefusal::NotAVariant => "variant selection requires a variant type",
        StructuredSelectorRefusal::UnknownField => "unknown structured selector field",
        StructuredSelectorRefusal::IndexOutOfRange => {
            "structured selector index is outside the exact collection bound"
        }
        StructuredSelectorRefusal::UnknownVariantTag => "unknown structured selector variant tag",
        StructuredSelectorRefusal::InvalidName(_)
        | StructuredSelectorRefusal::WrongInputType
        | StructuredSelectorRefusal::MalformedCheckedValue
        | StructuredSelectorRefusal::UnmatchedVariant
        | StructuredSelectorRefusal::FlowAlreadyClosed
        | StructuredSelectorRefusal::CanonicalEncodingTooLarge
        | StructuredSelectorRefusal::MalformedCanonicalEncoding => {
            "invalid finite structured selector"
        }
    }
}

fn diagnostic(span: crate::Span, message: &str) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-052",
        span,
        message: message.into(),
    }
}
