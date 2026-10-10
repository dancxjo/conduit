//! Preserve an already checked constructor result in the ordinary startup IR.
use super::*;
use conduit_core::StructuredInfoValueShape;

impl CanonicalStructuredStartupValue {
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
        assert_eq!(startup.try_concrete(), Some(original));
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
