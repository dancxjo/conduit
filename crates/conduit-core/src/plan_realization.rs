use super::{push_string, push_u32, CheckedFormId, KindContractRevision, KindId, SourceDocumentId};
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

/// Exact reusable Form selected while expanding one high-level Kind invocation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RealizationBack {
    pub invocation_path: String,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
}

pub(super) fn push_canonical(canonical: &mut Vec<u8>, backs: &[RealizationBack]) {
    push_string(canonical, "canonical-realization-backs@1");
    push_u32(canonical, backs.len() as u32);
    for back in backs {
        push_string(canonical, &back.invocation_path);
        push_string(canonical, back.kind_id.as_str());
        push_string(canonical, back.kind_contract_revision.as_str());
        push_string(canonical, back.source_document_id.as_str());
        push_string(canonical, back.checked_form_id.as_str());
    }
}
