use crate::prelude::*;
use crate::{CanonicalStartupValue, ExpressionSyntax, Span, SpannedText, SyntaxCheckDiagnostic};
use conduit_core::{
    StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CanonicalStructuredStartupValue {
    value_type: StructuredInfoType,
    node: CanonicalStructuredStartupNode,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CanonicalStructuredStartupNode {
    Literal {
        canonical: Vec<u8>,
    },
    Parameter(String),
    Collection(Vec<CanonicalStructuredStartupValue>),
    Record(Vec<CanonicalStructuredStartupField>),
    Variant {
        tag: String,
        payload: Box<CanonicalStructuredStartupValue>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalStructuredStartupField {
    name: String,
    value: CanonicalStructuredStartupValue,
}

impl CanonicalStructuredStartupValue {
    pub fn value_type(&self) -> &StructuredInfoType {
        &self.value_type
    }

    pub fn canonical_identity(&self) -> String {
        let mut identity = String::from("structured-startup-v1");
        push_bytes(
            &mut identity,
            &self
                .value_type
                .canonical_bytes()
                .expect("a constructed structured type remains within its canonical bound"),
        );
        push_node(&mut identity, &self.node);
        identity
    }

    pub fn try_concrete(&self) -> Option<StructuredInfoValue> {
        concrete(self).ok()
    }

    pub(crate) fn satisfies_concrete_bounds(&self) -> bool {
        self.value_type
            .canonical_bytes()
            .map(|value_type| value_type.len() + node_encoded_size(&self.node))
            .is_ok_and(|size| size <= conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES)
    }
}

pub(crate) fn check_structured_expression(
    expression: &ExpressionSyntax,
    expected: &StructuredInfoType,
    resolve_atomic: &mut dyn FnMut(
        &SpannedText,
        &StructuredInfoType,
    ) -> Result<CanonicalStartupValue, SyntaxCheckDiagnostic>,
) -> Result<CanonicalStructuredStartupValue, SyntaxCheckDiagnostic> {
    match expression {
        ExpressionSyntax::Atomic(atomic) => match resolve_atomic(atomic, expected)? {
            CanonicalStartupValue::Literal(value) => {
                let StructuredInfoTypeShape::Leaf(kind) = expected.shape() else {
                    return Err(structured_diagnostic(
                            atomic.span,
                            "an atomic literal cannot satisfy a structured record, variant, or collection",
                        ));
                };
                if value.len() > conduit_core::MAXIMUM_STRUCTURED_LEAF_BYTES {
                    return Err(structured_diagnostic(
                        atomic.span,
                        "structured leaf literal exceeds the finite byte limit",
                    ));
                }
                let canonical = canonical_leaf_literal(kind.as_str(), &value, atomic.span)?;
                Ok(CanonicalStructuredStartupValue {
                    value_type: expected.clone(),
                    node: CanonicalStructuredStartupNode::Literal { canonical },
                })
            }
            CanonicalStartupValue::FormParameter(name) => Ok(CanonicalStructuredStartupValue {
                value_type: expected.clone(),
                node: CanonicalStructuredStartupNode::Parameter(name),
            }),
            CanonicalStartupValue::Structured(value) => {
                if value.value_type != *expected {
                    return Err(structured_diagnostic(
                        atomic.span,
                        "structured local or parameter has an incompatible exact type",
                    ));
                }
                Ok(value)
            }
            CanonicalStartupValue::PoolReference(_) => Err(structured_diagnostic(
                atomic.span,
                "a shared-pool reference cannot be a structured data value",
            )),
        },
        ExpressionSyntax::Collection { values, span } => {
            let StructuredInfoTypeShape::Collection { element, length } = expected.shape() else {
                return Err(structured_diagnostic(
                    *span,
                    "collection literal is incompatible with the declared startup type",
                ));
            };
            if values.len() != usize::from(length) {
                return Err(structured_diagnostic(
                    *span,
                    &format!(
                        "collection literal has {} items but the exact type requires {length}",
                        values.len()
                    ),
                ));
            }
            let checked = values
                .iter()
                .map(|value| check_structured_expression(value, element, resolve_atomic))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(CanonicalStructuredStartupValue {
                value_type: expected.clone(),
                node: CanonicalStructuredStartupNode::Collection(checked),
            })
        }
        ExpressionSyntax::Record { fields, span } => {
            let StructuredInfoTypeShape::Record {
                fields: expected_fields,
                ..
            } = expected.shape()
            else {
                return Err(structured_diagnostic(
                    *span,
                    "record literal is incompatible with the declared startup type",
                ));
            };
            let mut fields = fields.iter().collect::<Vec<_>>();
            fields.sort_by(|left, right| left.name.text.cmp(&right.name.text));
            for pair in fields.windows(2) {
                if pair[0].name.text == pair[1].name.text {
                    return Err(structured_diagnostic(
                        pair[1].name.span,
                        &format!("duplicate structured field '{}'", pair[1].name.text),
                    ));
                }
            }
            for field in &fields {
                if !expected_fields
                    .iter()
                    .any(|expected| expected.name() == field.name.text)
                {
                    return Err(structured_diagnostic(
                        field.name.span,
                        &format!("unknown structured field '{}'", field.name.text),
                    ));
                }
            }
            if fields.len() != expected_fields.len() {
                let missing = expected_fields
                    .iter()
                    .find(|expected| {
                        !fields
                            .iter()
                            .any(|field| field.name.text == expected.name())
                    })
                    .expect("unequal exact field sets have one missing member");
                return Err(structured_diagnostic(
                    *span,
                    &format!("structured record is missing field '{}'", missing.name()),
                ));
            }
            let checked = fields
                .into_iter()
                .zip(expected_fields)
                .map(|(field, expected_field)| {
                    debug_assert_eq!(field.name.text, expected_field.name());
                    Ok(CanonicalStructuredStartupField {
                        name: field.name.text.clone(),
                        value: check_structured_expression(
                            &field.value,
                            expected_field.value_type(),
                            resolve_atomic,
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, SyntaxCheckDiagnostic>>()?;
            Ok(CanonicalStructuredStartupValue {
                value_type: expected.clone(),
                node: CanonicalStructuredStartupNode::Record(checked),
            })
        }
        ExpressionSyntax::Variant { tag, payload, span } => {
            let StructuredInfoTypeShape::Variant { cases, .. } = expected.shape() else {
                return Err(structured_diagnostic(
                    *span,
                    "tagged variant literal is incompatible with the declared startup type",
                ));
            };
            let case = cases
                .iter()
                .find(|case| case.tag() == tag.text)
                .ok_or_else(|| {
                    structured_diagnostic(
                        tag.span,
                        &format!("unknown structured variant tag '{}'", tag.text),
                    )
                })?;
            let payload =
                check_structured_expression(payload, case.payload_type(), resolve_atomic)?;
            Ok(CanonicalStructuredStartupValue {
                value_type: expected.clone(),
                node: CanonicalStructuredStartupNode::Variant {
                    tag: tag.text.clone(),
                    payload: Box::new(payload),
                },
            })
        }
    }
}

fn concrete(value: &CanonicalStructuredStartupValue) -> Result<StructuredInfoValue, ()> {
    match &value.node {
        CanonicalStructuredStartupNode::Literal { canonical, .. } => {
            StructuredInfoValue::leaf(value.value_type.clone(), canonical.clone()).map_err(|_| ())
        }
        CanonicalStructuredStartupNode::Parameter(_) => Err(()),
        CanonicalStructuredStartupNode::Collection(values) => StructuredInfoValue::collection(
            value.value_type.clone(),
            values.iter().map(concrete).collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|_| ()),
        CanonicalStructuredStartupNode::Record(fields) => StructuredInfoValue::record(
            value.value_type.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(field.name.clone(), concrete(&field.value)?)
                        .map_err(|_| ())
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|_| ()),
        CanonicalStructuredStartupNode::Variant { tag, payload } => {
            StructuredInfoValue::variant(value.value_type.clone(), tag.clone(), concrete(payload)?)
                .map_err(|_| ())
        }
    }
}

fn node_encoded_size(node: &CanonicalStructuredStartupNode) -> usize {
    match node {
        CanonicalStructuredStartupNode::Literal { canonical, .. } => 1 + 4 + canonical.len(),
        CanonicalStructuredStartupNode::Parameter(value) => 1 + 4 + value.len(),
        CanonicalStructuredStartupNode::Collection(values) => {
            1 + 4
                + values
                    .iter()
                    .map(|value| node_encoded_size(&value.node))
                    .sum::<usize>()
        }
        CanonicalStructuredStartupNode::Record(fields) => {
            1 + 4
                + fields
                    .iter()
                    .map(|field| 4 + field.name.len() + node_encoded_size(&field.value.node))
                    .sum::<usize>()
        }
        CanonicalStructuredStartupNode::Variant { tag, payload } => {
            1 + 4 + tag.len() + node_encoded_size(&payload.node)
        }
    }
}

fn push_node(identity: &mut String, node: &CanonicalStructuredStartupNode) {
    match node {
        CanonicalStructuredStartupNode::Literal { canonical, .. } => {
            identity.push('l');
            push_bytes(identity, canonical);
        }
        CanonicalStructuredStartupNode::Parameter(name) => {
            identity.push('p');
            push_text(identity, name);
        }
        CanonicalStructuredStartupNode::Collection(values) => {
            identity.push('c');
            push_text(identity, &values.len().to_string());
            for value in values {
                push_node(identity, &value.node);
            }
        }
        CanonicalStructuredStartupNode::Record(fields) => {
            identity.push('r');
            push_text(identity, &fields.len().to_string());
            for field in fields {
                push_text(identity, &field.name);
                push_node(identity, &field.value.node);
            }
        }
        CanonicalStructuredStartupNode::Variant { tag, payload } => {
            identity.push('v');
            push_text(identity, tag);
            push_node(identity, &payload.node);
        }
    }
}

fn push_text(identity: &mut String, value: &str) {
    identity.push_str(&value.len().to_string());
    identity.push(':');
    identity.push_str(value);
}

fn push_bytes(identity: &mut String, value: &[u8]) {
    identity.push_str(&value.len().to_string());
    identity.push(':');
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in value {
        identity.push(char::from(HEX[usize::from(byte >> 4)]));
        identity.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}

fn canonical_leaf_literal(
    kind: &str,
    literal: &str,
    span: Span,
) -> Result<Vec<u8>, SyntaxCheckDiagnostic> {
    if kind == conduit_core::QUANTITY_INFO_ID {
        return conduit_core::Quantity::parse_form_literal(literal)
            .map(|quantity| quantity.encode().to_vec())
            .map_err(|refusal| {
                structured_diagnostic(
                    span,
                    &format!(
                        "literal '{literal}' is incompatible with exact leaf kind '{kind}': {refusal:?}"
                    ),
                )
            });
    }
    let valid = match kind {
        "value/text@1" => crate::text_value::parse_quoted_text(literal).is_some(),
        "value/count@1" => literal.parse::<u64>().is_ok(),
        "value/bool@1" => matches!(literal, "true" | "false"),
        "value/scalar@1" => is_scalar_literal(literal),
        _ => true,
    };
    if valid {
        Ok(literal.as_bytes().to_vec())
    } else {
        Err(structured_diagnostic(
            span,
            &format!("literal '{literal}' is incompatible with exact leaf kind '{kind}'"),
        ))
    }
}

fn is_scalar_literal(value: &str) -> bool {
    let (negative, value) = value
        .strip_prefix('-')
        .map_or((false, value), |value| (true, value));
    let mut parts = value.split('.');
    let whole = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.is_none_or(|digits| {
            !digits.is_empty()
                && digits.len() <= 6
                && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
        || parts.next().is_some()
    {
        return false;
    }
    let Some(whole) = parse_decimal_magnitude(whole) else {
        return false;
    };
    let fraction_digits = fraction.unwrap_or_default();
    let fraction = if fraction_digits.is_empty() {
        0
    } else if let Some(value) = parse_decimal_magnitude(fraction_digits) {
        value
    } else {
        return false;
    };
    let Some(magnitude) = whole
        .checked_mul(conduit_core::Scalar::SCALE as u64)
        .and_then(|whole| {
            whole.checked_add(fraction * 10_u64.pow((6 - fraction_digits.len()) as u32))
        })
    else {
        return false;
    };
    if negative {
        magnitude <= (i64::MAX as u64) + 1
    } else {
        magnitude <= i64::MAX as u64
    }
}

fn parse_decimal_magnitude(value: &str) -> Option<u64> {
    value.bytes().try_fold(0_u64, |magnitude, digit| {
        magnitude
            .checked_mul(10)?
            .checked_add(u64::from(digit - b'0'))
    })
}

fn structured_diagnostic(span: Span, detail: &str) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-051",
        span,
        message: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::is_scalar_literal;

    #[test]
    fn fixed_six_decimal_scalar_literals_refuse_precision_and_range_overflow() {
        assert!(is_scalar_literal("0"));
        assert!(is_scalar_literal("-9223372036854.775808"));
        assert!(is_scalar_literal("9223372036854.775807"));
        assert!(!is_scalar_literal("1.0000001"));
        assert!(!is_scalar_literal("9223372036854.775808"));
        assert!(!is_scalar_literal("-9223372036854.775809"));
    }
}
