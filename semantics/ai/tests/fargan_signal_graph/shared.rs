use conduit_ai::{
    fixed_numeric_preparation::TENSOR_READ_AUTHORITY,
    fixed_tensor_resource::AdmittedFixedTensorResource,
};
use conduit_core::*;
use conduit_data::*;
use conduit_plot::rust_binding::BoundedSequence;
use std::{collections::BTreeMap, sync::Arc};
pub fn tensor(shape: &[u64], bytes: &[u8]) -> TensorValue {
    let digest = tensor_content_digest(bytes);
    TensorValue {
        element: TensorElement::F32,
        dimensions: BoundedSequence::try_from_iter(shape.iter().copied()).unwrap(),
        axes: BoundedSequence::try_from_iter(shape.iter().map(|_| TensorAxis {
            role: TensorAxisRole::Feature,
            identity: None,
            unit: None,
        }))
        .unwrap(),
        content_digest: digest,
        backing: TensorBacking::Resource(BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: kind_id("tensor/elements-ieee754-f32-le@1"),
            access_class: ResourceClassId::from("test/read@1"),
            extent: ResourceExtent {
                bytes: bytes.len() as u64,
                items: Some(shape.iter().product()),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([1; 32]),
                expires_at: None,
            },
        }),
    }
}
pub fn access(tensor: &TensorValue) -> ResourceReferenceBinding {
    let TensorBacking::Resource(reference) = &tensor.backing else {
        panic!("resource fixture")
    };
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("numeric-test-immutable"),
        authority_contract: AuthorityContractId::from(TENSOR_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("numeric-test-read"),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}
pub fn packed(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|x| x.to_le_bytes()).collect()
}
pub fn fixture_kind(name: &str, value: &StructuredInfoType, source: bool) -> Kind {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: if name == "op-test/index" {
            kind_id("value/u16")
        } else {
            value.profile().unwrap().value_kind().clone()
        },
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(name),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: if source { vec![] } else { vec![port.clone()] },
        outputs: if source { vec![port.clone()] } else { vec![] },
        semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
            location: if source {
                FrontValueLocation::Output(port.port_id)
            } else {
                FrontValueLocation::Input(port.port_id)
            },
            contract: CheckedValueContract::new(
                port.value_kind,
                conduit_plot::maximum_prepared_transport_value_bytes(value).unwrap(),
                vec![],
            )
            .unwrap(),
        }])],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16_384,
        },
    }
}
pub fn offer(kind: Kind) -> CapabilityOffer {
    let name = kind.kind_id.as_str().to_string();
    BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(name.clone()),
            execution_profile_id: ExecutionProfileId::from("numeric-test@1"),
            implementation_id: ImplementationId::from(name.clone()),
            artifact_id: ArtifactId::from(name),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

// This test runs the authored graph with generic numeric and ConduitOS pure
// expression owners. Synthetic tensors test topology, not pretrained parity.

#[derive(Clone)]
pub struct Resource {
    pub value_type: StructuredInfoType,
    pub tensor: Arc<TensorValue>,
    pub bytes: Arc<[u8]>,
}
impl Resource {
    pub fn new(value_type: StructuredInfoType, dimensions: &[u64], values: Vec<f32>) -> Self {
        let bytes: Arc<[u8]> = packed(&values).into();
        let tensor = Arc::new(tensor(dimensions, &bytes));
        Self {
            value_type,
            tensor,
            bytes,
        }
    }
    pub fn adopt(&self) -> Arc<AdmittedFixedTensorResource> {
        Arc::new(
            AdmittedFixedTensorResource::adopt(
                self.tensor.clone(),
                self.bytes.clone(),
                &access(&self.tensor),
            )
            .unwrap(),
        )
    }
}
pub type Resources = BTreeMap<String, Resource>;
