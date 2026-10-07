//! Source-produced compact codes retain their complete immutable lexical basis.
use super::{evaluate, PreparedWindow8State, Window8Refusal};
use crate::*;

#[derive(Clone)]
pub struct PreparedWindow8Lexical {
    lexical: LanguageParserWindow8Lexical,
    projection: LanguageParserWindow8RawProjection,
}
impl PreparedWindow8Lexical {
    pub fn lexical(&self) -> &LanguageParserWindow8Lexical {
        &self.lexical
    }
    pub fn projection(&self) -> &LanguageParserWindow8RawProjection {
        &self.projection
    }
}

pub fn prepare_window8_lexical(
    tape: &crate::lexical::PreparedLexicalTape,
) -> Result<PreparedWindow8Lexical, Window8Refusal> {
    let available: LanguageParserWindow8Available = evaluate(
        tape.tape().clone(),
        include_str!(concat!(env!("OUT_DIR"), "/window8_available.hex")),
    )?;
    let lexical = LanguageParserWindow8Lexical::new(tape.tape().clone(), *available.count())
        .map_err(Window8Refusal::Native)?;
    let mut tokens = alloc::vec::Vec::with_capacity(8);
    for ordinal in 0..8 {
        let codes: LanguageParserWindow8RawCodes = if ordinal < *lexical.token_count() {
            let query = LanguageParserWindow8CodeQuery::new(
                ordinal,
                lexical.tape().tokens()[ordinal as usize].clone(),
            )
            .map_err(Window8Refusal::Native)?;
            evaluate(
                query,
                include_str!(concat!(env!("OUT_DIR"), "/window8_token_codes.hex")),
            )?
        } else {
            let query =
                LanguageParserWindow8Ordinal::new(ordinal).map_err(Window8Refusal::Native)?;
            evaluate(
                query,
                include_str!(concat!(env!("OUT_DIR"), "/window8_empty_codes.hex")),
            )?
        };
        tokens.push(codes);
    }
    let projection = LanguageParserWindow8RawProjection::new(
        lexical.tape().profile().identity().clone(),
        *lexical.tape().source().sequence(),
        lexical.tape().source().material().revision().clone(),
        lexical.tape().source().material().identity().clone(),
        *lexical.token_count(),
        tokens.try_into().map_err(|_| Window8Refusal::Program)?,
    )
    .map_err(Window8Refusal::Native)?;
    Ok(PreparedWindow8Lexical {
        lexical,
        projection,
    })
}

pub struct PreparedWindow8Features {
    state: LanguageParserWindow8StateProof,
    lexical: LanguageParserWindow8Lexical,
    query: LanguageParserWindow8RawFeatureQuery,
    features: LanguageParserWindow8Features,
}
impl PreparedWindow8Features {
    pub fn state(&self) -> &LanguageParserWindow8StateProof {
        &self.state
    }
    pub fn lexical(&self) -> &LanguageParserWindow8Lexical {
        &self.lexical
    }
    pub fn query(&self) -> &LanguageParserWindow8RawFeatureQuery {
        &self.query
    }
    pub fn features(&self) -> &LanguageParserWindow8Features {
        &self.features
    }
}

/// This prepares exact arithmetic evidence, never a preferred lexical choice,
/// model prediction, stable arc, completed utterance or playback commitment.
pub fn prepare_window8_features(
    state: &PreparedWindow8State,
    lexical: &PreparedWindow8Lexical,
    expected_basis: &LanguageParserBasis,
    choices: [u64; 8],
) -> Result<PreparedWindow8Features, Window8Refusal> {
    let query = LanguageParserWindow8RawFeatureQuery::new(
        choices,
        expected_basis.clone(),
        lexical.projection().clone(),
        state.state().clone(),
    )
    .map_err(Window8Refusal::Native)?;
    let context: LanguageParserWindow8RawFeatureContext = evaluate(
        query.clone(),
        include_str!(concat!(env!("OUT_DIR"), "/window8_feature_context.hex")),
    )?;
    let raw: LanguageParserWindow8RawModelFeatures = evaluate(
        context,
        include_str!(concat!(env!("OUT_DIR"), "/window8_feature_values.hex")),
    )?;
    let features = LanguageParserWindow8Features::new(raw).map_err(Window8Refusal::Native)?;
    Ok(PreparedWindow8Features {
        state: state.proof().clone(),
        lexical: lexical.lexical().clone(),
        query,
        features,
    })
}
