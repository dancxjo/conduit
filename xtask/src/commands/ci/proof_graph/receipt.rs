use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const RECEIPT_SCHEMA: &str = "conduit.ci.proof-receipt/v1";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct ProofReceipt {
    pub(super) schema: String,
    pub(super) proof_id: String,
    pub(super) proof_contract_version: u32,
    pub(super) candidate_sha: String,
    pub(super) source_tree: String,
    pub(super) input_digest: String,
    pub(super) proof_key: String,
    pub(super) result: String,
    pub(super) artifact_digests: BTreeMap<String, String>,
    pub(super) evidence: Vec<String>,
}

pub(super) enum ReceiptLoad {
    Valid(Box<ProofReceipt>),
    Invalid,
}

pub(super) fn load_receipts(paths: &[PathBuf]) -> Vec<ReceiptLoad> {
    paths
        .iter()
        .map(|path| match read_receipt(path) {
            Some(receipt) if receipt.schema == RECEIPT_SCHEMA => {
                ReceiptLoad::Valid(Box::new(receipt))
            }
            _ => ReceiptLoad::Invalid,
        })
        .collect()
}

fn read_receipt(path: &Path) -> Option<ProofReceipt> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

pub(super) fn receipt_matches(
    receipt: &ProofReceipt,
    proof_id: &str,
    contract_version: u32,
    consumed_artifacts: &[&str],
    input_digest: &str,
    key: &str,
) -> bool {
    receipt.result == "success"
        && receipt.proof_id == proof_id
        && receipt.proof_contract_version == contract_version
        && receipt.input_digest == input_digest
        && receipt.proof_key == key
        && receipt.artifact_digests.len() == consumed_artifacts.len()
        && receipt.artifact_digests.iter().all(|(name, digest)| {
            consumed_artifacts.contains(&name.as_str()) && valid_digest(digest)
        })
        && receipt.evidence.iter().all(|item| !item.is_empty())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
