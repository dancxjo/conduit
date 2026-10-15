//! Preserve an already checked constructor result in the ordinary startup IR.
use super::*;
use conduit_core::StructuredInfoValueShape;

impl CanonicalStructuredStartupValue {
    /// Consume this startup value through the same validating Core constructors.
    /// Unresolved parameters and invalid values refuse exactly as `try_concrete`.
    pub fn into_concrete(self) -> Option<StructuredInfoValue> {
        owned_concrete(self).ok()
    }

    /// Move an admitted Core value into the identical startup representation.
    /// This avoids retaining a second recursive Type/value tree during installation.
    pub fn from_owned_checked_value(value: StructuredInfoValue) -> Self {
        use conduit_core::OwnedStructuredInfoValueShape as Shape;
        let (value_type, shape) = value.into_parts();
        let node = match shape {
            Shape::Leaf(canonical) => CanonicalStructuredStartupNode::Literal { canonical },
            Shape::Collection(values) => CanonicalStructuredStartupNode::Collection(
                values
                    .into_iter()
                    .map(Self::from_owned_checked_value)
                    .collect(),
            ),
            Shape::Record(fields) => CanonicalStructuredStartupNode::Record(
                fields
                    .into_iter()
                    .map(|field| {
                        let (name, value) = field.into_parts();
                        CanonicalStructuredStartupField {
                            name,
                            value: Self::from_owned_checked_value(value),
                        }
                    })
                    .collect(),
            ),
            Shape::Variant { tag, payload } => CanonicalStructuredStartupNode::Variant {
                tag,
                payload: Box::new(Self::from_owned_checked_value(*payload)),
            },
        };
        Self { value_type, node }
    }

    /// Converts an already constructed, bounded Core value without reparsing
    /// Source or inferring a different nominal Type from its representation.
    pub fn from_checked_value(value: &StructuredInfoValue) -> Self {
        let node = match value.shape() {
            StructuredInfoValueShape::Leaf(bytes) => CanonicalStructuredStartupNode::Literal {
                canonical: bytes.to_vec(),
            },
            StructuredInfoValueShape::Collection(values) => {
                CanonicalStructuredStartupNode::Collection(
                    values.iter().map(Self::from_checked_value).collect(),
                )
            }
            StructuredInfoValueShape::Record(fields) => CanonicalStructuredStartupNode::Record(
                fields
                    .iter()
                    .map(|field| CanonicalStructuredStartupField {
                        name: field.name().into(),
                        value: Self::from_checked_value(field.value()),
                    })
                    .collect(),
            ),
            StructuredInfoValueShape::Variant { tag, payload } => {
                CanonicalStructuredStartupNode::Variant {
                    tag: tag.into(),
                    payload: Box::new(Self::from_checked_value(payload)),
                }
            }
        };
        Self {
            value_type: value.value_type().clone(),
            node,
        }
    }
}

pub(super) fn concrete(value: &CanonicalStructuredStartupValue) -> Result<StructuredInfoValue, ()> {
    concrete_with_type(&value.value_type, &value.node)
}

fn concrete_with_type(
    value_type: &StructuredInfoType,
    node: &CanonicalStructuredStartupNode,
) -> Result<StructuredInfoValue, ()> {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = value_type.shape() {
        return StructuredInfoValue::nominal(
            value_type.clone(),
            concrete_with_type(representation, node)?,
        )
        .map_err(|_| ());
    }
    match node {
        CanonicalStructuredStartupNode::Literal { canonical, .. } => {
            StructuredInfoValue::leaf(value_type.clone(), canonical.clone()).map_err(|_| ())
        }
        CanonicalStructuredStartupNode::Parameter(_) => Err(()),
        CanonicalStructuredStartupNode::Collection(values) => {
            let values = values.iter().map(concrete).collect::<Result<Vec<_>, _>>()?;
            match value_type.shape() {
                StructuredInfoTypeShape::Collection { .. } => {
                    StructuredInfoValue::collection(value_type.clone(), values)
                }
                StructuredInfoTypeShape::Sequence { .. } => {
                    StructuredInfoValue::sequence(value_type.clone(), values)
                }
                _ => Err(StructuredInfoRefusal::WrongType),
            }
            .map_err(|_| ())
        }
        CanonicalStructuredStartupNode::Record(fields) => StructuredInfoValue::record(
            value_type.clone(),
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
            StructuredInfoValue::variant(value_type.clone(), tag.clone(), concrete(payload)?)
                .map_err(|_| ())
        }
    }
}

fn owned_concrete(value: CanonicalStructuredStartupValue) -> Result<StructuredInfoValue, ()> {
    let CanonicalStructuredStartupValue { value_type, node } = value;
    if let StructuredInfoTypeShape::Nominal { representation, .. } = value_type.shape() {
        let represented_type = representation.clone();
        let represented = owned_concrete(CanonicalStructuredStartupValue {
            value_type: represented_type,
            node,
        })?;
        return StructuredInfoValue::nominal(value_type, represented).map_err(|_| ());
    }
    match node {
        CanonicalStructuredStartupNode::Literal { canonical } => {
            StructuredInfoValue::leaf(value_type, canonical).map_err(|_| ())
        }
        CanonicalStructuredStartupNode::Parameter(_) => Err(()),
        CanonicalStructuredStartupNode::Collection(values) => {
            let values = values
                .into_iter()
                .map(owned_concrete)
                .collect::<Result<Vec<_>, _>>()?;
            match value_type.shape() {
                StructuredInfoTypeShape::Collection { .. } => {
                    StructuredInfoValue::collection(value_type, values)
                }
                StructuredInfoTypeShape::Sequence { .. } => {
                    StructuredInfoValue::sequence(value_type, values)
                }
                _ => Err(StructuredInfoRefusal::WrongType),
            }
            .map_err(|_| ())
        }
        CanonicalStructuredStartupNode::Record(fields) => StructuredInfoValue::record(
            value_type,
            fields
                .into_iter()
                .map(|field| {
                    StructuredFieldValue::new(field.name, owned_concrete(field.value)?)
                        .map_err(|_| ())
                })
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(|_| ()),
        CanonicalStructuredStartupNode::Variant { tag, payload } => {
            StructuredInfoValue::variant(value_type, tag, owned_concrete(*payload)?).map_err(|_| ())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::KindId;

    #[test]
    fn checked_constructor_value_preserves_nested_nominal_type_and_exact_bytes() {
        let text = StructuredInfoType::leaf(KindId::new("Text")).unwrap();
        let leaf = StructuredInfoValue::leaf(text.clone(), "kʰæt".as_bytes().to_vec()).unwrap();
        let collection_type = StructuredInfoType::collection(text, Some(2)).unwrap();
        let collection =
            StructuredInfoValue::collection(collection_type.clone(), vec![leaf.clone(), leaf])
                .unwrap();
        let nominal_type =
            StructuredInfoType::nominal(KindId::new("test/phonetic-evidence@1"), collection_type)
                .unwrap();
        let original = StructuredInfoValue::nominal(nominal_type.clone(), collection).unwrap();
        let startup = CanonicalStructuredStartupValue::from_checked_value(&original);
        assert_eq!(startup.value_type(), &nominal_type);
        assert!(startup.satisfies_concrete_bounds());
        assert_eq!(startup.clone().into_concrete(), Some(original.clone()));
        assert_eq!(startup.try_concrete(), Some(original));
    }
    #[test]
    fn consuming_conversion_preserves_parameter_and_invalid_value_refusals() {
        let text = StructuredInfoType::leaf(KindId::new("value/text")).unwrap();
        let cases = [
            CanonicalStructuredStartupValue {
                value_type: text.clone(),
                node: CanonicalStructuredStartupNode::Parameter("caller".into()),
            },
            CanonicalStructuredStartupValue {
                value_type: text.clone(),
                node: CanonicalStructuredStartupNode::Literal {
                    canonical: vec![0xff],
                },
            },
            CanonicalStructuredStartupValue {
                value_type: text.clone(),
                node: CanonicalStructuredStartupNode::Collection(vec![]),
            },
            CanonicalStructuredStartupValue {
                value_type: StructuredInfoType::collection(text.clone(), Some(1)).unwrap(),
                node: CanonicalStructuredStartupNode::Collection(vec![]),
            },
            CanonicalStructuredStartupValue {
                value_type: StructuredInfoType::collection(text.clone(), Some(1)).unwrap(),
                node: CanonicalStructuredStartupNode::Collection(vec![
                    CanonicalStructuredStartupValue {
                        value_type: text,
                        node: CanonicalStructuredStartupNode::Parameter("nested".into()),
                    },
                ]),
            },
        ];
        for value in cases {
            assert_eq!(value.try_concrete(), None);
            assert_eq!(value.into_concrete(), None);
        }
    }

    #[test]
    fn nested_glyph_requires_exact_admitted_nominal_constructor_result() {
        let span = Span {
            start: 0,
            end: 5,
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 6,
        };
        let authored = SpannedText {
            text: "ph[k]".into(),
            span,
        };
        let literal =
            ExpressionSyntax::TypedGlyphLiteral(Box::new(crate::TypedGlyphLiteralSyntax {
                source_document_id: crate::parse_syntax_document("ph[k]").source_document_id(),
                alias: SpannedText {
                    text: "ph".into(),
                    span,
                },
                delimiter: crate::TypedLiteralDelimiter::Square,
                family_identity: [0; 32],
                authored: authored.clone(),
                raw_payload: SpannedText {
                    text: "k".into(),
                    span,
                },
                payload: "k".into(),
                case_insensitive: false,
                anchored_start: false,
                anchored_end: false,
            }));
        let text = StructuredInfoType::leaf(KindId::new("Text")).unwrap();
        let nominal =
            StructuredInfoType::nominal(KindId::new("test/transcription@1"), text.clone()).unwrap();
        let leaf = StructuredInfoValue::leaf(text.clone(), b"k".to_vec()).unwrap();
        let value = StructuredInfoValue::nominal(nominal.clone(), leaf.clone()).unwrap();
        let collection_type = StructuredInfoType::collection(nominal.clone(), Some(1)).unwrap();
        let expression = ExpressionSyntax::Collection {
            values: vec![literal.clone()],
            span,
        };
        let checked =
            check_structured_expression(&expression, &collection_type, &mut |source, expected| {
                assert_eq!(source, &authored);
                assert_eq!(expected, &nominal);
                Ok(CanonicalStartupValue::Structured(
                    CanonicalStructuredStartupValue::from_checked_value(&value),
                ))
            })
            .unwrap();
        assert_eq!(
            checked.try_concrete(),
            Some(StructuredInfoValue::collection(collection_type, vec![value]).unwrap(),)
        );
        // The representation and an unchecked spelling are never substitutes.
        assert!(
            check_structured_expression(&literal, &nominal, &mut |_, _| {
                Ok(CanonicalStartupValue::Structured(
                    CanonicalStructuredStartupValue::from_checked_value(&leaf),
                ))
            })
            .is_err()
        );
        assert!(check_structured_expression(&literal, &text, &mut |_, _| {
            Ok(CanonicalStartupValue::Literal("k".into()))
        })
        .is_err());
        assert!(
            check_structured_expression(&literal, &nominal, &mut |_, _| {
                Err(structured_diagnostic(span, "no admitted receipt"))
            })
            .is_err()
        );
    }
}
