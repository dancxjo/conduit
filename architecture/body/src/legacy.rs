//! Decode-only historical Body evidence.
//!
//! These types preserve the exact v1 Seed-bearing schema. They deliberately
//! cannot be converted into current lifecycle truth: doing so would rewrite
//! historical provenance as if it had been emitted by the v2 workset model.

use alloc::{string::String, vec::Vec};
use conduit_core::{CheckedFormId, SignId, SourceDocumentId};
use serde::{Deserialize, Serialize};

use crate::{BodyId, BodyState, BodyWorkset};

pub const LEGACY_BODY_SCHEMA_V1: &str = "conduit.body/legacy-seed-body@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoricalSeedBodyV1 {
    pub body_id: BodyId,
    /// Exact opaque Seed identity from the historical schema.
    pub seed_id: String,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    #[serde(default)]
    pub workset: BodyWorkset,
    #[serde(default)]
    pub workload_revision: u64,
    pub birth_sequence: u64,
    pub state: BodyState,
    pub sign_ids: Vec<SignId>,
    pub events: Vec<HistoricalBodyLifecycleEventV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoricalBodyLifecycleEventV1 {
    Born {
        sign_id: SignId,
    },
    FormAdmitted {
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        workload_revision: u64,
        sign_id: SignId,
    },
    FormRemoved {
        source_document_id: SourceDocumentId,
        checked_form_id: CheckedFormId,
        workload_revision: u64,
        sign_id: SignId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoricalSeedBiographyV1 {
    pub schema: String,
    pub body_id: BodyId,
    pub friendly_name: String,
    pub initial_program: String,
    pub body: HistoricalSeedBodyV1,
}

impl HistoricalSeedBiographyV1 {
    pub fn validate_historical(&self) -> bool {
        self.schema == "conduit.body/biography-evidence@1"
            && self.body_id == self.body.body_id
            && !self.body.seed_id.is_empty()
            && !self.initial_program.is_empty()
            && matches!(
                self.body.events.first(),
                Some(HistoricalBodyLifecycleEventV1::Born { sign_id })
                    if self.body.sign_ids.first() == Some(sign_id)
            )
    }

    pub const fn disclosure_label(&self) -> &'static str {
        "Legacy Seed provenance (historical v1 evidence)"
    }
}
