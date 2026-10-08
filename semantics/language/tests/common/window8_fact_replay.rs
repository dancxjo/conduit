//! Dynamic exact Source fact admission pending additive generated-type wiring.
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_language::*;
use conduit_plot::rust_binding::{validate_native_invariants, NativeRustBinding};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedNativeType, CheckedSyntaxDocument,
    StartupCatalog,
};
pub struct FactSchema {
    checked: CheckedSyntaxDocument,
}
impl FactSchema {
    pub fn prepare() -> Self {
        // Check the unchanged authored dependency closure, retaining every
        // inherited contract and law in the generated nominal Type identity.
        let source = [
            include_str!("../../types.conduit"),
            include_str!("../../identity.conduit"),
            include_str!("../../coverage.conduit"),
            include_str!("../../syntax.conduit"),
            include_str!("../../text_revision.conduit"),
            include_str!("../../revision_lineage.conduit"),
            include_str!("../../lexical.conduit"),
            include_str!("../../parser.conduit"),
            include_str!("../../parser_window8.conduit"),
            include_str!("../../parser_window8_search.conduit"),
            include_str!("../../parser_window8_facts.conduit"),
            include_str!("../../parser_beam.conduit"),
            include_str!("../../parser_scorer.conduit"),
            include_str!("../../parser_mask.conduit"),
        ]
        .join("\n");
        let checked =
            check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
        Self { checked }
    }
    fn ty(&self, name: &str) -> &CheckedNativeType {
        self.checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
    }
    fn record(
        &self,
        name: &str,
        fields: Vec<(&str, StructuredInfoValue)>,
    ) -> Result<StructuredInfoValue, String> {
        let ty = self.ty(name);
        let fields = fields
            .into_iter()
            .map(|(name, value)| {
                StructuredFieldValue::new(name, value).map_err(|e| format!("{e:?}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let value = StructuredInfoValue::record(ty.value_type.clone(), fields)
            .map_err(|e| format!("{e:?}"))?;
        validate_native_invariants(&value, &ty.invariants).map_err(|e| format!("{e:?}"))?;
        Ok(value)
    }
    pub fn lexical_fact(
        &self,
        lexical: &LanguageParserWindow8Lexical,
        basis: &LanguageParserBasis,
        beam: &LanguageParserWindow8RawBeam,
        proofs: &[LanguageParserWindow8StateProof; 4],
        dependent: u64,
    ) -> Result<StructuredInfoValue, String> {
        let mut hypotheses = Vec::new();
        for (hypothesis, proof) in [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ]
        .into_iter()
        .zip(proofs)
        {
            hypotheses.push(self.record(
                "LanguageParserWindow8CheckedHypothesis",
                vec![
                        (
                            "hypothesis",
                            hypothesis
                                .clone()
                                .into_structured()
                                .map_err(|e| format!("{e:?}"))?,
                        ),
                        (
                            "proof",
                            proof
                                .clone()
                                .into_structured()
                                .map_err(|e| format!("{e:?}"))?,
                        ),
                    ],
            )?);
        }
        let snapshot = self.record(
            "LanguageParserWindow8Snapshot",
            vec![
                (
                    "lexical",
                    lexical
                        .clone()
                        .into_structured()
                        .map_err(|e| format!("{e:?}"))?,
                ),
                (
                    "basis",
                    basis
                        .clone()
                        .into_structured()
                        .map_err(|e| format!("{e:?}"))?,
                ),
                ("candidate0", hypotheses[0].clone()),
                ("candidate1", hypotheses[1].clone()),
                ("candidate2", hypotheses[2].clone()),
                ("candidate3", hypotheses[3].clone()),
            ],
        )?;
        let query_ty = self.ty("LanguageParserWindow8FactQuery");
        let StructuredInfoTypeShape::Record { fields, .. } = query_ty.value_type.shape() else {
            panic!("query")
        };
        let field = fields
            .iter()
            .find(|field| field.name() == "dependent")
            .unwrap();
        let ordinal =
            StructuredInfoValue::leaf(field.value_type().clone(), dependent.to_le_bytes().to_vec())
                .map_err(|e| format!("{e:?}"))?;
        let query = self.record(
            "LanguageParserWindow8FactQuery",
            vec![("snapshot", snapshot), ("dependent", ordinal)],
        )?;
        self.record(
            "LanguageParserWindow8StableLexicalFact",
            vec![("query", query)],
        )
    }
}

#[test]
fn fact_source_retains_exact_generated_types_and_ordered_laws() {
    use conduit_plot::rust_binding::PreparedNativeRustBinding;
    let schema = FactSchema::prepare();
    for (name, descriptor) in [
        (
            "LanguageParserWindow8CheckedHypothesis",
            LanguageParserWindow8CheckedHypothesis::PREPARED_DESCRIPTOR,
        ),
        (
            "LanguageParserWindow8Snapshot",
            LanguageParserWindow8Snapshot::PREPARED_DESCRIPTOR,
        ),
        (
            "LanguageParserWindow8FactQuery",
            LanguageParserWindow8FactQuery::PREPARED_DESCRIPTOR,
        ),
        (
            "LanguageParserWindow8StableLexicalFact",
            LanguageParserWindow8StableLexicalFact::PREPARED_DESCRIPTOR,
        ),
    ] {
        let checked = schema.ty(name);
        assert!(
            checked.value_type.canonical_bytes().unwrap() == descriptor.type_bytes,
            "exact full Type differs: {name}"
        );
        assert_eq!(checked.invariants.len(), descriptor.laws.len());
        for (law, bytes) in checked.invariants.iter().zip(descriptor.laws) {
            assert_eq!(law.canonical_bytes().unwrap(), *bytes);
        }
    }
}
