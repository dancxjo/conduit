//! Exact named Language record construction; every result still needs Native law admission.
use super::{storage::*, *};
use alloc::vec::Vec;
use conduit_core::*;
use conduit_plot::CheckedSyntaxDocument;
pub(super) const TYPES: &[&str] = &[
    "LanguageParserScorerBeam",
    "LanguageAnalysisInitialRawBeam",
    "LanguageParserState",
    "LanguageParserScorerHypothesis",
    "LanguageParserScoredClass",
    "LanguageParserScoredProposal",
    "LanguageParserRequest",
    "LanguageParserResult",
    "LanguageParserScorerLegality",
    "LanguageParserScorerRanking",
    "LanguageParserScorerBranch",
    "LanguageParserScorerBranchResult",
    "LanguageParserScorerExpansion",
    "LanguageParserScorerRawHypothesis",
    "LanguageParserScorerMerge",
    "LanguageParserScorerChoiceQuery",
    "LanguageParserScorerFeatures",
];
const RECORDS: &[&str] = &[
    "LanguageParserNumericState",
    "LanguageParserState",
    "LanguageParserHypothesis",
    "LanguageParserRawHypothesis",
    "LanguageParserScorerHypothesis",
    "LanguageParserScorerRawHypothesis",
    "LanguageParserScorerBeam",
    "LanguageParserScoredClass",
    "LanguageParserScorerLegality",
    "LanguageParserScorerBranch",
    "LanguageParserScorerExpansion",
    "LanguageParserScorerMerge",
    "LanguageParserScorerChoiceQuery",
    "LanguageParserScorerRanking",
];
struct Record {
    name: &'static str,
    fields: Vec<String>,
    composer: PreparedStructuredComposer,
}
pub(super) struct Materializer {
    records: Vec<Record>,
    state: Vec<u8>,
    parser: Vec<u8>,
    integer: PreparedStructuredComposer,
}
impl Materializer {
    pub fn prepare(document: &CheckedSyntaxDocument) -> R<Self> {
        let mut records = Vec::new();
        for name in RECORDS {
            let native = document
                .native_types
                .iter()
                .find(|t| t.name == *name)
                .ok_or(CursorRefusal::Preparation)?;
            let StructuredInfoTypeShape::Record { fields, .. } = native.value_type.shape() else {
                return Err(CursorRefusal::Preparation);
            };
            records.push(Record {
                name,
                fields: fields.iter().map(|f| String::from(f.name())).collect(),
                composer: PreparedStructuredComposer::new(&native.value_type, BYTES)
                    .map_err(|_| CursorRefusal::Preparation)?,
            });
        }
        let integer = PreparedStructuredComposer::new(
            &StructuredInfoType::leaf(kind_id("value/u64"))
                .map_err(|_| CursorRefusal::Preparation)?,
            32,
        )
        .map_err(|_| CursorRefusal::Preparation)?;
        Ok(Self {
            records,
            state: buffer(),
            parser: buffer(),
            integer,
        })
    }
    pub fn record(
        &mut self,
        name: &str,
        fields: &[(&str, ValidatedCanonicalStructuredValue<'_>)],
        output: &mut Vec<u8>,
    ) -> R<()> {
        record(&mut self.records, name, fields, output)
    }
    pub fn unsigned(&mut self, number: u64, output: &mut Vec<u8>) -> R<()> {
        put(
            output,
            self.integer
                .leaf(&number.to_le_bytes())
                .map_err(|_| CursorRefusal::Canonical)?,
        )
    }
    pub fn numeric(
        &mut self,
        state: ValidatedCanonicalStructuredValue<'_>,
        output: &mut Vec<u8>,
    ) -> R<()> {
        state_copy(
            &mut self.records,
            "LanguageParserNumericState",
            state,
            output,
        )
    }
    pub fn hypothesis(
        &mut self,
        raw: ValidatedCanonicalStructuredValue<'_>,
        output: &mut Vec<u8>,
    ) -> R<()> {
        let parser = field(raw, "parser")?;
        state_copy(
            &mut self.records,
            "LanguageParserState",
            field(parser, "state")?,
            &mut self.state,
        )?;
        record(
            &mut self.records,
            "LanguageParserHypothesis",
            &[
                ("active", field(parser, "active")?),
                ("identity", field(parser, "identity")?),
                ("score", field(parser, "score")?),
                ("state", view(&self.state)?),
            ],
            &mut self.parser,
        )?;
        record(
            &mut self.records,
            "LanguageParserScorerHypothesis",
            &[
                ("choices", field(raw, "choices")?),
                ("parser", view(&self.parser)?),
                ("selected", field(raw, "selected")?),
            ],
            output,
        )
    }
}
fn record(
    records: &mut [Record],
    name: &str,
    fields: &[(&str, ValidatedCanonicalStructuredValue<'_>)],
    output: &mut Vec<u8>,
) -> R<()> {
    let entry = records
        .iter_mut()
        .find(|v| v.name == name)
        .ok_or(CursorRefusal::Preparation)?;
    if fields.len() != entry.fields.len() || fields.is_empty() || fields.len() > 12 {
        return Err(CursorRefusal::Canonical);
    }
    let mut ordered = [fields[0].1; 12];
    for (index, name) in entry.fields.iter().enumerate() {
        let mut found = fields.iter().filter(|(n, _)| n == name);
        ordered[index] = found.next().ok_or(CursorRefusal::Canonical)?.1;
        if found.next().is_some() {
            return Err(CursorRefusal::Canonical);
        }
    }
    put(
        output,
        entry
            .composer
            .record(&ordered[..fields.len()])
            .map_err(|_| CursorRefusal::Canonical)?,
    )
}
fn state_copy(
    records: &mut [Record],
    target: &str,
    state: ValidatedCanonicalStructuredValue<'_>,
    output: &mut Vec<u8>,
) -> R<()> {
    let names = [
        "basis",
        "committed",
        "depth",
        "heads",
        "relation0",
        "relation1",
        "relation2",
        "relation3",
        "stack",
        "token_count",
        "unread",
    ];
    let placeholder = field(state, "basis")?;
    let mut fields = [("", placeholder); 11];
    for (index, name) in names.iter().enumerate() {
        fields[index] = (name, field(state, name)?);
    }
    record(records, target, &fields, output)
}
