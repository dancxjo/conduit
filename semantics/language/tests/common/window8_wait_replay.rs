//! Source-authorized availability boundary; no host-created dependency arcs.
use conduit_core::{StructuredFieldValue, StructuredInfoValue};
use conduit_language::*;
use conduit_plot::rust_binding::{validate_native_invariants, NativeRustBinding};
use conduit_plot::*;
pub struct WaitSchema {
    checked: CheckedSyntaxDocument,
}
impl WaitSchema {
    pub fn prepare() -> Self {
        let source = [
            include_str!("../../identity.conduit"),
            include_str!("../../types.conduit"),
            include_str!("../../text_revision.conduit"),
            include_str!("../../revision_lineage.conduit"),
            include_str!("../../lexical.conduit"),
            include_str!("../../parser.conduit"),
            include_str!("../../parser_beam.conduit"),
            include_str!("../../parser_scorer.conduit"),
            include_str!("../../parser_mask.conduit"),
            include_str!("../../parser_window8.conduit"),
            include_str!("../../parser_window8_continuation.conduit"),
        ]
        .join("\n");
        Self {
            checked: check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
                .unwrap(),
        }
    }
    fn record(
        &self,
        name: &str,
        fields: Vec<(&str, StructuredInfoValue)>,
    ) -> Result<StructuredInfoValue, String> {
        let ty = self
            .checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap();
        let value = StructuredInfoValue::record(
            ty.value_type.clone(),
            fields
                .into_iter()
                .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
                .collect(),
        )
        .map_err(|error| format!("{error:?}"))?;
        validate_native_invariants(&value, &ty.invariants).map_err(|error| format!("{error:?}"))?;
        Ok(value)
    }
    pub fn admit(
        &self,
        proof: &LanguageParserWindow8StateProof,
        lexical: &LanguageParserWindow8Lexical,
        basis: &LanguageParserBasis,
    ) -> Result<StructuredInfoValue, String> {
        let context = self.record(
            "LanguageParserWindow8Continuation",
            vec![
                (
                    "state",
                    proof
                        .clone()
                        .into_structured()
                        .map_err(|e| format!("{e:?}"))?,
                ),
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
            ],
        )?;
        self.record("LanguageParserWindow8Wait", vec![("context", context)])
    }
}
