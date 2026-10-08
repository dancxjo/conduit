//! Synthetic proof Root establishes fresh current-Boot read custody.
//! These resources are not a pretrained ModelArtifact or a voice admission.
use super::{PreparedTopology, recipe};
use alloc::{collections::BTreeMap, format, string::String, sync::Arc, vec::Vec};
use conduit_ai::{
    fixed_numeric_binding::FixedTensorPortBinding,
    fixed_tensor_resource::AdmittedFixedTensorResource,
};
use conduit_core::*;
use conduit_data::{TensorBacking, TensorValue};
use sha2::{Digest, Sha256};

pub struct File<'a> {
    pub name: &'a str,
    pub bytes: &'a [u8],
}
pub struct PreparedIngress {
    pub resources: BTreeMap<PlacementId, Arc<AdmittedFixedTensorResource>>,
    pub values: BTreeMap<KindId, Vec<u8>>,
    pub raw_resource_bytes: usize,
    pub descriptor_inline_bytes: usize,
}
impl PreparedIngress {
    pub fn prepare(topology: &PreparedTopology<'_>, files: &[File<'_>]) -> Result<Self, String> {
        let mut resources = BTreeMap::new();
        let mut values = BTreeMap::new();
        let mut raw_resource_bytes = 0usize;
        let mut descriptor_inline_bytes = 0usize;
        let fragment = &topology.plan.fragments[0];
        for row in &topology.recipe.resources {
            let bytes = file(files, &row.content_file)?;
            check_bytes(bytes, row.content_bytes, &row.content_sha256)?;
            let tensor = Arc::new(
                TensorValue::decode(&row.descriptor)
                    .map_err(|_| String::from("synthetic tensor descriptor"))?,
            );
            let TensorBacking::Resource(reference) = &tensor.backing else {
                return Err("synthetic tensor requires immutable resource".into());
            };
            let access = ResourceReferenceBinding {
                identity: reference.identity,
                version: reference.lifetime.version,
                content_profile: reference.content_profile.clone(),
                access_class: reference.access_class.clone(),
                handle: format!("numeric-proof/{}/{}", fragment.boot_id.as_str(), row.name).into(),
                authority_contract: conduit_ai::fixed_numeric_preparation::TENSOR_READ_AUTHORITY
                    .into(),
                authority_grant: format!(
                    "numeric-proof-root/{}/{}",
                    fragment.host_id.as_str(),
                    fragment.boot_id.as_str()
                )
                .into(),
                maximum_bytes: reference.extent.bytes,
                maximum_items: reference.extent.items,
                availability: ResourceReferenceAvailability::Available,
            };
            let binding =
                FixedTensorPortBinding::prepare(&recipe::schema(&row.type_canonical)?, &tensor)
                    .map_err(|_| String::from("synthetic exact tensor port"))?;
            if binding.encoded() != row.port_binding {
                return Err("synthetic retained port binding differs".into());
            }
            let gear = boundary(topology, &row.name, true)?;
            if values
                .insert(gear.kind_id.clone(), binding.encoded().into())
                .is_some()
            {
                return Err("duplicate synthetic ingress".into());
            }
            let resource = Arc::new(
                AdmittedFixedTensorResource::adopt(tensor, Arc::from(bytes), &access)
                    .map_err(|_| String::from("synthetic fresh resource custody"))?,
            );
            raw_resource_bytes = raw_resource_bytes
                .checked_add(bytes.len())
                .ok_or("resource extent overflow")?;
            descriptor_inline_bytes = descriptor_inline_bytes
                .checked_add(core::mem::size_of::<TensorValue>())
                .ok_or("descriptor extent overflow")?;
            if resources
                .insert(gear.placement_id.clone(), resource)
                .is_some()
            {
                return Err("duplicate synthetic resource placement".into());
            }
        }
        if raw_resource_bytes != topology.recipe.resource_bytes {
            return Err("synthetic whole resource extent".into());
        }
        for row in &topology.recipe.inputs {
            let bytes = file(files, &row.file)?;
            check_bytes(bytes, row.bytes, &row.sha256)?;
            let gear = boundary(topology, &row.name, true)?;
            if values.insert(gear.kind_id.clone(), bytes.into()).is_some() {
                return Err("duplicate synthetic input".into());
            }
        }
        Ok(Self {
            resources,
            values,
            raw_resource_bytes,
            descriptor_inline_bytes,
        })
    }
}
fn boundary<'a>(
    topology: &'a PreparedTopology<'_>,
    name: &str,
    source: bool,
) -> Result<&'a PlannedGear, String> {
    // Names correlate an exact reference boundary; no prefix discovers model layers.
    let original = topology
        .recipe
        .reference_fixture_placements
        .iter()
        .filter(|p| {
            let ports = if source { &p.outputs } else { &p.inputs };
            ports.len() == 1
                && ports[0].port_id.as_str() == "value"
                && p.kind_id.as_str().rsplit('/').next() == Some(name)
        })
        .collect::<Vec<_>>();
    if original.len() != 1 {
        return Err("synthetic exact named boundary ambiguity".into());
    }
    let matches = topology.plan.fragments[0]
        .placements
        .iter()
        .filter(|p| p.kind_id == original[0].kind_id)
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err("synthetic current boundary ambiguity".into());
    }
    Ok(matches[0])
}
fn file<'a>(files: &'a [File<'a>], name: &str) -> Result<&'a [u8], String> {
    let mut matches = files.iter().filter(|f| f.name == name);
    let first = matches.next().ok_or("missing retained synthetic content")?;
    if matches.next().is_some() {
        return Err("duplicate retained synthetic content".into());
    }
    Ok(first.bytes)
}
fn check_bytes(bytes: &[u8], length: usize, digest: &str) -> Result<(), String> {
    if bytes.len() != length || format!("{:x}", Sha256::digest(bytes)) != digest {
        return Err("synthetic content extent/digest".into());
    }
    Ok(())
}
