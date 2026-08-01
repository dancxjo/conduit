//! Optional bounded learned-model lifecycle contracts.
//!
//! Dataset snapshots, training jobs, checkpoints, evaluation reports, and
//! promotion receipts retain separate identities.  The deterministic provider
//! is a finite conformance witness; ordinary inference does not install it.

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};

use crate::{MODEL_ARTIFACT_BYTES, MODEL_ARTIFACT_IDENTITY, MODEL_ARTIFACT_TYPE};

pub const DATASET_SNAPSHOT_IDENTITY: &str =
    "sha256:71d58097138d514f3220280698874fecce2486878fc5a2f7a9bc50a8e1341186";
pub const DATASET_REVISION_IDENTITY: &str =
    "sha256:5c434f11669a98b9587880d824f8fda7c19d26dc862696b2602066c79e794972";
pub const FEATURE_SCHEMA_IDENTITY: &str =
    "sha256:d5ac227d73ef18638d38b51c67b816148cd18c837680cc2fb827e4ef773c5145";
pub const LABEL_SCHEMA_IDENTITY: &str =
    "sha256:ad38d644903656f90d1ffb3f2f465f82c7c1debd01f607648da8808f91891f73";
pub const DATASET_PROVENANCE_IDENTITY: &str =
    "sha256:e010280c5999c5f7b0dfbe947783c8bef2dac142103d9f5ed28e72db89765a20";
pub const TRAINING_JOB_IDENTITY: &str =
    "sha256:6483c5ec0278e838e741c47a00326843811240e68779861691eaefdf63acfb01";
pub const CHECKPOINT_IDENTITY: &str =
    "sha256:802134cf032dec3fc656c072748510009bf3f47192e91a42cd6d7d370fd9ce31";
pub const EVALUATION_SUITE_IDENTITY: &str =
    "sha256:32cc2f3149f9fb3bc1f49acfe831c3cf51077f4a43a8b0fd6fb00a05dd97d93e";
pub const METRIC_IDENTITY: &str =
    "sha256:f97c8933472e4bcffb4e6b8c64d598ca7e6a9e6ea4d991b4d8e335928ae92f2f";
pub const EVALUATION_REPORT_IDENTITY: &str =
    "sha256:72a56cd84cb7f8580976b99177996998e1434bbffcd2e85f1f5dca0d6b607d7b";
pub const PROMOTION_APPROVAL_IDENTITY: &str =
    "sha256:06428af77f473f5ea139b26e61c334096282034068249444382bcc27926d7df8";
pub const PROMOTION_RECEIPT_IDENTITY: &str =
    "sha256:37fa44058daed10ea46d9543393df9577fb4fe447a6a33a160aff9bdb864a002";

pub const DATASET_BYTES: &[u8] =
    b"CLD0|dataset:snapshot:tiny|revision=1|split=train|records=4|public";
pub const CHECKPOINT_BYTES: &[u8] =
    b"CLC0|checkpoint:job:tiny|step=4|base=fixed-linear|reproducible=exact";
pub const EVALUATION_BYTES: &[u8] =
    b"CLE0|evaluation:suite:tiny|metric=accuracy@1|cases=4|score=4/4";
pub const PROMOTION_RECEIPT_BYTES: &[u8] =
    b"CLP0|promotion:tiny|slot=learned/reference|commit=acknowledged";

pub const DATASET_SNAPSHOT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("learned/dataset-snapshot"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0xd1; 32]),
};
pub const CHECKPOINT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("learned/checkpoint"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0xc1; 32]),
};
pub const EVALUATION_REPORT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("learned/evaluation-report"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0xe1; 32]),
};
pub const PROMOTION_RECEIPT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("learned/promotion-receipt"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([0xf1; 32]),
};
const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};
const U64_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/u64"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0xba, 0xd3, 0xea, 0x53, 0xd3, 0xca, 0x01, 0xa0, 0xa4, 0xd6, 0x9f, 0x86, 0xc8, 0x25,
        0x65, 0x17, 0x07, 0x16, 0x45, 0xea, 0x7d, 0x68, 0xef, 0x63, 0x6b, 0x6d, 0x94, 0x87, 0x70,
        0xf0, 0xec,
    ]),
};

const fn field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }
}

const DATASET_FIELDS: [ConfigFieldContract<'static>; 11] = [
    field("fixture", TEXT_TYPE),
    field("snapshot_identity", TEXT_TYPE),
    field("revision_identity", TEXT_TYPE),
    field("feature_schema_identity", TEXT_TYPE),
    field("label_schema_identity", TEXT_TYPE),
    field("provenance_identity", TEXT_TYPE),
    field("sensitivity", TEXT_TYPE),
    field("access_scope", TEXT_TYPE),
    field("split", TEXT_TYPE),
    field("maximum_records", U64_TYPE),
    field("maximum_bytes", U64_TYPE),
];
const TRAIN_FIELDS: [ConfigFieldContract<'static>; 13] = [
    field("job_identity", TEXT_TYPE),
    field("dataset_identity", TEXT_TYPE),
    field("base_model_identity", TEXT_TYPE),
    field("trainer_profile", TEXT_TYPE),
    field("resource_identity", TEXT_TYPE),
    field("deadline_tick", U64_TYPE),
    field("maximum_steps", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("maximum_checkpoints", U64_TYPE),
    field("maximum_checkpoint_bytes", U64_TYPE),
    field("maximum_storage_bytes", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
    field("reproducibility", TEXT_TYPE),
];
const EVALUATE_FIELDS: [ConfigFieldContract<'static>; 9] = [
    field("suite_identity", TEXT_TYPE),
    field("metric_identity", TEXT_TYPE),
    field("metric_version", U64_TYPE),
    field("dataset_identity", TEXT_TYPE),
    field("checkpoint_identity", TEXT_TYPE),
    field("maximum_cases", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
    field("leakage_policy", TEXT_TYPE),
];
const PROMOTE_FIELDS: [ConfigFieldContract<'static>; 8] = [
    field("approval_identity", TEXT_TYPE),
    field("evaluation_report_identity", TEXT_TYPE),
    field("checkpoint_identity", TEXT_TYPE),
    field("target_slot", TEXT_TYPE),
    field("maximum_attempts", U64_TYPE),
    field("maximum_receipt_bytes", U64_TYPE),
    field("commit_policy", TEXT_TYPE),
    field("unknown_commit_policy", TEXT_TYPE),
];

const fn port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    connections: ConnectionCardinality,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections,
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const DATASET_OUTPUT: [PortContract<'static>; 1] = [port(
    "dataset",
    Direction::Output,
    DATASET_SNAPSHOT_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const TRAIN_INPUTS: [PortContract<'static>; 2] = [
    port(
        "dataset",
        Direction::Input,
        DATASET_SNAPSHOT_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    port(
        "model",
        Direction::Input,
        MODEL_ARTIFACT_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const TRAIN_OUTPUTS: [PortContract<'static>; 2] = [
    port(
        "checkpoint",
        Direction::Output,
        CHECKPOINT_TYPE,
        ConnectionCardinality::OneOrMore,
    ),
    port(
        "dataset",
        Direction::Output,
        DATASET_SNAPSHOT_TYPE,
        ConnectionCardinality::OneOrMore,
    ),
];
const EVALUATE_INPUTS: [PortContract<'static>; 2] = [
    port(
        "checkpoint",
        Direction::Input,
        CHECKPOINT_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    port(
        "dataset",
        Direction::Input,
        DATASET_SNAPSHOT_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const EVALUATE_OUTPUTS: [PortContract<'static>; 2] = [
    port(
        "report",
        Direction::Output,
        EVALUATION_REPORT_TYPE,
        ConnectionCardinality::OneOrMore,
    ),
    PortContract {
        id: Id("checkpoint"),
        direction: Direction::Output,
        value_type: CHECKPOINT_TYPE,
        presence: Presence::Optional,
        connections: ConnectionCardinality::ZeroOrOne,
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    },
];
const PROMOTE_INPUTS: [PortContract<'static>; 2] = [
    port(
        "checkpoint",
        Direction::Input,
        CHECKPOINT_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    port(
        "report",
        Direction::Input,
        EVALUATION_REPORT_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const PROMOTION_OUTPUT: [PortContract<'static>; 1] = [port(
    "receipt",
    Direction::Output,
    PROMOTION_RECEIPT_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const DATASET_INPUT: [PortContract<'static>; 1] = [port(
    "dataset",
    Direction::Input,
    DATASET_SNAPSHOT_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const RECEIPT_INPUT: [PortContract<'static>; 1] = [port(
    "receipt",
    Direction::Input,
    PROMOTION_RECEIPT_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const REPORT_INPUT: [PortContract<'static>; 1] = [port(
    "report",
    Direction::Input,
    EVALUATION_REPORT_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const SUMMARY_OUTPUT: [PortContract<'static>; 1] = [PortContract {
    id: Id("summary"),
    direction: Direction::Output,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::OneOrMore,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
}];

pub const DATASET_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/dataset/literal"),
    config: ConfigContract {
        fields: &DATASET_FIELDS,
    },
    inputs: &[],
    outputs: &DATASET_OUTPUT,
};
pub const TRAIN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/train"),
    config: ConfigContract {
        fields: &TRAIN_FIELDS,
    },
    inputs: &TRAIN_INPUTS,
    outputs: &TRAIN_OUTPUTS,
};
pub const EVALUATE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/evaluate"),
    config: ConfigContract {
        fields: &EVALUATE_FIELDS,
    },
    inputs: &EVALUATE_INPUTS,
    outputs: &EVALUATE_OUTPUTS,
};
pub const PROMOTE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/promote"),
    config: ConfigContract {
        fields: &PROMOTE_FIELDS,
    },
    inputs: &PROMOTE_INPUTS,
    outputs: &PROMOTION_OUTPUT,
};
pub const DATASET_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/dataset/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &DATASET_INPUT,
    outputs: &SUMMARY_OUTPUT,
};
pub const PROMOTION_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/promotion/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &RECEIPT_INPUT,
    outputs: &SUMMARY_OUTPUT,
};
pub const EVALUATION_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/evaluation/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &REPORT_INPUT,
    outputs: &SUMMARY_OUTPUT,
};
pub const LEARNED_LIFECYCLE_CONTRACTS: [&NodeContract<'static>; 7] = [
    &DATASET_LITERAL_CONTRACT,
    &TRAIN_CONTRACT,
    &EVALUATE_CONTRACT,
    &PROMOTE_CONTRACT,
    &DATASET_INSPECT_CONTRACT,
    &EVALUATION_INSPECT_CONTRACT,
    &PROMOTION_INSPECT_CONTRACT,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleReason {
    DatasetRevisionMismatch,
    SchemaMismatch,
    SensitivityDenied,
    ResourceExhaustion,
    StaleProvider,
    Cancelled,
    ProviderLost,
    IncompatibleCheckpoint,
    MetricVersionMismatch,
    EvaluationLeakage,
    PromotionDenied,
    UnknownCommit,
    DuplicateCommit,
    WrongType,
}

impl LifecycleReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DatasetRevisionMismatch => "CND-LEARN-010",
            Self::SchemaMismatch => "CND-LEARN-011",
            Self::SensitivityDenied => "CND-LEARN-012",
            Self::ResourceExhaustion => "CND-LEARN-013",
            Self::StaleProvider => "CND-LEARN-014",
            Self::Cancelled => "CND-LEARN-015",
            Self::ProviderLost => "CND-LEARN-016",
            Self::IncompatibleCheckpoint => "CND-LEARN-017",
            Self::MetricVersionMismatch | Self::EvaluationLeakage => "CND-LEARN-018",
            Self::PromotionDenied => "CND-LEARN-019",
            Self::UnknownCommit | Self::DuplicateCommit => "CND-LEARN-020",
            Self::WrongType => "CND-LEARN-021",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleBounds {
    pub maximum_records: usize,
    pub maximum_dataset_bytes: usize,
    pub maximum_steps: usize,
    pub maximum_work: usize,
    pub maximum_checkpoints: usize,
    pub maximum_checkpoint_bytes: usize,
    pub maximum_storage_bytes: usize,
    pub maximum_evidence_events: usize,
    pub deadline_tick: u64,
}

impl LifecycleBounds {
    pub const FIRST_PROOF: Self = Self {
        maximum_records: 4,
        maximum_dataset_bytes: 128,
        maximum_steps: 4,
        maximum_work: 64,
        maximum_checkpoints: 1,
        maximum_checkpoint_bytes: 128,
        maximum_storage_bytes: 256,
        maximum_evidence_events: 16,
        deadline_tick: 20,
    };

    fn validate(self) -> Result<(), LifecycleReason> {
        if self.maximum_records < 4
            || self.maximum_dataset_bytes < DATASET_BYTES.len()
            || self.maximum_steps < 4
            || self.maximum_work < 16
            || self.maximum_checkpoints < 1
            || self.maximum_checkpoint_bytes < CHECKPOINT_BYTES.len()
            || self.maximum_storage_bytes < DATASET_BYTES.len() + CHECKPOINT_BYTES.len()
            || self.maximum_evidence_events < 8
            || self.deadline_tick < 12
        {
            return Err(LifecycleReason::ResourceExhaustion);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrainingDisposition {
    Complete,
    Cancelled,
    ProviderLost,
    StaleProvider,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitDisposition {
    Acknowledged,
    Unknown,
    Duplicate,
}

pub fn train_fixture(
    dataset: &[u8],
    model: &[u8],
    bounds: LifecycleBounds,
    disposition: TrainingDisposition,
) -> Result<Vec<u8>, LifecycleReason> {
    bounds.validate()?;
    match disposition {
        TrainingDisposition::Complete => {}
        TrainingDisposition::Cancelled => return Err(LifecycleReason::Cancelled),
        TrainingDisposition::ProviderLost => return Err(LifecycleReason::ProviderLost),
        TrainingDisposition::StaleProvider => return Err(LifecycleReason::StaleProvider),
    }
    if dataset != DATASET_BYTES {
        return Err(LifecycleReason::DatasetRevisionMismatch);
    }
    if model != MODEL_ARTIFACT_BYTES {
        return Err(LifecycleReason::IncompatibleCheckpoint);
    }
    Ok(CHECKPOINT_BYTES.to_vec())
}

pub fn evaluate_fixture(
    checkpoint: &[u8],
    dataset: &[u8],
    metric_version: u64,
    leakage: bool,
) -> Result<Vec<u8>, LifecycleReason> {
    if checkpoint != CHECKPOINT_BYTES {
        return Err(LifecycleReason::IncompatibleCheckpoint);
    }
    if dataset != DATASET_BYTES {
        return Err(LifecycleReason::DatasetRevisionMismatch);
    }
    if metric_version != 1 {
        return Err(LifecycleReason::MetricVersionMismatch);
    }
    if leakage {
        return Err(LifecycleReason::EvaluationLeakage);
    }
    Ok(EVALUATION_BYTES.to_vec())
}

pub fn promote_fixture(
    checkpoint: &[u8],
    report: &[u8],
    authorized: bool,
    disposition: CommitDisposition,
) -> Result<Vec<u8>, LifecycleReason> {
    if !authorized {
        return Err(LifecycleReason::PromotionDenied);
    }
    if checkpoint != CHECKPOINT_BYTES {
        return Err(LifecycleReason::IncompatibleCheckpoint);
    }
    if report != EVALUATION_BYTES {
        return Err(LifecycleReason::MetricVersionMismatch);
    }
    match disposition {
        CommitDisposition::Acknowledged => Ok(PROMOTION_RECEIPT_BYTES.to_vec()),
        CommitDisposition::Unknown => Err(LifecycleReason::UnknownCommit),
        CommitDisposition::Duplicate => Err(LifecycleReason::DuplicateCommit),
    }
}

fn exact_u64(node: &Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn invalid(code: &'static str, message: &'static str) -> ResolutionError {
    ResolutionError::new(code, message)
}

fn validate_dataset(node: &Node) -> Result<(), ResolutionError> {
    if node.config.len() != DATASET_FIELDS.len()
        || node.config("fixture") != Some("tiny-supervised")
        || node.config("snapshot_identity") != Some(DATASET_SNAPSHOT_IDENTITY)
        || node.config("revision_identity") != Some(DATASET_REVISION_IDENTITY)
        || node.config("provenance_identity") != Some(DATASET_PROVENANCE_IDENTITY)
        || node.config("access_scope") != Some("conduit.scope/learned-fixture")
        || node.config("split") != Some("train")
    {
        return Err(invalid(
            LifecycleReason::DatasetRevisionMismatch.code(),
            "dataset literal requires the exact snapshot and revision",
        ));
    }
    if node.config("feature_schema_identity") != Some(FEATURE_SCHEMA_IDENTITY)
        || node.config("label_schema_identity") != Some(LABEL_SCHEMA_IDENTITY)
    {
        return Err(invalid(
            LifecycleReason::SchemaMismatch.code(),
            "dataset feature and label schemas must match exactly",
        ));
    }
    if node.config("sensitivity") != Some("public") {
        return Err(invalid(
            LifecycleReason::SensitivityDenied.code(),
            "dataset sensitivity is not admitted by this provider",
        ));
    }
    if exact_u64(node, "maximum_records") != Some(4)
        || exact_u64(node, "maximum_bytes") != Some(128)
    {
        return Err(invalid(
            LifecycleReason::ResourceExhaustion.code(),
            "dataset bounds must match the finite provider profile",
        ));
    }
    Ok(())
}

fn validate_train(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == TRAIN_FIELDS.len()
        && node.config("job_identity") == Some(TRAINING_JOB_IDENTITY)
        && node.config("dataset_identity") == Some(DATASET_SNAPSHOT_IDENTITY)
        && node.config("base_model_identity") == Some(MODEL_ARTIFACT_IDENTITY)
        && node.config("trainer_profile") == Some("conduit.learned/trainer/deterministic")
        && node.config("resource_identity") == Some("conduit.learned/resource/training-fixture-0")
        && exact_u64(node, "deadline_tick") == Some(20)
        && exact_u64(node, "maximum_steps") == Some(4)
        && exact_u64(node, "maximum_work") == Some(64)
        && exact_u64(node, "maximum_checkpoints") == Some(1)
        && exact_u64(node, "maximum_checkpoint_bytes") == Some(128)
        && exact_u64(node, "maximum_storage_bytes") == Some(256)
        && exact_u64(node, "maximum_evidence_events") == Some(16)
        && node.config("reproducibility") == Some("exact"))
    .then_some(())
    .ok_or_else(|| {
        invalid(
            "CND-LEARN-013",
            "training requires exact finite job resources and identities",
        )
    })
}

fn validate_evaluate(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == EVALUATE_FIELDS.len()
        && node.config("suite_identity") == Some(EVALUATION_SUITE_IDENTITY)
        && node.config("metric_identity") == Some(METRIC_IDENTITY)
        && exact_u64(node, "metric_version") == Some(1)
        && node.config("dataset_identity") == Some(DATASET_SNAPSHOT_IDENTITY)
        && node.config("checkpoint_identity") == Some(CHECKPOINT_IDENTITY)
        && exact_u64(node, "maximum_cases") == Some(4)
        && exact_u64(node, "maximum_work") == Some(32)
        && exact_u64(node, "maximum_evidence_events") == Some(8)
        && node.config("leakage_policy") == Some("reject-training-overlap"))
    .then_some(())
    .ok_or_else(|| {
        invalid(
            "CND-LEARN-018",
            "evaluation requires the exact suite, metric, leakage policy, and bounds",
        )
    })
}

fn validate_promote(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == PROMOTE_FIELDS.len()
        && node.config("approval_identity") == Some(PROMOTION_APPROVAL_IDENTITY)
        && node.config("evaluation_report_identity") == Some(EVALUATION_REPORT_IDENTITY)
        && node.config("checkpoint_identity") == Some(CHECKPOINT_IDENTITY)
        && node.config("target_slot") == Some("learned/reference")
        && exact_u64(node, "maximum_attempts") == Some(1)
        && exact_u64(node, "maximum_receipt_bytes") == Some(128)
        && node.config("commit_policy") == Some("acknowledged")
        && node.config("unknown_commit_policy") == Some("reconcile"))
    .then_some(())
    .ok_or_else(|| {
        invalid(
            "CND-LEARN-019",
            "promotion requires an exact approval, target, and commit policy",
        )
    })
}

fn validate_empty(node: &Node) -> Result<(), ResolutionError> {
    node.config
        .is_empty()
        .then_some(())
        .ok_or_else(|| invalid("CND-LEARN-021", "inspector accepts no configuration"))
}

fn runtime(reason: LifecycleReason) -> RuntimeError {
    RuntimeError::new(
        reason.code(),
        format!("learned lifecycle failed: {reason:?}"),
    )
}

struct DatasetLiteral;
impl Handler for DatasetLiteral {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(LifecycleReason::WrongType));
        }
        validate_dataset(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        Ok(vec![Value {
            value_type: DATASET_SNAPSHOT_TYPE,
            bytes: DATASET_BYTES.to_vec(),
        }])
    }
}
struct Train;
impl Handler for Train {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate_train(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let [dataset, model] = inputs else {
            return Err(runtime(LifecycleReason::WrongType));
        };
        if dataset.value_type != DATASET_SNAPSHOT_TYPE || model.value_type != MODEL_ARTIFACT_TYPE {
            return Err(runtime(LifecycleReason::WrongType));
        }
        let checkpoint = train_fixture(
            &dataset.bytes,
            &model.bytes,
            LifecycleBounds::FIRST_PROOF,
            TrainingDisposition::Complete,
        )
        .map_err(runtime)?;
        Ok(vec![
            Value {
                value_type: CHECKPOINT_TYPE,
                bytes: checkpoint,
            },
            dataset.clone(),
        ])
    }
}
struct Evaluate;
impl Handler for Evaluate {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate_evaluate(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let [checkpoint, dataset] = inputs else {
            return Err(runtime(LifecycleReason::WrongType));
        };
        if checkpoint.value_type != CHECKPOINT_TYPE || dataset.value_type != DATASET_SNAPSHOT_TYPE {
            return Err(runtime(LifecycleReason::WrongType));
        }
        let report =
            evaluate_fixture(&checkpoint.bytes, &dataset.bytes, 1, false).map_err(runtime)?;
        Ok(vec![
            Value {
                value_type: EVALUATION_REPORT_TYPE,
                bytes: report,
            },
            checkpoint.clone(),
        ])
    }
}
struct Promote;
impl Handler for Promote {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate_promote(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let [checkpoint, report] = inputs else {
            return Err(runtime(LifecycleReason::WrongType));
        };
        if checkpoint.value_type != CHECKPOINT_TYPE || report.value_type != EVALUATION_REPORT_TYPE {
            return Err(runtime(LifecycleReason::WrongType));
        }
        Ok(vec![Value {
            value_type: PROMOTION_RECEIPT_TYPE,
            bytes: promote_fixture(
                &checkpoint.bytes,
                &report.bytes,
                true,
                CommitDisposition::Acknowledged,
            )
            .map_err(runtime)?,
        }])
    }
}
struct DatasetInspect;
impl Handler for DatasetInspect {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [dataset] = inputs else {
            return Err(runtime(LifecycleReason::WrongType));
        };
        if dataset.value_type != DATASET_SNAPSHOT_TYPE || dataset.bytes != DATASET_BYTES {
            return Err(runtime(LifecycleReason::DatasetRevisionMismatch));
        }
        Ok(vec![Value::text("learned:dataset:tiny:train:4:public")])
    }
}
struct PromotionInspect;
impl Handler for PromotionInspect {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [receipt] = inputs else {
            return Err(runtime(LifecycleReason::WrongType));
        };
        if receipt.value_type != PROMOTION_RECEIPT_TYPE || receipt.bytes != PROMOTION_RECEIPT_BYTES
        {
            return Err(runtime(LifecycleReason::UnknownCommit));
        }
        Ok(vec![Value::text(
            "learned:promotion:learned/reference:acknowledged",
        )])
    }
}
struct EvaluationInspect;
impl Handler for EvaluationInspect {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [report] = inputs else {
            return Err(runtime(LifecycleReason::WrongType));
        };
        if report.value_type != EVALUATION_REPORT_TYPE || report.bytes != EVALUATION_BYTES {
            return Err(runtime(LifecycleReason::MetricVersionMismatch));
        }
        Ok(vec![Value::text(
            "learned:evaluation:accuracy@1:4/4:not-approval",
        )])
    }
}

pub fn register_learned_lifecycle_contracts(registry: &mut Registry) {
    for contract in LEARNED_LIFECYCLE_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

pub const PROMOTION_AUTHORITY: SemanticHash = SemanticHash::from_bytes([0x54; 32]);

pub fn register_deterministic_training_provider(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_learned_lifecycle_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, authorities, factory, validator) in [
        (
            &DATASET_LITERAL_CONTRACT,
            "conduit.learned/dataset-literal-deterministic",
            "conduit.learned/lifecycle-artifact",
            "learned-dataset-literal",
            &NO_AUTHORITIES[..],
            (|| Box::new(DatasetLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_dataset as conduit_runtime::ConfigValidator,
        ),
        (
            &TRAIN_CONTRACT,
            "conduit.learned/train-deterministic",
            "conduit.learned/lifecycle-artifact",
            "learned-train",
            &NO_AUTHORITIES[..],
            (|| Box::new(Train) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_train as conduit_runtime::ConfigValidator,
        ),
        (
            &EVALUATE_CONTRACT,
            "conduit.learned/evaluate-deterministic",
            "conduit.learned/lifecycle-artifact",
            "learned-evaluate",
            &NO_AUTHORITIES[..],
            (|| Box::new(Evaluate) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_evaluate as conduit_runtime::ConfigValidator,
        ),
        (
            &DATASET_INSPECT_CONTRACT,
            "conduit.learned/dataset-inspect-deterministic",
            "conduit.learned/lifecycle-artifact",
            "learned-dataset-inspect",
            &NO_AUTHORITIES[..],
            (|| Box::new(DatasetInspect) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_empty as conduit_runtime::ConfigValidator,
        ),
        (
            &EVALUATION_INSPECT_CONTRACT,
            "conduit.learned/evaluation-inspect-deterministic",
            "conduit.learned/lifecycle-artifact",
            "learned-evaluation-inspect",
            &NO_AUTHORITIES[..],
            (|| Box::new(EvaluationInspect) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_empty as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("lifecycle.rs"),
            required_authorities: authorities,
            factory,
            validate_config: validator,
        })?;
    }
    Ok(())
}

pub fn register_deterministic_lifecycle_provider(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_deterministic_training_provider(registry)?;
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    static PROMOTION_AUTHORITIES: [SemanticHash; 1] = [PROMOTION_AUTHORITY];
    for (contract, implementation_id, entrypoint, authorities, factory, validator) in [
        (
            &PROMOTE_CONTRACT,
            "conduit.learned/promote-deterministic",
            "learned-promote",
            &PROMOTION_AUTHORITIES[..],
            (|| Box::new(Promote) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_promote as conduit_runtime::ConfigValidator,
        ),
        (
            &PROMOTION_INSPECT_CONTRACT,
            "conduit.learned/promotion-inspect-deterministic",
            "learned-promotion-inspect",
            &NO_AUTHORITIES[..],
            (|| Box::new(PromotionInspect) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_empty as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id: "conduit.learned/lifecycle-artifact",
            entrypoint,
            source_bytes: include_bytes!("lifecycle.rs"),
            required_authorities: authorities,
            factory,
            validate_config: validator,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../conformance/c4/learned-lifecycle.json");

    #[test]
    fn lifecycle_identities_are_distinct_and_promotion_is_not_a_metric() {
        let identities = [
            DATASET_SNAPSHOT_IDENTITY,
            TRAINING_JOB_IDENTITY,
            CHECKPOINT_IDENTITY,
            EVALUATION_REPORT_IDENTITY,
            PROMOTION_APPROVAL_IDENTITY,
            PROMOTION_RECEIPT_IDENTITY,
        ];
        for (index, left) in identities.iter().enumerate() {
            assert!(identities.iter().skip(index + 1).all(|right| left != right));
        }
        assert_eq!(
            promote_fixture(
                CHECKPOINT_BYTES,
                EVALUATION_BYTES,
                false,
                CommitDisposition::Acknowledged
            ),
            Err(LifecycleReason::PromotionDenied)
        );
        assert_eq!(
            promote_fixture(
                CHECKPOINT_BYTES,
                EVALUATION_BYTES,
                true,
                CommitDisposition::Acknowledged
            )
            .unwrap(),
            PROMOTION_RECEIPT_BYTES
        );
    }

    #[test]
    fn lifecycle_bounds_and_terminal_outcomes_fail_closed() {
        let too_small = LifecycleBounds {
            maximum_work: 1,
            ..LifecycleBounds::FIRST_PROOF
        };
        assert_eq!(
            train_fixture(
                DATASET_BYTES,
                MODEL_ARTIFACT_BYTES,
                too_small,
                TrainingDisposition::Complete
            ),
            Err(LifecycleReason::ResourceExhaustion)
        );
        for (disposition, reason) in [
            (TrainingDisposition::Cancelled, LifecycleReason::Cancelled),
            (
                TrainingDisposition::ProviderLost,
                LifecycleReason::ProviderLost,
            ),
            (
                TrainingDisposition::StaleProvider,
                LifecycleReason::StaleProvider,
            ),
        ] {
            assert_eq!(
                train_fixture(
                    DATASET_BYTES,
                    MODEL_ARTIFACT_BYTES,
                    LifecycleBounds::FIRST_PROOF,
                    disposition
                ),
                Err(reason)
            );
        }
        assert_eq!(
            evaluate_fixture(CHECKPOINT_BYTES, DATASET_BYTES, 2, false),
            Err(LifecycleReason::MetricVersionMismatch)
        );
        assert_eq!(
            evaluate_fixture(CHECKPOINT_BYTES, DATASET_BYTES, 1, true),
            Err(LifecycleReason::EvaluationLeakage)
        );
        assert_eq!(
            promote_fixture(
                CHECKPOINT_BYTES,
                EVALUATION_BYTES,
                true,
                CommitDisposition::Unknown
            ),
            Err(LifecycleReason::UnknownCommit)
        );
        assert_eq!(
            promote_fixture(
                CHECKPOINT_BYTES,
                EVALUATION_BYTES,
                true,
                CommitDisposition::Duplicate
            ),
            Err(LifecycleReason::DuplicateCommit)
        );
    }

    #[test]
    fn inference_only_registry_does_not_install_lifecycle_providers() {
        let mut registry = Registry::default();
        crate::register_deterministic_inference_provider(&mut registry).unwrap();
        assert_eq!(
            registry.node_availability("learned/train").state,
            conduit_runtime::AvailabilityState::Unsupported
        );
        register_learned_lifecycle_contracts(&mut registry);
        assert_eq!(
            registry.node_availability("learned/train").state,
            conduit_runtime::AvailabilityState::ContractOnly
        );
    }

    #[test]
    fn conformance_fixture_owns_the_complete_first_lifecycle_matrix() {
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture["schema"], "conduit.learned-lifecycle-conformance");
        let positive = fixture["positive"].as_array().unwrap();
        let negative = fixture["negative"].as_array().unwrap();
        for name in [
            "identity-layers-remain-distinct",
            "training-without-promotion-provider",
            "promotion-requires-exact-authority",
            "production-executor",
        ] {
            assert!(positive.iter().any(|entry| entry == name), "{name}");
        }
        for name in [
            "dataset-revision-mismatch",
            "sensitivity-denial",
            "resource-exhaustion",
            "stale-provider",
            "cancellation",
            "provider-loss",
            "incompatible-checkpoint",
            "metric-version-mismatch",
            "evaluation-leakage",
            "promotion-without-grant",
            "unknown-commit",
            "duplicate-commit",
        ] {
            assert!(negative.iter().any(|entry| entry == name), "{name}");
        }
    }
}
