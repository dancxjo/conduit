//! Fixture-only custody schedule: Source decides every receipt/branch.
//! This is not a wired production parser session or playback commitment API.
use super::values::*;
use conduit_core::*;
use conduit_language::*;
use conduit_plot::rust_binding::{validate_native_invariants, NativeRustBinding};
use conduit_plot::*;
struct Item {
    origin: StructuredInfoValue,
    current: StructuredInfoValue,
    history: Vec<StructuredInfoValue>,
}
pub struct Ledger<'a> {
    checked: &'a CheckedSyntaxDocument,
    first: PortableExpressionProgram,
    rebase: PortableExpressionProgram,
    branch: PortableExpressionProgram,
    items: Vec<Item>,
}
fn ordinal(value: &StructuredInfoValue) -> Result<usize, String> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err("U64 scalar".into());
    };
    let raw = u64::from_le_bytes(bytes.try_into().map_err(|_| "U64 width")?);
    usize::try_from(raw).map_err(|_| "ordinal overflow".into())
}
fn at(value: &StructuredInfoValue, index: usize) -> Result<StructuredInfoValue, String> {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        return Err("collection".into());
    };
    values
        .get(index)
        .cloned()
        .ok_or_else(|| "ordinal unavailable".into())
}
fn occurrence(lexical: &StructuredInfoValue, index: usize) -> Result<StructuredInfoValue, String> {
    Ok(field(
        &at(&field(&field(lexical, "tape"), "tokens"), index)?,
        "identity",
    ))
}
impl<'a> Ledger<'a> {
    pub fn prepare(checked: &'a CheckedSyntaxDocument) -> Self {
        Self {
            checked,
            first: checked_program(checked, "language-window8-lexical-anchor"),
            rebase: checked_program(checked, "language-window8-lexical-rebase"),
            branch: checked_program(checked, "language-window8-lexical-ledger-branch"),
            items: Vec::new(),
        }
    }
    fn admit(&self, name: &str, value: &StructuredInfoValue) -> Result<(), String> {
        validate_native_invariants(value, &native(self.checked, name).invariants)
            .map_err(|e| format!("{e:?}"))
    }
    fn anchor(
        &self,
        program: &PortableExpressionProgram,
        input: &StructuredInfoValue,
    ) -> Result<StructuredInfoValue, String> {
        let bytes = program
            .evaluate(&input.canonical_bytes().map_err(|e| format!("{e:?}"))?)
            .map_err(|e| format!("{e:?}"))?;
        let value =
            StructuredInfoValue::from_canonical_bytes(&bytes).map_err(|e| format!("{e:?}"))?;
        let anchor = record(
            native(self.checked, "LanguageParserWindow8LexicalAnchor"),
            [
                "origin_basis",
                "current_basis",
                "dependent",
                "choice",
                "occurrence",
            ]
            .into_iter()
            .map(|name| (name, field(&value, name)))
            .collect(),
        );
        self.admit("LanguageParserWindow8LexicalAnchor", &anchor)?;
        Ok(anchor)
    }
    fn custody(
        &self,
        origin: StructuredInfoValue,
        anchor: StructuredInfoValue,
        lexical: StructuredInfoValue,
    ) -> Result<StructuredInfoValue, String> {
        let value = record(
            native(self.checked, "LanguageParserWindow8LexicalCustody"),
            vec![("origin", origin), ("anchor", anchor), ("lexical", lexical)],
        );
        self.admit("LanguageParserWindow8LexicalCustody", &value)?;
        Ok(value)
    }
    fn admit_origin(&self, origin: &StructuredInfoValue) -> Result<(), String> {
        self.admit("LanguageParserWindow8StableLexicalFact", origin)?;
        let query = field(origin, "query");
        self.admit("LanguageParserWindow8FactQuery", &query)?;
        let snapshot = field(&query, "snapshot");
        self.admit("LanguageParserWindow8Snapshot", &snapshot)?;
        LanguageParserWindow8Lexical::from_structured(field(&snapshot, "lexical"))
            .map_err(|e| format!("{e:?}"))?;
        for name in ["candidate0", "candidate1", "candidate2", "candidate3"] {
            let candidate = field(&snapshot, name);
            self.admit("LanguageParserWindow8CheckedHypothesis", &candidate)?;
            LanguageParserWindow8RawHypothesis::from_structured(field(&candidate, "hypothesis"))
                .map_err(|e| format!("{e:?}"))?;
            LanguageParserWindow8StateProof::from_structured(field(&candidate, "proof"))
                .map_err(|e| format!("{e:?}"))?;
        }
        Ok(())
    }
    pub fn insert(&mut self, receipt: StructuredInfoValue) -> Result<(), String> {
        if self.items.len() == 8 {
            return Err("eight-receipt profile exhausted".into());
        }
        self.admit("LanguageParserWindow8ProtectedLexicalChoice", &receipt)?;
        let origin = field(&receipt, "fact");
        self.admit_origin(&origin)?;
        let query = field(&origin, "query");
        let index = ordinal(&field(&query, "dependent"))?;
        let choice = at(
            &field(
                &field(
                    &field(&field(&query, "snapshot"), "candidate0"),
                    "hypothesis",
                ),
                "choices",
            ),
            index,
        )?;
        let projection = record(
            native(self.checked, "LanguageParserWindow8LexicalAnchorContext"),
            vec![
                ("receipt", receipt.clone()),
                ("occurrence", occurrence(&field(&receipt, "next"), index)?),
                ("choice", choice),
            ],
        );
        self.admit("LanguageParserWindow8LexicalAnchorContext", &projection)?;
        let anchor = self.anchor(&self.first, &projection)?;
        let current = self.custody(origin.clone(), anchor, field(&receipt, "next"))?;
        self.items.push(Item {
            origin,
            current,
            history: vec![receipt],
        });
        Ok(())
    }
    pub fn rebase(
        &mut self,
        next: StructuredInfoValue,
        lineage: StructuredInfoValue,
        basis: StructuredInfoValue,
    ) -> Result<(), String> {
        LanguageParserWindow8Lexical::from_structured(next.clone())
            .map_err(|e| format!("{e:?}"))?;
        let native_lineage = LanguageTextRevisionLineage::from_structured(lineage.clone())
            .map_err(|e| format!("{e:?}"))?;
        prepare_text_revision_lineage(native_lineage.previous(), native_lineage.next())
            .map_err(|e| format!("{e:?}"))?;
        let mut prepared = Vec::new();
        for item in &self.items {
            if item.history.len() == 32 {
                return Err("32-hop profile exhausted".into());
            }
            let context = record(
                native(self.checked, "LanguageParserWindow8LexicalRebase"),
                vec![
                    ("previous", item.current.clone()),
                    (
                        "occurrence",
                        occurrence(
                            &next,
                            ordinal(&field(&field(&item.current, "anchor"), "dependent"))?,
                        )?,
                    ),
                    ("next", next.clone()),
                    ("lineage", lineage.clone()),
                    ("basis", basis.clone()),
                ],
            );
            self.admit("LanguageParserWindow8LexicalRebase", &context)?;
            let anchor = self.anchor(&self.rebase, &context)?;
            let current = self.custody(item.origin.clone(), anchor, next.clone())?;
            prepared.push((current, context));
        }
        // A refusal leaves every prior immutable receipt intact.
        for (item, (current, context)) in self.items.iter_mut().zip(prepared) {
            item.current = current;
            item.history.push(context);
        }
        Ok(())
    }
    pub fn allows(
        &self,
        dependent: u64,
        choice: u64,
        basis: StructuredInfoValue,
    ) -> Result<bool, String> {
        let ty = native(self.checked, "LanguageParserWindow8LexicalLedgerBranch");
        for item in &self.items {
            let query = record(
                ty,
                vec![
                    ("receipt", item.current.clone()),
                    ("dependent", scalar(ty, "dependent", dependent)),
                    ("choice", scalar(ty, "choice", choice)),
                    ("basis", basis.clone()),
                ],
            );
            self.admit("LanguageParserWindow8LexicalLedgerBranch", &query)?;
            let output = StructuredInfoValue::from_canonical_bytes(
                &self
                    .branch
                    .evaluate(&query.canonical_bytes().map_err(|e| format!("{e:?}"))?)
                    .map_err(|e| format!("{e:?}"))?,
            )
            .map_err(|e| format!("{e:?}"))?;
            match field(&output, "allowed").shape() {
                StructuredInfoValueShape::Leaf([1]) => {}
                StructuredInfoValueShape::Leaf([0]) => return Ok(false),
                _ => return Err("non-Boolean Source decision".into()),
            }
        }
        Ok(true)
    }
    pub fn evidence(
        &self,
    ) -> Vec<(
        StructuredInfoValue,
        StructuredInfoValue,
        Vec<StructuredInfoValue>,
    )> {
        self.items
            .iter()
            .map(|item| {
                (
                    item.origin.clone(),
                    item.current.clone(),
                    item.history.clone(),
                )
            })
            .collect()
    }
}
