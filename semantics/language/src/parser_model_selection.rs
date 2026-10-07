//! Preparation-only admission for the pinned v2 learned scoring profile.
//! This owns no parse, lexical choice, legality, recurrence or fallback policy.
use crate::{
    LanguageId, LanguageLexicalCandidate, LanguageLexicalEntry, LanguageLexicalPos,
    LanguageLexicalProfile, LinguisticDerivationProvenance,
};
use alloc::{collections::BTreeMap, format, string::String, sync::Arc, vec};
use conduit_ai::{integer_categorical_step::PreparedCategoricalStep, *};
use conduit_core::semantic_digest;
use conduit_data::{TensorAxisRole, TensorElement};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

const PROFILE: &[u8] = include_bytes!("../training/ewt_joint_v2/lexical_profile.json");
const MANIFEST: &[u8] = include_bytes!("../training/ewt_joint_v2/manifest.json");
pub const V2_LOOKUPS: u64 = 25;
pub const V2_SCORES: u64 = 76;
pub const V2_MAXIMUM_SCORE: u64 = 229376;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserModelSelectionRefusal {
    ProfileData,
    SourceContract,
    LexicalProfile,
    Signature,
    ModelContent,
    ModelStateSchema,
    ScoreBound,
    ProfileIdentity,
    ModelArtifact,
    ModelDimensions,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserModelCompatibility {
    pub lexical_profile: [u8; 32],
    pub feature_contract: [u8; 32],
    pub availability_contract: [u8; 32],
    pub action_contract: [u8; 32],
    pub joint_choice_contract: [u8; 32],
    pub model_content: [u8; 32],
    pub signature: [u8; 32],
    pub maximum_score_magnitude: u64,
    pub lookups: u64,
    pub scores: u64,
}
/// Exact metadata declared by the checked Source profile's preparation driver.
/// These identities do not by themselves prove execution or linguistic accuracy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserSourceModelContract {
    pub feature_contract: [u8; 32],
    pub availability_contract: [u8; 32],
    pub action_contract: [u8; 32],
    pub joint_choice_contract: [u8; 32],
    pub numeric_indices_contract: [u8; 32],
    pub numeric_scores_contract: [u8; 32],
}
/// Separately versioned reviewed metadata; model swaps require a new declaration.
/// The artifact includes exact content/reference, precision and state schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserModelProfileDefinition {
    identity: String,
    version: u32,
    lexical: LanguageLexicalProfile,
    provenance: LinguisticDerivationProvenance,
    source: ParserSourceModelContract,
    artifact: ModelArtifact,
    signature: ModelSignature,
    dimensions: (usize, usize, usize),
    maximum_score_magnitude: u64,
}
impl ParserModelProfileDefinition {
    /// Dimensions are (feature categories, score outputs, lookups per frame).
    /// Identity/version name the declaration; admission compares its full material.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: String,
        version: u32,
        lexical: LanguageLexicalProfile,
        provenance: LinguisticDerivationProvenance,
        source: ParserSourceModelContract,
        artifact: ModelArtifact,
        signature: ModelSignature,
        dimensions: (usize, usize, usize),
        maximum_score_magnitude: u64,
    ) -> Result<Self, ParserModelSelectionRefusal> {
        use ParserModelSelectionRefusal::*;
        if identity.is_empty() || identity.len() > 256 || version == 0 {
            return Err(ProfileIdentity);
        }
        if dimensions.0 == 0 || dimensions.1 == 0 || dimensions.2 == 0 {
            return Err(ModelDimensions);
        }
        if artifact.state_schema_version == 0 {
            return Err(ModelStateSchema);
        }
        if artifact.signature_identity != signature.semantic_digest().map_err(|_| Signature)? {
            return Err(Signature);
        }
        if maximum_score_magnitude > i64::MAX as u64 {
            return Err(ScoreBound);
        }
        provenance
            .clone()
            .into_structured()
            .map_err(|_| ProfileData)?;
        lexical
            .clone()
            .into_structured()
            .map_err(|_| LexicalProfile)?;
        Ok(Self {
            identity,
            version,
            lexical,
            provenance,
            source,
            artifact,
            signature,
            dimensions,
            maximum_score_magnitude,
        })
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn version(&self) -> u32 {
        self.version
    }
    pub fn source_contract(&self) -> &ParserSourceModelContract {
        &self.source
    }
    pub fn artifact(&self) -> &ModelArtifact {
        &self.artifact
    }
    pub fn provenance(&self) -> &LinguisticDerivationProvenance {
        &self.provenance
    }
    pub fn lexical_profile(&self) -> &LanguageLexicalProfile {
        &self.lexical
    }
    pub fn signature(&self) -> &ModelSignature {
        &self.signature
    }
    pub fn dimensions(&self) -> (usize, usize, usize) {
        self.dimensions
    }
    pub fn maximum_score_magnitude(&self) -> u64 {
        self.maximum_score_magnitude
    }
}
pub struct PreparedParserModelSelection {
    categorical: Arc<PreparedCategoricalStep>,
    lexical: LanguageLexicalProfile,
    compatibility: ParserModelCompatibility,
    declaration: Option<Arc<ParserModelProfileDefinition>>,
}
fn hex(digest: [u8; 32]) -> String {
    use core::fmt::Write;
    let mut out = String::with_capacity(64);
    for byte in digest {
        write!(&mut out, "{byte:02x}").expect("String formatting");
    }
    out
}
fn pos(code: usize) -> Option<LanguageLexicalPos> {
    // Metadata ingestion of the separately versioned Source feature encoding.
    // Runtime enum-to-index projection remains in parser_scorer_v2.conduit.
    Some(match code {
        0 => LanguageLexicalPos::Adjective,
        1 => LanguageLexicalPos::Adposition,
        2 => LanguageLexicalPos::Adverb,
        3 => LanguageLexicalPos::Auxiliary,
        4 => LanguageLexicalPos::CoordinatingConjunction,
        5 => LanguageLexicalPos::Determiner,
        6 => LanguageLexicalPos::Interjection,
        7 => LanguageLexicalPos::Noun,
        8 => LanguageLexicalPos::Numeral,
        9 => LanguageLexicalPos::Particle,
        10 => LanguageLexicalPos::Pronoun,
        11 => LanguageLexicalPos::ProperNoun,
        12 => LanguageLexicalPos::Punctuation,
        13 => LanguageLexicalPos::SubordinatingConjunction,
        14 => LanguageLexicalPos::Symbol,
        15 => LanguageLexicalPos::Verb,
        16 => LanguageLexicalPos::Other,
        _ => return None,
    })
}
pub fn pinned_v2_lexical_profile() -> Result<LanguageLexicalProfile, ParserModelSelectionRefusal> {
    use ParserModelSelectionRefusal::ProfileData;
    let data: BTreeMap<String, alloc::vec::Vec<usize>> =
        serde_json::from_slice(PROFILE).map_err(|_| ProfileData)?;
    if data.len() != 64 {
        return Err(ProfileData);
    }
    let mut entries = alloc::vec::Vec::with_capacity(data.len());
    for (surface, codes) in data {
        let mut candidates = alloc::vec::Vec::with_capacity(codes.len());
        for code in codes {
            candidates.push(
                LanguageLexicalCandidate::new(
                    surface.clone(),
                    BoundedSequence::new(),
                    pos(code).ok_or(ProfileData)?,
                )
                .map_err(|_| ProfileData)?,
            );
        }
        entries.push(
            LanguageLexicalEntry::new(
                BoundedSequence::try_from_iter(candidates).map_err(|_| ProfileData)?,
                surface,
            )
            .map_err(|_| ProfileData)?,
        );
    }
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter(entries).map_err(|_| ProfileData)?,
        hex(semantic_digest(
            "language/parser-lexical-profile@1",
            PROFILE,
        )),
        LanguageId::new("language/en".into()).map_err(|_| ProfileData)?,
        LinguisticDerivationProvenance::deterministic_rule(
            "language/parser-joint-v2-reviewed-and-ewt".into(),
            "ud/ewt-2.18+reviewed-vocative@1".into(),
        )
        .map_err(|_| ProfileData)?,
    )
    .map_err(|_| ProfileData)
}
fn contracts() -> Result<ParserModelCompatibility, ParserModelSelectionRefusal> {
    use ParserModelSelectionRefusal::SourceContract;
    let manifest: serde_json::Value =
        serde_json::from_slice(MANIFEST).map_err(|_| SourceContract)?;
    let mut result = ParserModelCompatibility {
        lexical_profile: semantic_digest("language/parser-lexical-profile@1", PROFILE),
        feature_contract: semantic_digest(
            "language/parser-v2-scorer-encoding@1",
            include_bytes!("../parser_scorer_v2.conduit"),
        ),
        availability_contract: semantic_digest(
            "language/parser-available-contract@1",
            include_bytes!("../parser_available.conduit"),
        ),
        action_contract: semantic_digest(
            "language/parser-scorer-encoding@1",
            include_bytes!("../parser_scorer.conduit"),
        ),
        joint_choice_contract: semantic_digest(
            "language/parser-joint-encoding@1",
            include_bytes!("../parser_joint.conduit"),
        ),
        model_content: [0; 32],
        signature: [0; 32],
        maximum_score_magnitude: 0,
        lookups: V2_LOOKUPS,
        scores: V2_SCORES,
    };
    for (key, digest) in [
        ("lexical_profile_identity", result.lexical_profile),
        ("feature_class_contract_identity", result.feature_contract),
        (
            "availability_contract_identity",
            result.availability_contract,
        ),
        ("action_class_contract_identity", result.action_contract),
        (
            "joint_choice_contract_identity",
            result.joint_choice_contract,
        ),
    ] {
        if manifest[key].as_str() != Some(hex(digest).as_str()) {
            return Err(SourceContract);
        }
    }
    if manifest["lookups"].as_u64() != Some(V2_LOOKUPS)
        || manifest["features"].as_u64() != Some(413)
    {
        return Err(SourceContract);
    }
    let content = manifest["model_content_identity"]
        .as_str()
        .ok_or(SourceContract)?;
    if content.len() != 64 {
        return Err(SourceContract);
    }
    for (index, chunk) in content.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let text = core::str::from_utf8(chunk).map_err(|_| SourceContract)?;
        result.model_content[index] = u8::from_str_radix(text, 16).map_err(|_| SourceContract)?;
    }
    Ok(result)
}
pub fn pinned_v2_model_signature() -> Result<ModelSignature, ParserModelSelectionRefusal> {
    use ParserModelSelectionRefusal::Signature;
    let c = contracts()?;
    let encoding = hex(c.feature_contract);
    let profile = format!(
        "v2/{}/{}",
        hex(c.availability_contract),
        hex(c.lexical_profile)
    );
    let contract = semantic_digest(
        "language/parser-model-contract@1",
        format!("{encoding}/{profile}").as_bytes(),
    );
    let port = |name: &str, element, count| {
        ModelPortConstraint::new(
            ModelPortIdentity::new(name.into()).map_err(|_| Signature)?,
            ModelPortPresence::Required,
            ModelSemanticKind::new(format!("language/parser-{name}/{encoding}@1"))
                .map_err(|_| Signature)?,
            ModelValueConstraint::tensor(
                ModelTensorConstraint::from_parts(
                    vec![element],
                    vec![ModelAxisConstraint::new(
                        ModelDimensionConstraint::fixed(count).map_err(|_| Signature)?,
                        TensorAxisRole::Feature,
                    )
                    .map_err(|_| Signature)?],
                    count * 8,
                )
                .map_err(|_| Signature)?,
            )
            .map_err(|_| Signature)?,
        )
        .map_err(|_| Signature)
    };
    ModelSignature::from_parts(
        format!("language/parser-joint/{}@1", hex(contract)),
        1,
        vec![ModelOperation::Infer],
        vec![port("features", TensorElement::U64, V2_LOOKUPS)?],
        vec![port("scores", TensorElement::I64, V2_SCORES)?],
    )
    .map_err(|_| Signature)
}
impl PreparedParserModelSelection {
    pub fn prepare(
        categorical: Arc<PreparedCategoricalStep>,
        lexical: &LanguageLexicalProfile,
    ) -> Result<Self, ParserModelSelectionRefusal> {
        let expected = pinned_v2_lexical_profile()?;
        if lexical != &expected {
            return Err(ParserModelSelectionRefusal::LexicalProfile);
        }
        let signature = pinned_v2_model_signature()?;
        if categorical.resource().signature() != &signature {
            return Err(ParserModelSelectionRefusal::Signature);
        }
        let mut compatibility = contracts()?;
        if categorical.resource().artifact().content_identity() != compatibility.model_content {
            return Err(ParserModelSelectionRefusal::ModelContent);
        }
        if categorical.resource().artifact().state_schema_version != 1 {
            return Err(ParserModelSelectionRefusal::ModelStateSchema);
        }
        let bound = categorical.maximum_score_magnitude();
        if bound > V2_MAXIMUM_SCORE {
            return Err(ParserModelSelectionRefusal::ScoreBound);
        }
        compatibility.maximum_score_magnitude = bound;
        compatibility.signature = signature
            .semantic_digest()
            .map_err(|_| ParserModelSelectionRefusal::Signature)?;
        Ok(Self {
            categorical,
            lexical: expected,
            compatibility,
            declaration: None,
        })
    }
    /// Admit an explicitly declared alternative profile without weakening pinned v2.
    /// The caller must retain and execute the checked Source whose contract is supplied.
    /// This preparation owns metadata/custody, never grammar or parse choice policy.
    pub fn prepare_declared(
        categorical: Arc<PreparedCategoricalStep>,
        declaration: Arc<ParserModelProfileDefinition>,
        lexical: &LanguageLexicalProfile,
        actual_source: &ParserSourceModelContract,
    ) -> Result<Self, ParserModelSelectionRefusal> {
        use ParserModelSelectionRefusal::*;
        if lexical != &declaration.lexical {
            return Err(LexicalProfile);
        }
        if actual_source != &declaration.source {
            return Err(SourceContract);
        }
        if categorical.resource().signature() != &declaration.signature {
            return Err(Signature);
        }
        if categorical.resource().artifact() != &declaration.artifact {
            return Err(ModelArtifact);
        }
        if categorical.dimensions() != declaration.dimensions {
            return Err(ModelDimensions);
        }
        if categorical
            .indices_type()
            .semantic_digest()
            .map_err(|_| SourceContract)?
            != actual_source.numeric_indices_contract
            || categorical
                .scores_type()
                .semantic_digest()
                .map_err(|_| SourceContract)?
                != actual_source.numeric_scores_contract
        {
            return Err(SourceContract);
        }
        let maximum = categorical.maximum_score_magnitude();
        if maximum > declaration.maximum_score_magnitude {
            return Err(ScoreBound);
        }
        let lexical_bytes = lexical
            .clone()
            .into_structured()
            .map_err(|_| LexicalProfile)?
            .canonical_bytes()
            .map_err(|_| LexicalProfile)?;
        let compatibility = ParserModelCompatibility {
            lexical_profile: semantic_digest("language/parser-declared-lexical@1", &lexical_bytes),
            feature_contract: actual_source.feature_contract,
            availability_contract: actual_source.availability_contract,
            action_contract: actual_source.action_contract,
            joint_choice_contract: actual_source.joint_choice_contract,
            model_content: categorical.resource().artifact().content_identity(),
            signature: declaration
                .signature
                .semantic_digest()
                .map_err(|_| Signature)?,
            maximum_score_magnitude: maximum,
            lookups: declaration.dimensions.2 as u64,
            scores: declaration.dimensions.1 as u64,
        };
        Ok(Self {
            categorical,
            lexical: lexical.clone(),
            compatibility,
            declaration: Some(declaration),
        })
    }
    pub fn declaration(&self) -> Option<&ParserModelProfileDefinition> {
        self.declaration.as_deref()
    }
    pub fn expected_lexical_profile(&self) -> &LanguageLexicalProfile {
        &self.lexical
    }
    pub fn prepared_categorical(&self) -> &Arc<PreparedCategoricalStep> {
        &self.categorical
    }
    pub fn compatibility(&self) -> &ParserModelCompatibility {
        &self.compatibility
    }
}
