//! Provider-neutral model inference Fore. Artifact selection is preparation truth.
use crate::{ModelOperation, ModelSignature};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_data::NamedTensorBundle;
use conduit_plot::{
    rust_binding::NativeRustBinding, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};

pub const MODEL_INFER_KIND: &str = "model/infer";
pub const MODEL_INFER_REVISION: &str = "conduit.ai/named-model-infer@1";

pub fn model_signature_profile() -> KindId {
    ModelSignature::semantic_type()
        .expect("checked model signature Type")
        .profile()
        .expect("finite signature profile")
        .value_kind()
        .clone()
}

pub fn model_tensor_bundle_profile() -> KindId {
    NamedTensorBundle::semantic_type()
        .expect("checked named tensor Type")
        .profile()
        .expect("finite tensor bundle profile")
        .value_kind()
        .clone()
}

pub fn signature_configuration(signature: &ModelSignature) -> Result<ConfigurationValue, String> {
    signature
        .validate()
        .map_err(|e| format!("invalid model signature: {e:?}"))?;
    if signature.operations.get().as_slice() != [ModelOperation::Infer] {
        return Err("inference Fore requires an inference-only semantic signature".into());
    }
    StructuredConfigurationValue::new(
        model_signature_profile(),
        signature
            .clone()
            .encode()
            .map_err(|e| format!("signature encoding: {e:?}"))?,
    )
    .map(ConfigurationValue::Structured)
    .ok_or_else(|| "invalid signature configuration".into())
}

pub fn configured_model_signature(
    configuration: &[ConfigurationEntry],
) -> Result<ModelSignature, String> {
    let Some(ConfigurationEntry {
        value: ConfigurationValue::Structured(value),
        ..
    }) = configuration.iter().find(|e| e.key == "signature")
    else {
        return Err("missing typed model signature".into());
    };
    if value.profile() != &model_signature_profile() {
        return Err("wrong model signature profile".into());
    }
    let signature = ModelSignature::decode(value.canonical_value())
        .map_err(|e| format!("signature decoding: {e:?}"))?;
    signature_configuration(&signature)?;
    Ok(signature)
}

pub fn model_infer_contract() -> Result<Kind, String> {
    let signature_profile = model_signature_profile();
    let bundle_profile = model_tensor_bundle_profile();
    let parameter = FrontStartupParameter {
        name: "signature".into(),
        value_type: signature_profile.clone(),
        has_default: false,
    };
    let port = |name: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: bundle_profile.clone(),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    // Configuration fields need a typed exemplar. The required startup parameter
    // has no default, so authored Source must supply its actual semantic request.
    let tensor = crate::ModelTensorConstraint::from_parts(
        vec![conduit_data::TensorElement::F32],
        vec![crate::ModelAxisConstraint::new(
            crate::ModelDimensionConstraint::fixed(1)
                .map_err(|e| format!("signature dimension: {e:?}"))?,
            conduit_data::TensorAxisRole::Feature,
        )
        .map_err(|e| format!("signature axis: {e:?}"))?],
        4,
    )
    .map_err(|e| format!("signature tensor: {e:?}"))?;
    let input = crate::ModelPortConstraint::from_parts(
        "input".into(),
        "data/tensor@1".into(),
        crate::ModelPortPresence::Required,
        crate::ModelValueConstraint::tensor(tensor)
            .map_err(|e| format!("signature value: {e:?}"))?,
    )
    .map_err(|e| format!("signature port: {e:?}"))?;
    let exemplar = ModelSignature::from_parts(
        "model/required-signature@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![input],
        vec![],
    )
    .map_err(|e| format!("signature exemplar: {e:?}"))?;
    let contracts = vec![
        (
            FrontValueLocation::Startup("signature".into()),
            signature_profile.clone(),
        ),
        (
            FrontValueLocation::Input(port_id("inputs")),
            bundle_profile.clone(),
        ),
        (
            FrontValueLocation::Output(port_id("outputs")),
            bundle_profile.clone(),
        ),
    ]
    .into_iter()
    .map(|(location, profile)| {
        Ok(FrontValueContract {
            location,
            contract: CheckedValueContract::new(
                profile,
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                vec![],
            )
            .map_err(|e| format!("finite model Fore: {e:?}"))?,
        })
    })
    .collect::<Result<_, String>>()?;
    let kind = Kind {
        startup_parameters: vec![parameter],
        shorthand: None,
        kind_id: kind_id(MODEL_INFER_KIND),
        kind_contract_revision: MODEL_INFER_REVISION.into(),
        inputs: vec![port("inputs", PortDirection::Input)],
        outputs: vec![port("outputs", PortDirection::Output)],
        configuration: vec![KindConfigurationField {
            key: "signature".into(),
            default_value: signature_configuration(&exemplar)?,
            rule: KindConfigurationRule::Structured {
                profile: signature_profile.clone(),
            },
        }],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(contracts),
            KindSemanticLaw::RealizationRequirement {
                property_profile: signature_profile,
                configuration_key: "signature".into(),
            },
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    };
    kind.validate()
        .map_err(|e| format!("invalid inference contract: {e:?}"))?;
    Ok(kind)
}

pub fn install_model_infer_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.ensure_structured_type(
        "ModelSignature",
        ModelSignature::semantic_type().map_err(|e| format!("signature Type: {e:?}"))?,
    )?;
    startup.ensure_structured_type(
        "NamedTensorBundle",
        NamedTensorBundle::semantic_type().map_err(|e| format!("tensor bundle Type: {e:?}"))?,
    )?;
    let kind = model_infer_contract()?;
    startup.insert(KindSignature {
        kind: MODEL_INFER_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "signature".into(),
            value_type: "ModelSignature".into(),
            default: None,
        }],
    })?;
    startup.insert_fore(MODEL_INFER_KIND, kind.checked_front())?;
    profile
        .insert_kind(kind)
        .map_err(|e| format!("inference catalog: {e:?}"))
}
