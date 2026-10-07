//! Dynamic exact Source fact admission pending additive generated-type wiring.
use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_language::*;
use conduit_plot::rust_binding::{validate_native_invariants, NativeRustBinding};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedNativeType, CheckedSyntaxDocument,
    ProfileCatalog, StartupCatalog,
};
pub struct FactSchema {
    checked: CheckedSyntaxDocument,
}
impl FactSchema {
    pub fn prepare() -> Self {
        let mut startup = StartupCatalog::new();
        install_linguistics_catalogs(&mut startup, &mut ProfileCatalog::new()).unwrap();
        let checked = check_syntax_document(
            &parse_syntax_document(include_str!("../../parser_window8_facts.conduit")),
            &startup,
        )
        .unwrap();
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
