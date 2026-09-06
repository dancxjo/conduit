use crate::proof::ProofClass;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const OBLIGATION_SCHEMA_VERSION: u16 = 1;
pub const SPECIMEN_COMMAND: &str = "cargo xtask proofs --json";
pub const SPECIMEN_TOOL: &str = "cargo-xtask";
pub const SPECIMEN_PROFILE: &str = "conduit.repo/proof-catalog-validation@1";
pub const SPECIMEN_ARTIFACT: &str = "tools/xtask/src/proof.rs";
pub const MAX_ATTEMPTS: u8 = 4;
pub const MAX_RETAINED_ATTEMPTS: usize = 3;
pub const MAX_SIGNS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObligationVerdict {
    Completed,
    Interrupted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidualStep {
    ExecuteProofCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationBasis {
    pub source_commit: String,
    pub command: String,
    pub tool: String,
    pub profile: String,
    pub artifact: String,
    pub artifact_digest: String,
    pub proof_class: ProofClass,
}

impl ObligationBasis {
    pub fn current(source_commit: String) -> Self {
        Self {
            source_commit,
            command: SPECIMEN_COMMAND.into(),
            tool: SPECIMEN_TOOL.into(),
            profile: SPECIMEN_PROFILE.into(),
            artifact: SPECIMEN_ARTIFACT.into(),
            artifact_digest: digest(include_bytes!("../proof.rs")),
            proof_class: ProofClass::DeterministicUnit,
        }
    }

    pub fn identity(&self) -> String {
        digest(serde_json::to_vec(self).expect("basis is serializable"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationCheckpoint {
    pub schema_version: u16,
    pub obligation_id: String,
    pub basis: ObligationBasis,
    pub checkpoint_id: String,
    pub completed_steps: Vec<String>,
    pub residual: Vec<ResidualStep>,
    pub attempts_used: u8,
}

impl ObligationCheckpoint {
    pub fn validate(&self, expected: &ObligationBasis) -> Result<(), ObligationRefusal> {
        if self.schema_version != OBLIGATION_SCHEMA_VERSION {
            return Err(ObligationRefusal::CorruptCheckpoint);
        }
        if &self.basis != expected {
            return Err(classify_basis_change(&self.basis, expected));
        }
        if self.obligation_id != obligation_id(expected)
            || self.completed_steps != ["basis-checked"]
            || self.residual != [ResidualStep::ExecuteProofCatalog]
            || self.attempts_used == 0
            || self.attempts_used > MAX_ATTEMPTS
            || self.checkpoint_id != checkpoint_id(expected, self.attempts_used)
        {
            return Err(ObligationRefusal::CorruptCheckpoint);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptRecord {
    pub attempt_id: String,
    pub play_id: String,
    pub checkpoint_id: Option<String>,
    pub receipt: Option<ValidationReceipt>,
    pub verdict: ObligationVerdict,
    pub signs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReceipt {
    pub receipt_id: String,
    pub command: String,
    pub artifact_digest: String,
    pub succeeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObligationRecord {
    pub schema_version: u16,
    pub obligation_id: String,
    pub basis: ObligationBasis,
    pub form_id: String,
    pub plan_id: String,
    pub attempts: Vec<AttemptRecord>,
    pub retention_gap: u64,
    pub checkpoint: Option<ObligationCheckpoint>,
    pub terminal_verdict: Option<ObligationVerdict>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationRefusal {
    StaleCommit,
    ChangedCommand,
    ChangedProfile,
    IncompatibleArtifact,
    CorruptCheckpoint,
    AttemptBudgetExhausted,
    StepFailed,
}

fn classify_basis_change(
    actual: &ObligationBasis,
    expected: &ObligationBasis,
) -> ObligationRefusal {
    if actual.source_commit != expected.source_commit {
        ObligationRefusal::StaleCommit
    } else if actual.command != expected.command || actual.tool != expected.tool {
        ObligationRefusal::ChangedCommand
    } else if actual.profile != expected.profile || actual.proof_class != expected.proof_class {
        ObligationRefusal::ChangedProfile
    } else {
        ObligationRefusal::IncompatibleArtifact
    }
}

pub fn obligation_id(basis: &ObligationBasis) -> String {
    digest(format!("conduit-obligation-v1:{}", basis.identity()))
}

pub fn checkpoint_id(basis: &ObligationBasis, attempts_used: u8) -> String {
    digest(format!(
        "checkpoint:{}:{attempts_used}",
        obligation_id(basis)
    ))
}

pub fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
