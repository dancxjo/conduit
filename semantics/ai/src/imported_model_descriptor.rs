//! Inspectable selected-Plan correspondence, independent of model tensor data.
use crate::{
    imported_back_admission::{ImportedBackAdmissionReceipt, ImportedCheckpointIdentity},
    imported_onnx::{ImportedOnnxResource, ImportedPortBinding},
    MAXIMUM_MODEL_IDENTITY_BYTES,
};
use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, valid_realization_properties, KindId, PlannedGear, StructuredConfigurationValue,
    StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, StructuredInfoValueShape, BYTES_INFO_ID,
    MAXIMUM_REALIZATION_PROPERTY_BYTES, TEXT_INFO_ID,
};

pub const IMPORTED_MODEL_INSPECTION_SCHEMA: &str = "conduit.ai/imported-model-inspection@1";
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportedInspectionRefusal {
    Encoding(StructuredInfoRefusal),
    Bound,
    Schema,
    MissingProperty,
    InvalidProperties,
    DuplicateName,
    Checkpoint,
    SelectedBack,
    Signature,
}
impl From<StructuredInfoRefusal> for ImportedInspectionRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Encoding(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedModelInspection {
    pub checkpoint: ImportedCheckpointIdentity,
    pub graph_name: String,
    pub inputs: Vec<ImportedPortBinding>,
    pub outputs: Vec<ImportedPortBinding>,
    pub implementation_identity: String,
    pub adapter_artifact_identity: String,
    pub execution_profile: String,
    pub back_profile_identity: String,
    pub admission_identity: [u8; 32],
}
impl ImportedModelInspection {
    pub fn from_resource(
        resource: &ImportedOnnxResource,
        receipt: &ImportedBackAdmissionReceipt,
    ) -> Result<Self, ImportedInspectionRefusal> {
        let checkpoint = ImportedCheckpointIdentity::from_resource(resource);
        if receipt.checkpoint() != &checkpoint {
            return Err(ImportedInspectionRefusal::Checkpoint);
        }
        let profile = receipt.profile();
        let value = Self {
            checkpoint,
            graph_name: resource.metadata().graph_name.clone(),
            inputs: resource.sidecar().inputs.clone(),
            outputs: resource.sidecar().outputs.clone(),
            implementation_identity: profile.implementation_identity.clone(),
            adapter_artifact_identity: profile.adapter_artifact_identity.clone(),
            execution_profile: profile.execution_profile.clone(),
            back_profile_identity: profile.profile_identity.clone(),
            admission_identity: receipt.identity(),
        };
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<(), ImportedInspectionRefusal> {
        for value in [
            &self.graph_name,
            &self.implementation_identity,
            &self.adapter_artifact_identity,
            &self.execution_profile,
            &self.back_profile_identity,
        ] {
            checked_name(value, MAXIMUM_MODEL_IDENTITY_BYTES)?;
        }
        checked_name(&self.graph_name, 128)?;
        if self.inputs.len() + self.outputs.len() > 32 {
            return Err(ImportedInspectionRefusal::Bound);
        }
        for ports in [&self.inputs, &self.outputs] {
            for (index, port) in ports.iter().enumerate() {
                checked_name(&port.semantic_port, MAXIMUM_MODEL_IDENTITY_BYTES)?;
                checked_name(&port.graph_name, 128)?;
                if ports[..index].iter().any(|p| {
                    p.semantic_port == port.semantic_port || p.graph_name == port.graph_name
                }) {
                    return Err(ImportedInspectionRefusal::DuplicateName);
                }
            }
        }
        Ok(())
    }
    pub fn property_profile() -> Result<KindId, ImportedInspectionRefusal> {
        Ok(schema()?.profile()?.value_kind().clone())
    }
    pub fn to_property(&self) -> Result<StructuredConfigurationValue, ImportedInspectionRefusal> {
        self.validate()?;
        let ty = schema()?;
        let mut fields = Vec::new();
        for (name, value) in [
            ("graph-name", &self.graph_name),
            ("implementation", &self.implementation_identity),
            ("adapter-artifact", &self.adapter_artifact_identity),
            ("execution-profile", &self.execution_profile),
            ("back-profile", &self.back_profile_identity),
        ] {
            fields.push(StructuredFieldValue::new(name, text(value)?)?);
        }
        for (name, value) in [
            ("descriptor-id", &self.checkpoint.descriptor),
            ("graph-sha256", &self.checkpoint.graph),
            ("signature-id", &self.checkpoint.signature),
            ("sidecar-id", &self.checkpoint.sidecar),
            ("admission-id", &self.admission_identity),
        ] {
            fields.push(StructuredFieldValue::new(name, bytes(value)?)?);
        }
        fields.push(StructuredFieldValue::new(
            "inputs",
            bindings(&self.inputs)?,
        )?);
        fields.push(StructuredFieldValue::new(
            "outputs",
            bindings(&self.outputs)?,
        )?);
        let canonical = StructuredInfoValue::record(ty, fields)?.canonical_bytes()?;
        if canonical.len() > MAXIMUM_REALIZATION_PROPERTY_BYTES {
            return Err(ImportedInspectionRefusal::Bound);
        }
        StructuredConfigurationValue::new(Self::property_profile()?, canonical)
            .ok_or(ImportedInspectionRefusal::Schema)
    }
    pub fn from_property(
        property: &StructuredConfigurationValue,
    ) -> Result<Self, ImportedInspectionRefusal> {
        if property.canonical_value().len() > MAXIMUM_REALIZATION_PROPERTY_BYTES {
            return Err(ImportedInspectionRefusal::Bound);
        }
        if property.profile() != &Self::property_profile()? {
            return Err(ImportedInspectionRefusal::Schema);
        }
        let value = StructuredInfoValue::from_canonical_bytes(property.canonical_value())?;
        if value.value_type() != &schema()? {
            return Err(ImportedInspectionRefusal::Schema);
        }
        let result = Self {
            checkpoint: ImportedCheckpointIdentity {
                descriptor: read_id(field(&value, "descriptor-id")?)?,
                graph: read_id(field(&value, "graph-sha256")?)?,
                signature: read_id(field(&value, "signature-id")?)?,
                sidecar: read_id(field(&value, "sidecar-id")?)?,
            },
            graph_name: read_text(field(&value, "graph-name")?)?,
            inputs: read_bindings(field(&value, "inputs")?)?,
            outputs: read_bindings(field(&value, "outputs")?)?,
            implementation_identity: read_text(field(&value, "implementation")?)?,
            adapter_artifact_identity: read_text(field(&value, "adapter-artifact")?)?,
            execution_profile: read_text(field(&value, "execution-profile")?)?,
            back_profile_identity: read_text(field(&value, "back-profile")?)?,
            admission_identity: read_id(field(&value, "admission-id")?)?,
        };
        result.validate()?;
        // Exact canonical correspondence, rather than accepting normalized aliases.
        if result.to_property()?.canonical_value() != property.canonical_value() {
            return Err(ImportedInspectionRefusal::Schema);
        }
        Ok(result)
    }
    /// Reads the actual selected Gear carrier; no model bytes, paths or tensor
    /// payloads are involved. This inspection value is not execution authority.
    pub fn from_planned_gear(gear: &PlannedGear) -> Result<Self, ImportedInspectionRefusal> {
        if !valid_realization_properties(&gear.realization_properties) {
            return Err(ImportedInspectionRefusal::InvalidProperties);
        }
        let profile = Self::property_profile()?;
        let property = gear
            .realization_properties
            .iter()
            .find(|p| p.profile() == &profile)
            .ok_or(ImportedInspectionRefusal::MissingProperty)?;
        let value = Self::from_property(property)?;
        if value.implementation_identity != gear.implementation_id.as_str()
            || value.adapter_artifact_identity != gear.artifact_id.as_str()
            || value.execution_profile != gear.execution_profile_id.as_str()
        {
            return Err(ImportedInspectionRefusal::SelectedBack);
        }
        let signature = crate::model_infer_catalog::configured_model_signature(&gear.configuration)
            .map_err(|_| ImportedInspectionRefusal::Signature)?;
        if signature
            .semantic_digest()
            .map_err(|_| ImportedInspectionRefusal::Signature)?
            != value.checkpoint.signature
        {
            return Err(ImportedInspectionRefusal::Signature);
        }
        for (ports, bindings) in [
            (signature.inputs.get().as_slice(), value.inputs.as_slice()),
            (signature.outputs.get().as_slice(), value.outputs.as_slice()),
        ] {
            if ports.len() != bindings.len()
                || ports
                    .iter()
                    .zip(bindings)
                    .any(|(p, b)| p.identity.get() != &b.semantic_port)
            {
                return Err(ImportedInspectionRefusal::Signature);
            }
        }
        Ok(value)
    }
}
fn checked_name(value: &str, maximum: usize) -> Result<(), ImportedInspectionRefusal> {
    if value.is_empty() || value.len() > maximum || value.as_bytes().contains(&0) {
        Err(ImportedInspectionRefusal::Bound)
    } else {
        Ok(())
    }
}
fn schema() -> Result<StructuredInfoType, StructuredInfoRefusal> {
    let mut fields = Vec::new();
    for name in [
        "graph-name",
        "implementation",
        "adapter-artifact",
        "execution-profile",
        "back-profile",
    ] {
        fields.push(StructuredFieldType::new(
            name,
            StructuredInfoType::leaf(kind_id(TEXT_INFO_ID))?,
        )?);
    }
    for name in [
        "descriptor-id",
        "graph-sha256",
        "signature-id",
        "sidecar-id",
        "admission-id",
    ] {
        fields.push(StructuredFieldType::new(
            name,
            StructuredInfoType::leaf(kind_id(BYTES_INFO_ID))?,
        )?);
    }
    for name in ["inputs", "outputs"] {
        fields.push(StructuredFieldType::new(name, bindings_type()?)?);
    }
    StructuredInfoType::record(kind_id(IMPORTED_MODEL_INSPECTION_SCHEMA), fields)
}
fn binding_type() -> Result<StructuredInfoType, StructuredInfoRefusal> {
    StructuredInfoType::record(
        kind_id("conduit.ai/imported-port-correspondence@1"),
        vec![
            StructuredFieldType::new(
                "semantic-port",
                StructuredInfoType::leaf(kind_id(TEXT_INFO_ID))?,
            )?,
            StructuredFieldType::new(
                "graph-name",
                StructuredInfoType::leaf(kind_id(TEXT_INFO_ID))?,
            )?,
        ],
    )
}
fn bindings_type() -> Result<StructuredInfoType, StructuredInfoRefusal> {
    StructuredInfoType::bounded_sequence(binding_type()?, 0, 32)
}
fn text(value: &str) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(TEXT_INFO_ID))?,
        value.as_bytes().to_vec(),
    )
}
fn bytes(value: &[u8]) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(BYTES_INFO_ID))?,
        value.to_vec(),
    )
}
fn bindings(ports: &[ImportedPortBinding]) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    let values = ports
        .iter()
        .map(|p| {
            StructuredInfoValue::record(
                binding_type()?,
                vec![
                    StructuredFieldValue::new("semantic-port", text(&p.semantic_port)?)?,
                    StructuredFieldValue::new("graph-name", text(&p.graph_name)?)?,
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    StructuredInfoValue::sequence(bindings_type()?, values)
}
fn field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, ImportedInspectionRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => fields
            .iter()
            .find(|f| f.name() == name)
            .map(|f| f.value())
            .ok_or(ImportedInspectionRefusal::Schema),
        _ => Err(ImportedInspectionRefusal::Schema),
    }
}
fn leaf(value: &StructuredInfoValue) -> Result<&[u8], ImportedInspectionRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => Ok(bytes),
        _ => Err(ImportedInspectionRefusal::Schema),
    }
}
fn read_text(value: &StructuredInfoValue) -> Result<String, ImportedInspectionRefusal> {
    core::str::from_utf8(leaf(value)?)
        .map(String::from)
        .map_err(|_| ImportedInspectionRefusal::Schema)
}
fn read_id(value: &StructuredInfoValue) -> Result<[u8; 32], ImportedInspectionRefusal> {
    leaf(value)?
        .try_into()
        .map_err(|_| ImportedInspectionRefusal::Schema)
}
fn read_bindings(
    value: &StructuredInfoValue,
) -> Result<Vec<ImportedPortBinding>, ImportedInspectionRefusal> {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        return Err(ImportedInspectionRefusal::Schema);
    };
    values
        .iter()
        .map(|v| {
            Ok(ImportedPortBinding {
                semantic_port: read_text(field(v, "semantic-port")?)?,
                graph_name: read_text(field(v, "graph-name")?)?,
            })
        })
        .collect()
}
