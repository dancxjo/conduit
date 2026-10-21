//! Immutable protocol values; the only mutable reducer is canonical BirthDraft.
use super::wire::{MAX_BLOB_BYTES, Refusal};
use alloc::{string::String, vec::Vec};
use conduit_birth_plot::{BirthFaceBasis, BirthPlotChoice};
use conduit_face::{FaceInteraction, MaskShow, SemanticApplicationView};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Initialize {
    pub uuid: String,
    #[serde(deserialize_with = "choices")]
    pub choices: Vec<BirthPlotChoice>,
    pub allow_idle_body: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceRequest {
    pub revision: u32,
    pub basis: BirthFaceBasis,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionRequest {
    pub revision: u32,
    pub basis: BirthFaceBasis,
    pub show: MaskShow,
    pub input: FaceInteraction,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftSnapshot {
    pub revision: u32,
    pub native_focus: u32,
    pub native_refusal: Option<String>,
    pub friendly_name: String,
    pub search: String,
    pub requested_system: String,
    pub naming_systems: Vec<(String, String)>,
    #[serde(deserialize_with = "choices")]
    pub choices: Vec<BirthPlotChoice>,
    pub widget: SemanticApplicationView,
}
fn choices<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Vec<BirthPlotChoice>, D::Error> {
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<BirthPlotChoice>;
        fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.write_str("at most sixteen reviewed choices")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut result = Vec::new();
            while let Some(choice) = seq.next_element()? {
                if result.len() == conduit_body::MAX_BODY_PLOTS {
                    return Err(serde::de::Error::custom("choice count"));
                }
                result.push(choice);
            }
            Ok(result)
        }
    }
    decoder.deserialize_seq(Bounded)
}
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    if bytes.is_empty() || bytes.len() > MAX_BLOB_BYTES as usize {
        return Err(Refusal::Bounds);
    }
    serde_json::from_slice(bytes).map_err(|_| Refusal::Malformed)
}
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Refusal> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_BLOB_BYTES as usize)
        .map_err(|_| Refusal::Capacity)?;
    bytes.resize(MAX_BLOB_BYTES as usize, 0);
    let length = serde_json_core::to_slice(value, &mut bytes).map_err(|_| Refusal::Capacity)?;
    bytes.truncate(length);
    if bytes.is_empty() {
        return Err(Refusal::Malformed);
    }
    Ok(bytes)
}
pub fn validate_basis(basis: &BirthFaceBasis) -> Result<(), Refusal> {
    for value in [
        basis.host_id.as_str(),
        basis.boot_id.as_str(),
        basis.encounter_id.as_str(),
        basis.producer_plan_id.as_str(),
        basis.producer_plot.source_document_id.as_str(),
        basis.producer_plot.checked_plot_id.as_str(),
    ] {
        if value.is_empty() || value.len() > 2048 || value.chars().any(char::is_control) {
            return Err(Refusal::Bounds);
        }
    }
    Ok(())
}
