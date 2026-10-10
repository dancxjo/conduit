use super::*;
use crate::{ExpressionSyntax, StructuredExpressionField};

type Result<T> = core::result::Result<T, PhysicalDeclarationDiagnostic>;

fn span(source: &str, start: usize, end: usize) -> Span {
    let (line, column) = crate::surface_lex::location(source, start);
    let (end_line, end_column) = crate::surface_lex::location(source, end);
    Span {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}

fn named(source: &str, text: &str, start: usize) -> SpannedText {
    SpannedText {
        text: text.into(),
        span: span(source, start, start + text.len()),
    }
}

fn reference(value: &ExpressionSyntax) -> Result<SpannedText> {
    match value {
        ExpressionSyntax::Binary {
            operator: crate::BinaryOperator::Divide,
            left,
            right,
            span,
        } if left.span().end + 1 == right.span().start => {
            let left = reference(left)?;
            let right = reference(right)?;
            Ok(SpannedText {
                text: alloc::format!("{}/{}", left.text, right.text),
                span: *span,
            })
        }
        ExpressionSyntax::Atomic(name)
            if !name.text.starts_with('"')
                && !name.text.is_empty()
                && !name.text.starts_with(|c: char| c.is_ascii_digit()) =>
        {
            Ok(name.clone())
        }
        _ => Err(refusal(
            value.span(),
            "expected an unquoted declaration reference",
        )),
    }
}

fn fields(value: &ExpressionSyntax) -> Result<&[StructuredExpressionField]> {
    match value {
        ExpressionSyntax::Record { fields, .. } => {
            let mut names = alloc::collections::BTreeSet::new();
            for field in fields {
                if field.punned || !names.insert(field.name.text.as_str()) {
                    return Err(refusal(
                        field.span,
                        "physical declaration fields must be explicit and unique",
                    ));
                }
            }
            Ok(fields)
        }
        _ => Err(refusal(
            value.span(),
            "expected a physical declaration record",
        )),
    }
}

fn field<'a>(values: &'a [StructuredExpressionField], name: &str) -> Option<&'a ExpressionSyntax> {
    values
        .iter()
        .find(|field| field.name.text == name)
        .map(|field| &field.value)
}

fn required<'a>(
    values: &'a [StructuredExpressionField],
    name: &str,
    at: Span,
) -> Result<&'a ExpressionSyntax> {
    field(values, name)
        .ok_or_else(|| refusal(at, alloc::format!("physical declaration requires '{name}'")))
}

fn only(values: &[StructuredExpressionField], names: &[&str]) -> Result<()> {
    for value in values {
        if !names.contains(&value.name.text.as_str()) {
            return Err(refusal(
                value.name.span,
                alloc::format!("unknown physical declaration field '{}'", value.name.text),
            ));
        }
    }
    Ok(())
}

pub(crate) fn parse_dimension_declaration(
    source: &str,
    text: &str,
    start: usize,
) -> Result<DimensionDeclarationSyntax> {
    let value = text
        .strip_prefix("dimension ")
        .ok_or_else(|| {
            refusal(
                span(source, start, start + text.len()),
                "expected dimension declaration",
            )
        })?
        .trim();
    if !crate::surface_lex::is_name(value) {
        return Err(refusal(
            span(source, start, start + text.len()),
            "dimension requires one name",
        ));
    }
    Ok(DimensionDeclarationSyntax {
        name: named(source, value, start + text.find(value).unwrap()),
        span: span(source, start, start + text.len()),
    })
}

pub(crate) fn parse_quantity_definition(
    value: &ExpressionSyntax,
) -> Result<QuantityDefinitionSyntax> {
    let values = fields(value)?;
    only(values, &["dimension", "point"])?;
    match (field(values, "dimension"), field(values, "point")) {
        (Some(dimension), None) => {
            let dimensions = if let ExpressionSyntax::Record { fields: terms, .. } = dimension {
                fields(dimension)?;
                terms
                    .iter()
                    .map(|term| DimensionPowerSyntax {
                        dimension: term.name.clone(),
                        power: term.value.clone(),
                        span: term.span,
                    })
                    .collect()
            } else {
                let dimension = reference(dimension)?;
                alloc::vec![DimensionPowerSyntax {
                    span: dimension.span,
                    power: ExpressionSyntax::Atomic(SpannedText {
                        text: "1".into(),
                        span: dimension.span
                    }),
                    dimension,
                }]
            };
            if dimensions.len() > 8 {
                return Err(refusal(
                    value.span(),
                    "quantity dimension exceeds eight terms",
                ));
            }
            Ok(QuantityDefinitionSyntax::Linear {
                dimensions,
                span: value.span(),
            })
        }
        (None, Some(point)) => Ok(QuantityDefinitionSyntax::Point {
            difference_type: reference(point)?,
            span: value.span(),
        }),
        _ => Err(refusal(
            value.span(),
            "quantity declares exactly one of dimension or point",
        )),
    }
}

pub(crate) fn parse_prefix_declaration(
    source: &str,
    text: &str,
    start: usize,
) -> Result<PrefixDeclarationSyntax> {
    let at = span(source, start, start + text.len());
    let (header, body) = text
        .split_once('=')
        .ok_or_else(|| refusal(at, "prefix declaration requires '='"))?;
    let header = header
        .strip_prefix("prefix ")
        .ok_or_else(|| refusal(at, "expected prefix GROUP SYMBOL"))?
        .trim();
    let mut words = header.split_whitespace();
    let group = words
        .next()
        .ok_or_else(|| refusal(at, "prefix requires a group"))?;
    let symbol = words
        .next()
        .ok_or_else(|| refusal(at, "prefix requires a symbol"))?;
    if words.next().is_some() || !matches!(group, "si" | "binary") {
        return Err(refusal(at, "prefix group must be si or binary"));
    }
    let group = named(source, group, start + text.find(group).unwrap());
    let symbol = named(source, symbol, start + text.find(symbol).unwrap());
    if !symbol_allowed(&symbol) {
        return Err(refusal(symbol.span, "invalid prefix symbol"));
    }
    let body = body.trim();
    let offset = start
        + text.find('=').unwrap()
        + 1
        + text[text.find('=').unwrap() + 1..].find(body).unwrap();
    let value = crate::pure_expression::parse(source, body, offset)
        .map_err(|(message, span)| refusal(span, message))?;
    let values = fields(&value)?;
    only(values, &["exponent", "alias"])?;
    let exponent = required(values, "exponent", value.span())?.clone();
    let integer = check_exact_scalar(&exponent)?.integer();
    let valid = if group.text == "si" {
        integer.is_some_and(|v| v != 0 && (-30..=30).contains(&v))
    } else {
        integer.is_some_and(|v| (1..=80).contains(&v))
    };
    if !valid {
        return Err(refusal(
            exponent.span(),
            "prefix exponent is outside its bounded arithmetic group",
        ));
    }
    let alias = field(values, "alias").map(reference).transpose()?;
    Ok(PrefixDeclarationSyntax {
        group,
        symbol,
        exponent,
        alias,
        span: at,
    })
}

fn transform(
    values: &[StructuredExpressionField],
    at: Span,
    difference: bool,
) -> Result<UnitTransformSyntax> {
    let value = required(values, "reference", at)?;
    let reference = if let ExpressionSyntax::Variant { tag, payload, span } = value {
        if tag.text != "origin" {
            return Err(refusal(*span, "named unit origin uses origin(NAME)"));
        }
        UnitReferenceSyntax::NamedOrigin {
            name: reference(payload)?,
            span: *span,
        }
    } else {
        let value = reference(value)?;
        if value.text == "origin" {
            UnitReferenceSyntax::Origin(value.span)
        } else {
            UnitReferenceSyntax::Unit(value)
        }
    };
    let scale = required(values, "scale", at)?.clone();
    check_exact_scalar(&scale)?;
    let offset = field(values, "offset").cloned();
    if difference && offset.is_some() {
        return Err(refusal(
            at,
            "a difference unit cannot declare an affine offset",
        ));
    }
    if let Some(offset) = &offset {
        check_exact_scalar(offset)?;
    }
    Ok(UnitTransformSyntax {
        reference,
        scale,
        offset,
        span: at,
    })
}

fn prefixes(value: Option<&ExpressionSyntax>) -> Result<PrefixPolicySyntax> {
    let Some(value) = value else {
        return Ok(PrefixPolicySyntax::default());
    };
    let mut policy = PrefixPolicySyntax::default();
    let items = match value {
        ExpressionSyntax::Collection { values, .. } => values.as_slice(),
        value => core::slice::from_ref(value),
    };
    if items.is_empty() {
        return Err(refusal(
            value.span(),
            "prefix policy uses none, si, binary, or [si, binary]",
        ));
    }
    for item in items {
        let name = reference(item)?;
        match name.text.as_str() {
            "none" if items.len() == 1 => {}
            "si" if !policy.si => policy.si = true,
            "binary" if !policy.binary => policy.binary = true,
            _ => return Err(refusal(name.span, "duplicate or unknown prefix policy")),
        }
    }
    Ok(policy)
}

pub(crate) fn parse_unit_declaration(
    source: &str,
    text: &str,
    start: usize,
) -> Result<UnitDeclarationSyntax> {
    let at = span(source, start, start + text.len());
    let (header, body) = text
        .split_once('=')
        .ok_or_else(|| refusal(at, "unit declaration requires '='"))?;
    let (symbol, quantity) = header
        .strip_prefix("unit ")
        .and_then(|header| header.split_once(':'))
        .ok_or_else(|| refusal(at, "expected unit SYMBOL : QuantityType"))?;
    let symbol = symbol.trim();
    let quantity = quantity.trim();
    let symbol = named(source, symbol, start + text.find(symbol).unwrap());
    if !symbol_allowed(&symbol) || !crate::surface_lex::is_name(quantity) {
        return Err(refusal(at, "invalid unit symbol or quantity Type name"));
    }
    let quantity_type = named(source, quantity, start + header.find(quantity).unwrap());
    let body = body.trim();
    let offset = start
        + text.find('=').unwrap()
        + 1
        + text[text.find('=').unwrap() + 1..].find(body).unwrap();
    let expression = crate::pure_expression::parse(source, body, offset)
        .map_err(|(message, span)| refusal(span, message))?;
    let values = fields(&expression)?;
    only(
        values,
        &["reference", "scale", "offset", "delta", "prefixes", "power"],
    )?;
    let difference = field(values, "delta")
        .map(|delta| {
            let values = fields(delta)?;
            only(values, &["quantity", "reference", "scale"])?;
            Ok(DifferenceUnitSyntax {
                quantity: reference(required(values, "quantity", delta.span())?)?,
                transform: transform(values, delta.span(), true)?,
                span: delta.span(),
            })
        })
        .transpose()?;
    let prefix_power = field(values, "power").cloned();
    if let Some(power) = &prefix_power {
        check_exact_scalar(power)?;
    }
    Ok(UnitDeclarationSyntax {
        symbol,
        quantity_type,
        transform: transform(values, expression.span(), false)?,
        difference,
        prefixes: prefixes(field(values, "prefixes"))?,
        prefix_power,
        span: at,
    })
}
