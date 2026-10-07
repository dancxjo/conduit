//! Private conditioning interface correspondence. Logical tensor constraints
//! remain distinct from exact native canonical transport and Source semantics.
use super::*;
use conduit_ai::{
    fixed_numeric_codec::FixedF32VectorCodec, fixed_numeric_u16_profile::PreparedU16Profile,
    AdmittedModelResource, ModelDimensionConstraint, ModelValueConstraint,
};
use conduit_data::TensorElement;
use std::{collections::BTreeMap, sync::Arc};

pub(super) struct ConditioningInterface {
    pub model: Arc<AdmittedModelResource>,
    pub source: String,
    pub front: CheckedFront,
    pub native_ports: BTreeMap<String, StructuredInfoType>,
    pub layout: Vec<u8>,
    pub period: PreparedU16Profile,
}
impl ConditioningInterface {
    pub fn prepare(model: Arc<AdmittedModelResource>, layout: Vec<u8>) -> Result<Self, String> {
        if layout.as_slice()
            != include_bytes!("../../../../proof/fargan/conditioning-layout-f32.json")
            || model.artifact().architecture_profile != "speech/fargan-xiph-503d81b1@1"
            || model.artifact().format_profile != "model/fargan-array-bundle-f32-le@1"
            || model.artifact().state_schema_version != 1
            || model.signature().identity() != "speech/fargan-source-conditioning-f32@1"
        {
            return Err("reviewed conditioning model interface/layout required".into());
        }
        let source = String::from("type FarganPeriod = U16 in 32..=255\n")
            + include_str!("../../../speech/fargan_conditioning_flow.conduit");
        let (startup, _) = catalogs(true);
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
            .map_err(|e| format!("{e:?}"))?;
        let front = checked
            .plots
            .iter()
            .find(|p| p.name == "speech/flow-fargan-conditioning-core")
            .ok_or("missing conditioning Source entry")?
            .runtime_front
            .clone();
        let period = PreparedU16Profile::check_definition("type FarganPeriod = U16 in 32..=255\n")?;
        let native_ports = BTreeMap::from([
            ("features".into(), fixed_numeric_type("NumericF32Vector20")?),
            ("period".into(), period.value_type().clone()),
            ("history".into(), fixed_numeric_type("NumericHistory2x64")?),
            (
                "condition".into(),
                fixed_numeric_type("NumericF32Vector320")?,
            ),
            (
                "next_history".into(),
                fixed_numeric_type("NumericHistory2x64")?,
            ),
        ]);
        FixedF32VectorCodec::<20>::prepare(&native_ports["features"])
            .map_err(|e| format!("{e:?}"))?;
        FixedF32VectorCodec::<320>::prepare(&native_ports["condition"])
            .map_err(|e| format!("{e:?}"))?;
        let history =
            FixedF32VectorCodec::<128>::prepare_history().map_err(|e| format!("{e:?}"))?;
        let _ = history;
        model.signature().validate().map_err(|e| format!("{e:?}"))?;
        let exact = conduit_ai::ModelSignature::from_parts(
            "speech/fargan-source-conditioning-f32@1".into(),
            1,
            vec![conduit_ai::ModelOperation::Infer],
            vec![
                super::custody::port("features", 20, TensorElement::F32, 4),
                super::custody::port("period", 1, TensorElement::U16, 2),
                super::custody::port("history", 128, TensorElement::F32, 4),
            ],
            vec![
                super::custody::port("condition", 320, TensorElement::F32, 4),
                super::custody::port("next_history", 128, TensorElement::F32, 4),
            ],
        )
        .map_err(|e| format!("{e:?}"))?;
        if model.signature() != &exact {
            return Err("exact conditioning operation/version/axis signature required".into());
        }

        for (ports, source_ports, expected) in [
            (
                model.signature().inputs().get().as_slice(),
                front.inputs(),
                vec![
                    ("features", 20, TensorElement::F32),
                    ("period", 1, TensorElement::U16),
                    ("history", 128, TensorElement::F32),
                ],
            ),
            (
                model.signature().outputs().get().as_slice(),
                front.outputs(),
                vec![
                    ("condition", 320, TensorElement::F32),
                    ("next_history", 128, TensorElement::F32),
                ],
            ),
        ] {
            if ports.len() != expected.len() {
                return Err("logical conditioning port count mismatch".into());
            }
            for (name, width, element) in expected {
                let port = ports
                    .iter()
                    .find(|p| p.identity().get() == name)
                    .ok_or("missing logical conditioning port")?;
                let ModelValueConstraint::Tensor(value) = port.value() else {
                    return Err("conditioning logical tensor required".into());
                };
                let constraint = value.constraint();
                if port.semantic_kind().get() != "data/tensor@1"
                    || port.presence() != &conduit_ai::ModelPortPresence::Required
                    || constraint.elements().get().as_slice() != [element]
                    || constraint.axes().get().len() != 1
                    || *constraint.maximum_bytes() != width * element.byte_width()
                {
                    return Err("logical conditioning tensor profile mismatch".into());
                }
                let ModelDimensionConstraint::Fixed(dimension) =
                    constraint.axes().get()[0].dimension()
                else {
                    return Err("fixed conditioning tensor extent required".into());
                };
                if *dimension.value() != width {
                    return Err("logical/native conditioning shape mismatch".into());
                }
                let source_port = source_ports
                    .iter()
                    .find(|p| p.port_id.as_str() == name)
                    .ok_or("missing authored native conditioning port")?;
                if source_port.value_kind
                    != *native_ports[name]
                        .profile()
                        .map_err(|e| format!("{e:?}"))?
                        .value_kind()
                    || source_port.temporal != (PortTemporal::Flow { closes: true })
                {
                    return Err("authored native conditioning port mismatch".into());
                }
            }
        }
        let definition: serde_json::Value =
            serde_json::from_slice(&layout).map_err(|e| e.to_string())?;
        if definition["profile"] != "speech/fargan-conditioning-layout-f32@1"
            || definition["resources"]
                .as_array()
                .ok_or("conditioning layout slots")?
                .len()
                != 7
        {
            return Err("reviewed conditioning layout required".into());
        }
        use sha2::{Digest, Sha256};
        if definition["raw_blob_sha256"] != format!("{:x}", Sha256::digest(model.bytes()))
            || definition["precision"].as_str() != Some(model.artifact().precision_profile.as_str())
            || definition["upstream_revision"] != "503d81b138d76621aae4b12786e90de48aa8db3a"
        {
            return Err("model/layout/precision provenance mismatch".into());
        }
        Ok(Self {
            model,
            source,
            front,
            native_ports,
            layout,
            period,
        })
    }
}

#[test]
#[ignore = "private exact model bundle; checked logical/native conditioner correspondence"]
fn pinned_conditioner_interface_matches_exact_source_front_native_codecs_and_signature() {
    let root = std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
    let retained = super::custody::RetainedSignalModel::load(&root);
    let (layout, resources) = retained.conditioning_resources();
    let admitted =
        ConditioningInterface::prepare(retained.conditioning_descriptor(), layout.clone()).unwrap();
    assert_eq!(admitted.front.inputs().len(), 10);
    assert_eq!(admitted.front.outputs().len(), 2);
    assert_eq!(admitted.native_ports.len(), 5);
    assert_eq!(resources.len(), 7);
    assert!(admitted
        .source
        .contains("speech/flow-fargan-conditioning-core"));
    assert_eq!(admitted.layout, layout);
    assert_eq!(
        admitted.period.value_type(),
        &admitted.native_ports["period"]
    );
    assert!(Arc::ptr_eq(
        &admitted.model.shared_storage(),
        &retained.model.shared_storage()
    ));
    assert!(
        ConditioningInterface::prepare(retained.compound_descriptor(), layout.clone()).is_err()
    );
    let wrong = conduit_ai::ModelSignature::from_parts(
        "speech/fargan-source-conditioning-f32@1".into(),
        1,
        vec![conduit_ai::ModelOperation::Infer],
        vec![
            super::custody::port("features", 19, TensorElement::F32, 4),
            super::custody::port("period", 1, TensorElement::U16, 2),
            super::custody::port("history", 128, TensorElement::F32, 4),
        ],
        vec![
            super::custody::port("condition", 320, TensorElement::F32, 4),
            super::custody::port("next_history", 128, TensorElement::F32, 4),
        ],
    )
    .unwrap();
    let mut artifact = admitted.model.artifact().clone();
    artifact.signature_identity = wrong.semantic_digest().unwrap();
    let wrong = Arc::new(
        AdmittedModelResource::adopt(
            artifact,
            wrong,
            retained.model.shared_storage(),
            &retained.model_binding,
        )
        .unwrap(),
    );
    assert!(ConditioningInterface::prepare(wrong, layout.clone()).is_err());
    let wrong_version = conduit_ai::ModelSignature::from_parts(
        "speech/fargan-source-conditioning-f32@1".into(),
        2,
        vec![conduit_ai::ModelOperation::Infer],
        admitted
            .model
            .signature()
            .inputs()
            .get()
            .as_slice()
            .to_vec(),
        admitted
            .model
            .signature()
            .outputs()
            .get()
            .as_slice()
            .to_vec(),
    )
    .unwrap();
    let mut artifact = admitted.model.artifact().clone();
    artifact.signature_identity = wrong_version.semantic_digest().unwrap();
    let wrong_version = Arc::new(
        AdmittedModelResource::adopt(
            artifact,
            wrong_version,
            retained.model.shared_storage(),
            &retained.model_binding,
        )
        .unwrap(),
    );
    assert!(ConditioningInterface::prepare(wrong_version, layout.clone()).is_err());
    let mut foreign = layout;
    foreign.push(b' ');
    assert!(ConditioningInterface::prepare(retained.conditioning_descriptor(), foreign).is_err());
}
