//! Transactional installation of declared, source-owned glyph families.
use super::PackageExportCatalog;
use crate::prelude::*;
use crate::{
    ExpressionSyntax, StructuredExpressionField, SyntaxCheckDiagnostic, TypedLiteralBranch,
    TypedLiteralDelimiter, TypedLiteralFamily, TypedLiteralLexicalPolicy,
};

impl PackageExportCatalog {
    /// Resolve declared metadata against installed ordinary constructor and Type
    /// truth. This installs no payload parser, callback, offer or executable Gear.
    pub fn install_shipped_glyph_notations(
        &self,
        startup: &mut crate::StartupCatalog,
        profile: &crate::ProfileCatalog,
    ) -> Result<Vec<String>, SyntaxCheckDiagnostic> {
        let mut staged = startup.clone();
        let mut shipped = Vec::new();
        for (path, (syntax, origin)) in &self.glyph_notation_exports {
            let family = source_family(&syntax.metadata.syntax, origin.clone(), &staged)?;
            staged
                .insert_typed_literal_family(path.clone(), family, profile)
                .map_err(|message| failure(&syntax.metadata.syntax, message))?;
            shipped.push(path.clone());
        }
        *startup = staged;
        Ok(shipped)
    }
}

fn source_family(
    metadata: &ExpressionSyntax,
    origin: crate::TypedLiteralFamilyOrigin,
    startup: &crate::StartupCatalog,
) -> Result<TypedLiteralFamily, SyntaxCheckDiagnostic> {
    let fields = record(metadata, &["revision", "branches"])?;
    let revision = text(field(fields, "revision"))?;
    let branches_source = field(fields, "branches");
    let ExpressionSyntax::Collection { values, .. } = branches_source else {
        return Err(failure(
            branches_source,
            "glyph notation branches must be a finite collection",
        ));
    };
    if values.is_empty() || values.len() > crate::MAXIMUM_TYPED_LITERAL_BRANCHES {
        return Err(failure(
            branches_source,
            "glyph notation needs one to four declared branches",
        ));
    }
    let mut branches = Vec::with_capacity(values.len());
    for value in values {
        let fields = record(
            value,
            &[
                "delimiter",
                "lexical-policy",
                "parser",
                "constructor",
                "constructor-revision",
                "result",
                "maximum-payload-bytes",
            ],
        )?;
        let delimiter = match text(field(fields, "delimiter"))?.as_str() {
            "square" => TypedLiteralDelimiter::Square,
            "slash" => TypedLiteralDelimiter::Slash,
            "angle" => TypedLiteralDelimiter::Angle,
            "double-square" => TypedLiteralDelimiter::DoubleSquare,
            _ => {
                return Err(failure(
                    field(fields, "delimiter"),
                    "undeclared language delimiter pair",
                ))
            }
        };
        let lexical_policy = match text(field(fields, "lexical-policy"))?.as_str() {
            "raw-unicode" => TypedLiteralLexicalPolicy::RawUnicode,
            "portable-pattern" => TypedLiteralLexicalPolicy::PortablePattern,
            _ => {
                return Err(failure(
                    field(fields, "lexical-policy"),
                    "unknown reviewed lexical policy",
                ))
            }
        };
        let result_source = field(fields, "result");
        let result = text(result_source)?;
        let kind = crate::value_type::canonical_value_kind(&result);
        if startup.structured_type(&result).is_none()
            && startup.value_kind_alias(&result).is_none()
            && conduit_core::primitive_info_kind(kind.as_str()).is_none()
        {
            return Err(failure(
                result_source,
                "glyph notation result Type is not installed",
            ));
        }
        let result_type = crate::value_type::checked_value_type(&result, startup)
            .map_err(|_| failure(result_source, "invalid checked notation result Type"))?;
        let limit_source = field(fields, "maximum-payload-bytes");
        let ExpressionSyntax::Atomic(limit) = limit_source else {
            return Err(failure(limit_source, "payload bound must be a literal U16"));
        };
        let maximum_payload_bytes = limit
            .text
            .parse::<u16>()
            .map_err(|_| failure(limit_source, "payload bound must be a literal U16"))?
            as usize;
        branches.push(TypedLiteralBranch {
            delimiter,
            lexical_policy,
            result_type,
            maximum_payload_bytes,
            parser_contract: text(field(fields, "parser"))?,
            constructor_kind: conduit_core::KindId::from(text(field(fields, "constructor"))?),
            constructor_revision: conduit_core::KindIdentity::from(text(field(
                fields,
                "constructor-revision",
            ))?),
        });
    }
    let family = TypedLiteralFamily {
        revision,
        origin,
        branches,
    };
    family
        .identity_bytes()
        .map_err(|message| failure(metadata, message))?;
    Ok(family)
}

fn record<'a>(
    value: &'a ExpressionSyntax,
    names: &[&str],
) -> Result<&'a [StructuredExpressionField], SyntaxCheckDiagnostic> {
    let ExpressionSyntax::Record { fields, .. } = value else {
        return Err(failure(value, "glyph notation metadata must be a record"));
    };
    if fields.len() != names.len()
        || names.iter().any(|name| {
            fields
                .iter()
                .filter(|field| field.name.text == *name && !field.punned)
                .count()
                != 1
        })
    {
        return Err(failure(
            value,
            "glyph notation record has missing, duplicate or unknown metadata fields",
        ));
    }
    Ok(fields)
}

fn field<'a>(fields: &'a [StructuredExpressionField], name: &str) -> &'a ExpressionSyntax {
    &fields
        .iter()
        .find(|field| field.name.text == name)
        .expect("validated exact fields")
        .value
}

fn text(value: &ExpressionSyntax) -> Result<String, SyntaxCheckDiagnostic> {
    let ExpressionSyntax::Atomic(atom) = value else {
        return Err(failure(value, "notation identity must be literal Text"));
    };
    crate::text_value::parse_quoted_text_mapped(&atom.text, 256, |_, _| {})
        .ok_or_else(|| failure(value, "notation identity must be finite quoted Text"))
}

fn failure(value: &ExpressionSyntax, message: impl Into<String>) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-062",
        span: value.span(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
