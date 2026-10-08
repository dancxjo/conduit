//! Finite development preparation recipe. Local grants are deliberately absent:
//! the selected synthetic proof Root must establish fresh custody for this Boot.
use alloc::{string::String, vec::Vec};
use conduit_core::{PlanId, PlannedGear, SourceDocumentId, StructuredInfoType};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Recipe {
    pub schema: String,
    pub entry_plot: String,
    pub u16_profile_source: String,
    pub scope: String,
    pub native: Vec<Native>,
    pub weakening: Vec<Input>,
    pub guards: Vec<Input>,
    pub pairs: Vec<Pair>,
    pub reference_fixture_placements: Vec<PlannedGear>,
    pub resources: Vec<Resource>,
    pub inputs: Vec<Ingress>,
    pub resource_bytes: usize,
    pub source_document_id: SourceDocumentId,
    pub reference_plan_id: PlanId,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Native {
    pub name: String,
    pub kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    pub input_type_canonical: Vec<u8>,
    pub kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pair {
    pub left_type_canonical: Vec<u8>,
    pub right_type_canonical: Vec<u8>,
    pub kind: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resource {
    pub name: String,
    pub type_canonical: Vec<u8>,
    pub descriptor: Vec<u8>,
    pub guest_access: String,
    pub content_file: String,
    pub content_bytes: usize,
    pub content_sha256: String,
    pub port_binding: Vec<u8>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Ingress {
    pub name: String,
    pub file: String,
    pub bytes: usize,
    pub sha256: String,
}

impl Recipe {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("numeric recipe byte bound".into());
        }
        let recipe: Self =
            serde_json::from_slice(bytes).map_err(|_| String::from("numeric recipe encoding"))?;
        if recipe.schema != "conduit/synthetic-numeric-guest-preparation@1"
            || recipe.entry_plot.is_empty()
            || recipe.entry_plot.len() > 256
            || recipe.scope.len() > 1024
            || recipe.u16_profile_source.len() > 16384
            || recipe.native.len() > 16
            || recipe.weakening.len() > 16
            || recipe.guards.len() > 16
            || recipe.pairs.len() > 16
            || recipe.reference_fixture_placements.len() > 64
            || recipe.resources.len() > 64
            || recipe.inputs.len() > 8
            || recipe.resource_bytes > 8 * 1024 * 1024
        {
            return Err("numeric recipe selected capacity".into());
        }
        for row in &recipe.resources {
            if row.guest_access != "fresh synthetic Root read grant required"
                || row.name.len() > 256
                || row.descriptor.len() > 16384
                || row.content_bytes > 1024 * 1024
                || row.port_binding.len() > 16384
                || !file_name(&row.content_file)
                || row.content_sha256.len() != 64
            {
                return Err("numeric resource recipe bound".into());
            }
            schema(&row.type_canonical)?;
        }
        for row in &recipe.inputs {
            if row.name.len() > 256
                || !file_name(&row.file)
                || row.bytes > 16384
                || row.sha256.len() != 64
            {
                return Err("numeric ingress recipe bound".into());
            }
        }
        for row in recipe.weakening.iter().chain(&recipe.guards) {
            schema(&row.input_type_canonical)?;
        }
        for row in &recipe.pairs {
            schema(&row.left_type_canonical)?;
            schema(&row.right_type_canonical)?;
        }
        Ok(recipe)
    }
}
fn file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        && !name.contains("..")
}
pub(super) fn schema(bytes: &[u8]) -> Result<StructuredInfoType, String> {
    if bytes.is_empty() || bytes.len() > 16384 {
        return Err("numeric recipe schema bound".into());
    }
    StructuredInfoType::from_canonical_bytes(bytes)
        .map_err(|_| String::from("numeric recipe exact schema"))
}
