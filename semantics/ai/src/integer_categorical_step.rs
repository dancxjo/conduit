//! Ordinary planned categorical inference over an adopted immutable model.
//! No feature extraction, ranking, domain grammar or state mutation lives here.
use crate::{integer_categorical::IntegerCategoricalModel, AdmittedModelResource};
use alloc::{format, string::String, sync::Arc, vec, vec::Vec};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub const CATEGORICAL_STEP_IMPLEMENTATION: &str = "conduit.numeric/resource-categorical-i16-i64@1";

/// A fixed primitive collection codec. Core encoding establishes every payload
/// offset before Play; runtime compares all other canonical bytes exactly.
struct IntegerCollectionCodec {
    template: Vec<u8>,
    offsets: Vec<usize>,
    encoded: Vec<u8>,
}
impl IntegerCollectionCodec {
    fn prepare(ty: &StructuredInfoType, width: usize) -> Result<Self, String> {
        let StructuredInfoTypeShape::Collection { element, length } = ty.shape() else {
            return Err("categorical primitive collection required".into());
        };
        if usize::from(length) != width || width == 0 || width > 128 {
            return Err("categorical collection bound".into());
        }
        let build = |bytes: [u8; 8]| -> Result<Vec<u8>, String> {
            let values = (0..width)
                .map(|_| StructuredInfoValue::leaf(element.clone(), bytes.to_vec()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("{e:?}"))?;
            StructuredInfoValue::collection(ty.clone(), values)
                .and_then(|v| v.canonical_bytes())
                .map_err(|e| format!("{e:?}"))
        };
        let template = build([0; 8])?;
        let marker = [1, 2, 3, 4, 5, 6, 7, 8];
        let marked = build(marker)?;
        let mut offsets = Vec::with_capacity(width);
        let mut cursor = 0;
        while cursor < template.len() {
            if template[cursor] == marked[cursor] {
                cursor += 1;
                continue;
            }
            if cursor + 8 > marked.len()
                || marked[cursor..cursor + 8] != marker
                || template[cursor..cursor + 8] != [0; 8]
            {
                return Err("categorical canonical payload framing".into());
            }
            offsets.push(cursor);
            cursor += 8;
        }
        if offsets.len() != width {
            return Err("categorical canonical payload count".into());
        }
        Ok(Self {
            encoded: template.clone(),
            template,
            offsets,
        })
    }
    fn decode(&self, bytes: &[u8], values: &mut [u64]) -> Result<(), ()> {
        if bytes.len() != self.template.len() || values.len() != self.offsets.len() {
            return Err(());
        }
        let mut previous = 0;
        for (value, &offset) in values.iter_mut().zip(&self.offsets) {
            if bytes[previous..offset] != self.template[previous..offset] {
                return Err(());
            }
            *value = u64::from_le_bytes(bytes[offset..offset + 8].try_into().map_err(|_| ())?);
            previous = offset + 8;
        }
        if bytes[previous..] != self.template[previous..] {
            return Err(());
        }
        Ok(())
    }
    fn encode(&mut self, scores: &[i64]) -> &[u8] {
        for (score, &offset) in scores.iter().zip(&self.offsets) {
            self.encoded[offset..offset + 8].copy_from_slice(&score.to_le_bytes());
        }
        &self.encoded
    }
}

/// Full resource custody is private. The semantic profile binds the complete
/// artifact/reference/signature; its Back identity additionally binds adoption.
pub struct PreparedCategoricalStep {
    resource: Arc<AdmittedModelResource>,
    model: IntegerCategoricalModel,
    indices: StructuredInfoType,
    scores: StructuredInfoType,
    input_bytes: u32,
    output_bytes: u32,
    identity: String,
    adoption_identity: String,
    pool: ResourcePoolId,
    resource_offer: ResourceOffer,
    bound_content: ResourceContentOffer,
}
fn hexadecimal(bytes: [u8; 32]) -> String {
    let mut value = String::new();
    use core::fmt::Write;
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("String formatting");
    }
    value
}
fn text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}
impl PreparedCategoricalStep {
    /// Preparation-time conservative range for typed downstream admission.
    pub fn maximum_score_magnitude(&self) -> u64 {
        self.model.maximum_score_magnitude()
    }
    pub fn prepare(
        resource: Arc<AdmittedModelResource>,
        pool: ResourcePoolId,
        residence: ResourceContentOffer,
    ) -> Result<Self, String> {
        let reference = &resource.artifact().content;
        if reference.extent.items != Some(1)
            || reference.lifetime.expires_at.is_some()
            || pool.as_str().is_empty()
        {
            return Err("one immutable unexpiring model item required".into());
        }
        let model = IntegerCategoricalModel::prepare(
            resource.artifact(),
            resource.signature(),
            resource.bytes(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let (_, outputs, lookups) = model.dimensions();
        let indices = StructuredInfoType::collection(
            StructuredInfoType::leaf(kind_id("value/u64")).map_err(|e| format!("{e:?}"))?,
            Some(u16::try_from(lookups).map_err(|_| "lookup bound")?),
        )
        .map_err(|e| format!("{e:?}"))?;
        let scores = StructuredInfoType::collection(
            StructuredInfoType::leaf(kind_id("value/i64")).map_err(|e| format!("{e:?}"))?,
            Some(u16::try_from(outputs).map_err(|_| "score bound")?),
        )
        .map_err(|e| format!("{e:?}"))?;
        let input_bytes = IntegerCollectionCodec::prepare(&indices, lookups)?
            .template
            .len() as u32;
        let output_bytes = IntegerCollectionCodec::prepare(&scores, outputs)?
            .template
            .len() as u32;
        let content = ResourceContentRequirement {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile.clone(),
            maximum_bytes: u32::try_from(reference.extent.bytes).map_err(|_| "model byte bound")?,
            maximum_items: 1,
            retention: ResourceRetention::Play,
            sharing: ResourceSharing::ImmutableReadMany,
            access: ResourceAccessMode::ReadPublished,
            generation_slots: 1,
            reader_leases: residence.contract.reader_leases,
            publication_slots: 0,
            sensitive: false,
        };
        content
            .accepts_reference(reference)
            .map_err(|e| format!("{e:?}"))?;
        let requirement = ResourceRequirement {
            class_id: reference.access_class.clone(),
            units: 1,
            protected_role: None,
            compute: None,
            content: Some(content),
        };
        let resource_offer = ResourceOffer {
            pool_id: pool.clone(),
            class_id: reference.access_class.clone(),
            capacity_units: u32::from(residence.contract.reader_leases),
            compute: None,
            content: Some(residence.clone()),
        };
        let bound_content = bind_resource_content(
            &requirement,
            &resource_offer,
            &residence.owner_host,
            &residence.owner_boot,
        )
        .map_err(|e| format!("{e:?}"))?
        .ok_or("model content residence missing")?;
        let mut definition = Vec::new();
        definition.extend_from_slice(&resource.descriptor_identity());
        definition.extend_from_slice(&reference.encode().map_err(|e| format!("{e:?}"))?);
        definition.extend_from_slice(&indices.canonical_bytes().map_err(|e| format!("{e:?}"))?);
        definition.extend_from_slice(&scores.canonical_bytes().map_err(|e| format!("{e:?}"))?);
        let identity = hexadecimal(semantic_digest(
            "numeric/resource-categorical-profile@1",
            &definition,
        ));
        let mut adoption = definition;
        text(&mut adoption, resource.access().handle.as_str());
        text(&mut adoption, resource.access().authority_grant.as_str());
        adoption.extend_from_slice(&resource.access().maximum_bytes.to_le_bytes());
        match resource.access().maximum_items {
            Some(items) => {
                adoption.push(1);
                adoption.extend_from_slice(&items.to_le_bytes());
            }
            None => adoption.push(0),
        }
        text(&mut adoption, pool.as_str());
        text(&mut adoption, residence.owner_host.as_str());
        text(&mut adoption, residence.owner_boot.as_str());
        text(&mut adoption, residence.base_id.as_str());
        text(&mut adoption, residence.residence_profile.as_str());
        let adoption_identity = hexadecimal(semantic_digest(
            "numeric/resource-categorical-adoption@1",
            &adoption,
        ));
        Ok(Self {
            resource,
            model,
            indices,
            scores,
            input_bytes,
            output_bytes,
            identity,
            adoption_identity,
            pool,
            resource_offer,
            bound_content,
        })
    }
    pub fn resource(&self) -> &AdmittedModelResource {
        &self.resource
    }
    pub fn dimensions(&self) -> (usize, usize, usize) {
        self.model.dimensions()
    }
    pub fn indices_type(&self) -> &StructuredInfoType {
        &self.indices
    }
    pub fn scores_type(&self) -> &StructuredInfoType {
        &self.scores
    }
    pub fn resource_offer(&self) -> &ResourceOffer {
        &self.resource_offer
    }
    pub fn kind_identity(&self, flow: bool) -> String {
        format!(
            "numeric/{}resource-categorical/{}",
            if flow { "flow-" } else { "" },
            self.identity
        )
    }
    pub fn contract(&self, flow: bool) -> Result<Kind, String> {
        let temporal = if flow {
            PortTemporal::Flow { closes: true }
        } else {
            PortTemporal::Value
        };
        let input = PortDescriptor {
            port_id: port_id("indices"),
            value_kind: self
                .indices
                .profile()
                .map_err(|e| format!("{e:?}"))?
                .value_kind()
                .clone(),
            direction: PortDirection::Input,
            temporal,
            abnormal_kind: None,
        };
        let output = PortDescriptor {
            port_id: port_id("scores"),
            value_kind: self
                .scores
                .profile()
                .map_err(|e| format!("{e:?}"))?
                .value_kind()
                .clone(),
            direction: PortDirection::Output,
            temporal,
            abnormal_kind: None,
        };
        let contracts = [(&input, self.input_bytes), (&output, self.output_bytes)]
            .into_iter()
            .map(|(p, maximum)| FrontValueContract {
                location: match p.direction {
                    PortDirection::Input => FrontValueLocation::Input(p.port_id.clone()),
                    PortDirection::Output => FrontValueLocation::Output(p.port_id.clone()),
                },
                contract: CheckedValueContract::new(p.value_kind.clone(), maximum, vec![])
                    .expect("checked finite integer collection"),
            })
            .collect();
        Ok(Kind {
            kind_id: kind_id(&self.kind_identity(flow)),
            kind_contract_revision: CATEGORICAL_STEP_IMPLEMENTATION.into(),
            startup_parameters: vec![],
            shorthand: None,
            configuration: vec![],
            inputs: vec![input],
            outputs: vec![output],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
            limits: CapabilityLimits {
                max_active_instances: 16,
                max_queue_items: 1,
                max_queue_bytes: self.input_bytes.max(self.output_bytes),
            },
        })
    }
    pub fn offer(&self, flow: bool) -> Result<CapabilityOffer, String> {
        Ok(BackOfferBuilder::new(
            self.contract(flow)?,
            Back {
                capability_id: format!(
                    "{CATEGORICAL_STEP_IMPLEMENTATION}/{}/{flow}",
                    self.adoption_identity
                )
                .into(),
                execution_profile_id: CATEGORICAL_STEP_IMPLEMENTATION.into(),
                implementation_id: CATEGORICAL_STEP_IMPLEMENTATION.into(),
                artifact_id: format!("{CATEGORICAL_STEP_IMPLEMENTATION}/{}", self.identity).into(),
                host_calls: vec![],
                resource_requirements: vec![ResourceRequirement {
                    class_id: self.resource.artifact().content.access_class.clone(),
                    units: 1,
                    protected_role: None,
                    compute: None,
                    content: Some(self.bound_content.contract.clone()),
                }],
                authority_requirements: vec![],
            },
        )
        .build())
    }
    pub fn install(
        &self,
        startup: &mut conduit_plot::StartupCatalog,
        profiles: &mut conduit_plot::ProfileCatalog,
        flow: bool,
    ) -> Result<(), String> {
        let kind = self.contract(flow)?;
        let identity = kind.kind_id.as_str();
        startup.insert(conduit_plot::KindSignature {
            kind: identity.into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(identity, kind.checked_front())?;
        startup.insert_structured_type(format!("{identity}/indices"), self.indices.clone())?;
        startup.insert_structured_type(format!("{identity}/scores"), self.scores.clone())?;
        profiles.insert_kind(kind).map_err(|e| format!("{e:?}"))
    }
    pub fn verify_placement(&self, gear: &PlannedGear, flow: bool) -> Result<(), String> {
        let expected = self.offer(flow)?;
        if gear.kind_id != expected.kind_id
            || gear.kind_contract_revision != expected.kind_contract_revision
            || gear.capability_id != expected.capability_id
            || gear.execution_profile_id != expected.implementation.execution_profile_id
            || gear.implementation_id != expected.implementation.implementation_id
            || gear.artifact_id != expected.implementation.artifact_id
            || gear.inputs != expected.inputs
            || gear.outputs != expected.outputs
            || gear.limits != expected.limits
            || gear.semantic_contract != expected.semantic_contract
            || !gear.configuration.is_empty()
            || !gear.host_calls.is_empty()
            || !gear.authority.is_empty()
            || gear.base.is_some()
            || !gear.realization_characteristics.is_empty()
            || !gear.realization_properties.is_empty()
            || !gear.pool_references.is_empty()
            || !gear.terminal_transductions.is_empty()
            || gear.host_id != self.bound_content.owner_host
            || gear.boot_id != self.bound_content.owner_boot
            || gear.resources.len() != 1
        {
            return Err("categorical exact selected placement".into());
        }
        let binding = &gear.resources[0];
        if binding.pool_id != self.pool
            || binding.class_id != self.resource.artifact().content.access_class
            || binding.units != 1
            || binding.protected.is_some()
            || binding.compute.is_some()
            || binding.content.as_ref() != Some(&self.bound_content)
        {
            return Err("categorical exact model content binding".into());
        }
        Ok(())
    }
}

mod back;
pub use back::{CategoricalStepBack, CategoricalStepReceipt};

#[cfg(all(target_has_atomic = "ptr", target_has_atomic = "8"))]
pub mod invocation;

#[cfg(feature = "kernel-operation-owners")]
pub mod owner;
