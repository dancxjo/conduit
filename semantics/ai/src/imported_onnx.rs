//! Checkpoint-local names bound to existing semantic model ports during preparation.

use crate::{
    onnx_metadata::{inspect_onnx, OnnxDimension, OnnxMetadata, OnnxMetadataRefusal, OnnxPort},
    AdmittedModelResource, ModelDimensionConstraint, ModelPortConstraint, ModelPortPresence,
    ModelValueConstraint,
};
use alloc::{string::String, vec::Vec};
use conduit_core::semantic_digest;

pub const ONNX_FORMAT_PROFILE: &str = "model/onnx@1";
pub const ONNX_SIDECAR_PROFILE: &str = "model/onnx-bounded-signature@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedPortBinding {
    pub semantic_port: String,
    pub graph_name: String,
}

/// Ordered by semantic signature port order, independently for each direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxSidecar {
    pub graph_sha256: [u8; 32],
    pub graph_name: String,
    pub signature_identity: [u8; 32],
    pub inputs: Vec<ImportedPortBinding>,
    pub outputs: Vec<ImportedPortBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedOnnxRefusal {
    Metadata(OnnxMetadataRefusal),
    Format,
    Signature,
    GraphName,
    PortCount,
    PortName,
    Presence,
    Dtype,
    Rank,
    Dimension,
    SymbolBounds,
}

/// Cannot be constructed from a claimed digest alone: retains the already
/// admitted immutable bytes and read grant alongside checked metadata.
pub struct ImportedOnnxResource {
    resource: AdmittedModelResource,
    metadata: OnnxMetadata,
    sidecar_identity: [u8; 32],
}

impl ImportedOnnxResource {
    pub fn bind(
        resource: AdmittedModelResource,
        sidecar: &OnnxSidecar,
    ) -> Result<Self, ImportedOnnxRefusal> {
        use ImportedOnnxRefusal as R;
        if resource.artifact().format_profile != ONNX_FORMAT_PROFILE {
            return Err(R::Format);
        }
        let signature = resource.signature();
        if signature.operations.get().as_slice() != [crate::ModelOperation::Infer] {
            return Err(R::Signature);
        }
        if signature.semantic_digest().map_err(|_| R::Signature)? != sidecar.signature_identity {
            return Err(R::Signature);
        }
        let metadata = inspect_onnx(resource.bytes(), sidecar.graph_sha256).map_err(R::Metadata)?;
        if metadata.graph_name != sidecar.graph_name {
            return Err(R::GraphName);
        }
        let mut symbols = Vec::new();
        validate_ports(
            &metadata.inputs,
            signature.inputs.get().as_slice(),
            &sidecar.inputs,
            &mut symbols,
        )?;
        validate_ports(
            &metadata.outputs,
            signature.outputs.get().as_slice(),
            &sidecar.outputs,
            &mut symbols,
        )?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&resource.descriptor_identity());
        bytes.extend_from_slice(&sidecar.graph_sha256);
        bytes.extend_from_slice(&sidecar.signature_identity);
        push_text(&mut bytes, &sidecar.graph_name);
        for bindings in [&sidecar.inputs, &sidecar.outputs] {
            bytes.extend_from_slice(&(bindings.len() as u16).to_le_bytes());
            for binding in bindings {
                push_text(&mut bytes, &binding.semantic_port);
                push_text(&mut bytes, &binding.graph_name);
            }
        }
        let sidecar_identity = semantic_digest(ONNX_SIDECAR_PROFILE, &bytes);
        Ok(Self {
            resource,
            metadata,
            sidecar_identity,
        })
    }

    pub fn resource(&self) -> &AdmittedModelResource {
        &self.resource
    }
    pub fn metadata(&self) -> &OnnxMetadata {
        &self.metadata
    }
    pub fn sidecar_identity(&self) -> [u8; 32] {
        self.sidecar_identity
    }
}

fn validate_ports(
    graph: &[OnnxPort],
    semantic: &[ModelPortConstraint],
    bindings: &[ImportedPortBinding],
    symbols: &mut Vec<(String, u64, u64)>,
) -> Result<(), ImportedOnnxRefusal> {
    use ImportedOnnxRefusal as R;
    if graph.len() != semantic.len() || semantic.len() != bindings.len() {
        return Err(R::PortCount);
    }
    for (index, (port, binding)) in semantic.iter().zip(bindings).enumerate() {
        if port.identity.get() != &binding.semantic_port
            || bindings[..index]
                .iter()
                .any(|b| b.graph_name == binding.graph_name)
        {
            return Err(R::PortName);
        }
        let imported = graph
            .iter()
            .find(|p| p.name == binding.graph_name)
            .ok_or(R::PortName)?;
        if port.presence != ModelPortPresence::Required {
            return Err(R::Presence);
        }
        let ModelValueConstraint::Tensor(value) = &port.value else {
            return Err(R::Signature);
        };
        let tensor = value.constraint();
        if tensor.elements.get().as_slice() != [imported.element] {
            return Err(R::Dtype);
        }
        let axes = tensor.axes.get().as_slice();
        if axes.len() != imported.dimensions.len() {
            return Err(R::Rank);
        }
        for (axis, dimension) in axes.iter().zip(&imported.dimensions) {
            let (minimum, maximum) = match &axis.dimension {
                ModelDimensionConstraint::Fixed(v) => (*v.value(), *v.value()),
                ModelDimensionConstraint::Bounded(v) => (*v.minimum(), *v.maximum()),
            };
            match dimension {
                OnnxDimension::Fixed(n) if minimum != *n || maximum != *n => {
                    return Err(R::Dimension)
                }
                OnnxDimension::Symbol(name) => {
                    if minimum == 0 || maximum < minimum {
                        return Err(R::Dimension);
                    }
                    if let Some((_, min, max)) = symbols.iter().find(|(s, _, _)| s == name) {
                        if (*min, *max) != (minimum, maximum) {
                            return Err(R::SymbolBounds);
                        }
                    } else {
                        symbols.push((name.clone(), minimum, maximum));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn push_text(bytes: &mut Vec<u8>, text: &str) {
    // All strings were matched against bounded checked metadata/signature names.
    bytes.extend_from_slice(&(text.len() as u16).to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
}
