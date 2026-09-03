//! Reproducible, bounded report for the paired-audio/articulation research experiment.

use crate::{
    std_compute_evidence, train_shared_latent, ComputeEvidence, ObjectiveMetrics, Pb2007Slice,
    ResearchDataError, ResearchModelError, TrainingUtterance, PB2007_ARCHIVE_SHA256,
    TRAINING_WORK_BOUND,
};
use conduit_ai::{
    ModelAxisConstraint, ModelDimensionConstraint, ModelOperation, ModelPortConstraint,
    ModelPortPresence, ModelRelationSignature, ModelSignature, ModelTensorConstraint,
    ModelValueConstraint, RelationQueryMode, RelationResultProfile, RelationVariable,
    SupportedRelationQuery,
};
use conduit_data::{TensorAxisRole, TensorElement};
use serde::Serialize;

pub const RESEARCH_SEED: u64 = 2_132;

#[derive(Clone, Debug, Serialize)]
pub struct ResearchReport {
    pub schema: String,
    pub corpus: CorpusEvidence,
    pub training: crate::TrainingEvidence,
    pub alternate_checkpoint_identity: String,
    pub callable_signature_identity: String,
    pub relation_signature_identity: String,
    pub std_host_compute: ComputeEvidence,
    pub held_out: Vec<SplitEvidence>,
    pub bidirectional_query: BidirectionalEvidence,
    pub post_freeze_probe: ProbeEvidence,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CorpusEvidence {
    pub doi: String,
    pub archive_sha256: String,
    pub derivation_identity: String,
    pub utterances: usize,
    pub speakers: usize,
    pub splits: Vec<SplitCount>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SplitCount {
    pub identity: String,
    pub utterances: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SplitEvidence {
    pub split: String,
    pub frames: usize,
    pub objectives_millionths: ObjectiveMetrics,
}

#[derive(Clone, Debug, Serialize)]
pub struct BidirectionalEvidence {
    pub utterance: String,
    pub audio_to_latent: Vec<f64>,
    pub articulation_to_latent: Vec<f64>,
    pub generated_audio: Vec<f64>,
    pub inferred_articulation: crate::ArticulationPosterior,
    pub next_latent: Vec<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProbeEvidence {
    pub checkpoint_frozen_before_labels: bool,
    pub task: String,
    pub train_frames: usize,
    pub test_frames: usize,
    pub correct_test_frames: usize,
    pub accuracy_millionths: u64,
    pub unsupported_claims: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResearchError {
    Data(ResearchDataError),
    Model(ResearchModelError),
    InvalidSignature,
    InvalidForm,
    MissingHeldOutData,
}

impl From<ResearchDataError> for ResearchError {
    fn from(value: ResearchDataError) -> Self {
        Self::Data(value)
    }
}

impl From<ResearchModelError> for ResearchError {
    fn from(value: ResearchModelError) -> Self {
        Self::Model(value)
    }
}

pub fn run_research() -> Result<ResearchReport, ResearchError> {
    crate::check_research_forms().map_err(|_| ResearchError::InvalidForm)?;
    let corpus = Pb2007Slice::load()?;
    let training_utterances = corpus.training_utterances();
    let (checkpoint, training) = train_shared_latent(&training_utterances, RESEARCH_SEED)?;
    let (alternate, _) = train_shared_latent(&training_utterances, RESEARCH_SEED + 1)?;
    let checkpoint_digest = checkpoint.identity()?;
    let frozen = corpus.freeze(checkpoint_digest)?;
    let _labels = corpus.probe_labels(&frozen)?;

    let signature = shared_latent_signature();
    let signature_digest = signature
        .semantic_digest()
        .map_err(|_| ResearchError::InvalidSignature)?;
    let relation = shared_relation_signature(&signature);
    relation
        .validate()
        .map_err(|_| ResearchError::InvalidSignature)?;
    let relation_digest = relation
        .semantic_digest()
        .map_err(|_| ResearchError::InvalidSignature)?;
    let compute = std_compute_evidence()?;
    let held_out = ["validation", "test"]
        .into_iter()
        .map(|split| evaluate_split(&checkpoint, &training_utterances, split))
        .collect::<Result<Vec<_>, _>>()?;
    let sample = corpus
        .utterances
        .iter()
        .find(|value| value.split == "test")
        .ok_or(ResearchError::MissingHeldOutData)?;
    let audio_latent = checkpoint.encode_acoustic(&sample.acoustic[8])?;
    let articulation_latent = checkpoint.encode_articulation(&sample.articulation[8])?;
    let bidirectional_query = BidirectionalEvidence {
        utterance: sample.identity.clone(),
        generated_audio: checkpoint.decode_acoustic(&articulation_latent.mean)?,
        inferred_articulation: checkpoint.infer_articulation(&sample.acoustic[8], RESEARCH_SEED)?,
        next_latent: checkpoint.next_latent(&audio_latent.mean)?,
        audio_to_latent: audio_latent.mean,
        articulation_to_latent: articulation_latent.mean,
    };

    Ok(ResearchReport {
        schema: "conduit.tongues/paired-research-report@1".into(),
        corpus: corpus_evidence(&corpus),
        alternate_checkpoint_identity: hex(alternate.identity()?),
        callable_signature_identity: hex(signature_digest),
        relation_signature_identity: hex(relation_digest),
        std_host_compute: compute,
        held_out,
        bidirectional_query,
        post_freeze_probe: evaluate_probe(&corpus, &checkpoint, &frozen)?,
        limitations: vec![
            "one PB2007 speaker; cross-speaker generalization and speaker conditioning are not identifiable".into(),
            "four deterministic acoustic summary features are not a self-supervised waveform encoder".into(),
            "the deposit does not establish head-correction provenance for these EMA coordinates".into(),
            "twelve utterances make this a bounded architecture proof, not a state-of-the-art speech result".into(),
            format!("training is refused above {TRAINING_WORK_BOUND} admitted work units"),
        ],
        training,
    })
}

pub fn run_research_json() -> Result<String, ResearchError> {
    serde_json::to_string_pretty(&run_research()?).map_err(|_| ResearchError::InvalidSignature)
}

pub fn shared_latent_signature() -> ModelSignature {
    ModelSignature {
        identity: "conduit.tongues/shared-paired-latent@1".into(),
        compatibility_version: 1,
        operations: vec![
            ModelOperation::Encode,
            ModelOperation::Decode,
            ModelOperation::Sample,
            ModelOperation::Evaluate,
            ModelOperation::Train,
        ],
        inputs: vec![port("acoustic", 4, false), port("articulation", 6, false)],
        outputs: vec![
            port("latent", 2, true),
            port("generated-acoustic", 4, false),
            probabilistic_port("inferred-articulation", 6),
        ],
    }
}

pub fn shared_relation_signature(signature: &ModelSignature) -> ModelRelationSignature {
    let variables = [
        ("acoustic", 4, false),
        ("articulation", 6, true),
        ("latent", 2, false),
    ]
    .into_iter()
    .map(|(identity, dimensions, probabilistic)| RelationVariable {
        identity: identity.into(),
        semantic_role: format!("tongues/{identity}@1"),
        value: if probabilistic {
            ModelValueConstraint::ProbabilisticTensor(tensor(dimensions))
        } else {
            ModelValueConstraint::Tensor(tensor(dimensions))
        },
    })
    .collect();
    let deterministic = RelationResultProfile::Deterministic;
    let probabilistic = RelationResultProfile::Probabilistic { maximum_samples: 2 };
    ModelRelationSignature {
        identity: "conduit.tongues/shared-paired-relation@1".into(),
        compatibility_version: 1,
        callable_signature_identity: signature.semantic_digest().expect("known-valid signature"),
        variables,
        supported_queries: vec![
            query(
                "acoustic",
                "latent",
                RelationQueryMode::EncodeLatent,
                deterministic,
            ),
            query(
                "articulation",
                "latent",
                RelationQueryMode::EncodeLatent,
                deterministic,
            ),
            query(
                "acoustic",
                "articulation",
                RelationQueryMode::InferPosterior,
                probabilistic,
            ),
            query(
                "articulation",
                "acoustic",
                RelationQueryMode::DecodeGenerate,
                deterministic,
            ),
            query(
                "latent",
                "acoustic",
                RelationQueryMode::DecodeGenerate,
                deterministic,
            ),
            query(
                "latent",
                "articulation",
                RelationQueryMode::DecodeGenerate,
                probabilistic,
            ),
        ],
    }
}

fn query(
    evidence: &str,
    target: &str,
    mode: RelationQueryMode,
    result_profile: RelationResultProfile,
) -> SupportedRelationQuery {
    SupportedRelationQuery {
        evidence_variables: vec![evidence.into()],
        target_variables: vec![target.into()],
        mode,
        result_profile,
        maximum_work_units: 4_096,
        maximum_output_bytes: 4_096,
    }
}

fn port(identity: &str, dimensions: u64, optional: bool) -> ModelPortConstraint {
    ModelPortConstraint {
        identity: identity.into(),
        semantic_kind: format!("tongues/{identity}@1"),
        presence: if optional {
            ModelPortPresence::Optional
        } else {
            ModelPortPresence::Required
        },
        value: ModelValueConstraint::Tensor(tensor(dimensions)),
    }
}

fn probabilistic_port(identity: &str, dimensions: u64) -> ModelPortConstraint {
    ModelPortConstraint {
        identity: identity.into(),
        semantic_kind: format!("tongues/{identity}@1"),
        presence: ModelPortPresence::Optional,
        value: ModelValueConstraint::ProbabilisticTensor(tensor(dimensions)),
    }
}

fn tensor(dimensions: u64) -> ModelTensorConstraint {
    ModelTensorConstraint {
        elements: vec![TensorElement::F64],
        axes: vec![ModelAxisConstraint {
            role: TensorAxisRole::Feature,
            dimension: ModelDimensionConstraint::Fixed(dimensions),
        }],
        maximum_bytes: dimensions * 8,
    }
}

fn corpus_evidence(corpus: &Pb2007Slice) -> CorpusEvidence {
    CorpusEvidence {
        doi: corpus.source.doi.clone(),
        archive_sha256: PB2007_ARCHIVE_SHA256.into(),
        derivation_identity: corpus.derivation.identity.clone(),
        utterances: corpus.utterances.len(),
        speakers: 1,
        splits: ["train", "validation", "test"]
            .into_iter()
            .map(|split| SplitCount {
                identity: split.into(),
                utterances: corpus
                    .utterances
                    .iter()
                    .filter(|value| value.split == split)
                    .count(),
            })
            .collect(),
    }
}

fn evaluate_split(
    checkpoint: &crate::ResearchCheckpoint,
    utterances: &[TrainingUtterance],
    split: &str,
) -> Result<SplitEvidence, ResearchError> {
    let selected = utterances
        .iter()
        .filter(|value| value.split == split)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(ResearchError::MissingHeldOutData);
    }
    let mut totals: [f64; 6] = [0.0; 6];
    let mut frames = 0;
    for utterance in selected {
        let mut previous: Option<Vec<f64>> = None;
        for (audio, articulation) in utterance.acoustic.iter().zip(&utterance.articulation) {
            let zaudio = checkpoint.encode_acoustic(audio)?.mean;
            let zart = checkpoint.encode_articulation(articulation)?.mean;
            totals[0] += mse_i64(&checkpoint.decode_acoustic(&zaudio)?, audio);
            totals[1] += mse_i64(&checkpoint.decode_articulation(&zart)?, articulation);
            totals[2] += mse_i64(&checkpoint.decode_articulation(&zaudio)?, articulation);
            totals[3] += mse_i64(&checkpoint.decode_acoustic(&zart)?, audio);
            totals[4] += mse(&zaudio, &zart);
            if let Some(value) = previous {
                totals[5] += mse(&checkpoint.next_latent(&value)?, &zaudio);
            }
            previous = Some(zaudio);
            frames += 1;
        }
    }
    let scale = |index: usize| {
        (totals[index] / frames as f64 * 1_000_000.0)
            .round()
            .max(0.0) as u64
    };
    Ok(SplitEvidence {
        split: split.into(),
        frames,
        objectives_millionths: ObjectiveMetrics {
            acoustic_reconstruction: scale(0),
            articulatory_reconstruction: scale(1),
            acoustic_to_articulatory: scale(2),
            articulatory_to_acoustic: scale(3),
            latent_agreement: scale(4),
            dynamics_prediction: scale(5),
        },
    })
}

fn evaluate_probe(
    corpus: &Pb2007Slice,
    checkpoint: &crate::ResearchCheckpoint,
    frozen: &crate::FrozenRepresentation,
) -> Result<ProbeEvidence, ResearchError> {
    let labels = corpus.probe_labels(frozen)?;
    let mut centroids = [[0.0; 2]; 2];
    let mut counts = [0_usize; 2];
    for utterance in corpus
        .utterances
        .iter()
        .filter(|value| value.split == "train")
    {
        for (bin, acoustic) in utterance.acoustic.iter().enumerate() {
            let class = usize::from(label_at(&labels, &utterance.identity, bin) != "__");
            let latent = checkpoint.encode_acoustic(acoustic)?.mean;
            for (target, value) in centroids[class].iter_mut().zip(latent) {
                *target += value;
            }
            counts[class] += 1;
        }
    }
    for class in 0..2 {
        for value in &mut centroids[class] {
            *value /= counts[class] as f64;
        }
    }
    let mut correct = 0;
    let mut total = 0;
    for utterance in corpus
        .utterances
        .iter()
        .filter(|value| value.split == "test")
    {
        for (bin, acoustic) in utterance.acoustic.iter().enumerate() {
            let expected = usize::from(label_at(&labels, &utterance.identity, bin) != "__");
            let latent = checkpoint.encode_acoustic(acoustic)?.mean;
            let predicted = usize::from(mse(&latent, &centroids[1]) < mse(&latent, &centroids[0]));
            correct += usize::from(predicted == expected);
            total += 1;
        }
    }
    Ok(ProbeEvidence {
        checkpoint_frozen_before_labels: true,
        task: "binary silence-versus-labelled-phone nearest-centroid probe@1".into(),
        train_frames: counts.iter().sum(),
        test_frames: total,
        correct_test_frames: correct,
        accuracy_millionths: (correct as f64 / total as f64 * 1_000_000.0).round() as u64,
        unsupported_claims: vec![
            "phone-class generalization: test phone labels are absent from the tiny training slice"
                .into(),
            "causal or interpretable articulatory factors".into(),
        ],
    })
}

fn label_at<'a>(
    labels: &'a [(&str, &'a [crate::ProbeSegment])],
    identity: &str,
    bin: usize,
) -> &'a str {
    labels
        .iter()
        .find(|(candidate, _)| *candidate == identity)
        .and_then(|(_, segments)| {
            segments
                .iter()
                .find(|segment| segment.start_bin <= bin && bin < segment.end_bin)
        })
        .map_or("__", |segment| segment.label.as_str())
}

fn mse_i64(actual: &[f64], expected: &[i64]) -> f64 {
    actual
        .iter()
        .zip(expected)
        .map(|(left, right)| (left - *right as f64).powi(2))
        .sum::<f64>()
        / actual.len() as f64
}

fn mse(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        / left.len() as f64
}

fn hex(identity: [u8; 32]) -> String {
    format!(
        "sha256:{}",
        identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}
