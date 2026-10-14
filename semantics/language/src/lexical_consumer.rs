//! Supported lexical alternatives retain the original occurrence and policy.
use crate::{lexical::PreparedLexicalTape, pronunciation_selection::*, *};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum LexicalConsumerRefusal {
    Native(NativeBindingRefusal),
    Selection(PronunciationSelectionRefusal),
    Variety,
    Feature,
    Token,
}
pub struct PreparedLexicalConsumer<'a> {
    lexical: &'a PreparedLexicalTape,
    inventory: &'a LanguageLexicalConsumerInventory,
    variety: &'a LanguageVariety,
    selection: PreparedPronunciationSelection,
    basis: LanguageLexicalConsumerBasis,
    variety_match: LanguageLexicalConsumerVarietyMatch,
    features: Vec<LanguageLexicalConsumerFeatureMatch>,
}
impl<'a> PreparedLexicalConsumer<'a> {
    pub fn lexical(&self) -> &'a PreparedLexicalTape {
        self.lexical
    }
    pub fn inventory(&self) -> &'a LanguageLexicalConsumerInventory {
        self.inventory
    }
    pub fn variety(&self) -> &'a LanguageVariety {
        self.variety
    }
    pub fn selection(&self) -> &PreparedPronunciationSelection {
        &self.selection
    }
    pub fn basis(&self) -> &LanguageLexicalConsumerBasis {
        &self.basis
    }
    pub fn variety_match(&self) -> &LanguageLexicalConsumerVarietyMatch {
        &self.variety_match
    }
    pub fn features(&self) -> &[LanguageLexicalConsumerFeatureMatch] {
        &self.features
    }
    pub fn prepare(
        lexical: &'a PreparedLexicalTape,
        ordinal: usize,
        analysis: &LanguageAnalysisRevisionId,
        arc: &LanguageDependencyArc,
        profile: &LanguagePronunciationSelectionProfile,
        inventory: &'a LanguageLexicalConsumerInventory,
        variety: &'a LanguageVariety,
    ) -> Result<Self, LexicalConsumerRefusal> {
        use LexicalConsumerRefusal::*;
        let basis =
            LanguageLexicalConsumerBasis::new(inventory.clone(), lexical.tape().source().clone())
                .map_err(Native)?;
        let index = inventory
            .varieties()
            .as_slice()
            .iter()
            .position(|v| v == variety)
            .ok_or(Variety)?;
        let variety_match = LanguageLexicalConsumerVarietyMatch::new(
            index as u64,
            inventory.clone(),
            variety.clone(),
        )
        .map_err(Native)?;
        let token = lexical
            .tape()
            .tokens()
            .as_slice()
            .get(ordinal)
            .ok_or(Token)?;
        let mut features = Vec::new();
        // Check every retained alternative. Unsupported analyses are not silently pruned.
        for candidate in token.candidates().as_slice() {
            for feature in candidate.morphology().as_slice() {
                let index = inventory
                    .features()
                    .as_slice()
                    .iter()
                    .position(|f| f == feature)
                    .ok_or(Feature)?;
                features.push(
                    LanguageLexicalConsumerFeatureMatch::new(
                        feature.clone(),
                        index as u64,
                        inventory.clone(),
                    )
                    .map_err(Native)?,
                );
            }
        }
        let selection = prepare_pronunciation_selection(lexical, ordinal, analysis, arc, profile)
            .map_err(Selection)?;
        Ok(Self {
            lexical,
            inventory,
            variety,
            selection,
            basis,
            variety_match,
            features,
        })
    }
}

pub fn lexical_consumer_types() -> Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageLexicalConsumerInventory",
            LanguageLexicalConsumerInventory::semantic_type().expect("checked Type")
        ),
        (
            "LanguageLexicalConsumerVarietyMatch",
            LanguageLexicalConsumerVarietyMatch::semantic_type().expect("checked Type")
        ),
        (
            "LanguageLexicalConsumerFeatureMatch",
            LanguageLexicalConsumerFeatureMatch::semantic_type().expect("checked Type")
        ),
        (
            "LanguageLexicalConsumerBasis",
            LanguageLexicalConsumerBasis::semantic_type().expect("checked Type")
        ),
    ]
}
