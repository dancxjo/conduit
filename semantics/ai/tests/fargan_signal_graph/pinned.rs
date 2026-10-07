use super::{plan::*, shared::*, state::*};
use conduit_ai::*;
use conduit_core::*;
use conduit_data::*;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};
const BLOB_SHA: &str = "d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47";
fn typed_port(
    name: &str,
    width: u64,
    element: TensorElement,
    bytes_per_element: u64,
) -> ModelPortConstraint {
    let tensor = ModelTensorConstraint::from_parts(
        vec![element],
        vec![ModelAxisConstraint {
            role: TensorAxisRole::Feature,
            dimension: ModelDimensionConstraint::fixed(width).unwrap(),
        }],
        width * bytes_per_element,
    )
    .unwrap();
    ModelPortConstraint::from_parts(
        name.into(),
        "data/tensor@1".into(),
        ModelPortPresence::Required,
        ModelValueConstraint::tensor(tensor).unwrap(),
    )
    .unwrap()
}
fn port(name: &str, width: u64) -> ModelPortConstraint {
    typed_port(name, width, TensorElement::F32, 4)
}
pub fn load(root: &Path, schema: &SourceSchema) -> Resources {
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("resources/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["upstream_sha"],
        "503d81b138d76621aae4b12786e90de48aa8db3a"
    );
    assert_eq!(
        manifest["source_sha256"],
        "68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c"
    );
    let blob = std::fs::read(root.join("resources/f32.bin")).unwrap();
    assert_eq!(blob.len(), 3_272_868);
    assert_eq!(format!("{:x}", Sha256::digest(&blob)), BLOB_SHA);
    // Logical oracle tensor signature includes every dynamic input. Source
    // nominal records retain their own exact checked transport contract; this
    // fixture does not replace it with a host tensor facade.
    let signature = ModelSignature::from_parts(
        "speech/fargan-subframe-float@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![
            port("condition80", 80),
            typed_port("period", 1, TensorElement::U16, 2),
            port("active_state837", 837),
        ],
        vec![port("pcm40", 40), port("next_active_state837", 837)],
    )
    .unwrap();
    let artifact = ModelArtifact {
        architecture_profile: "speech/fargan-xiph-503d81b1@1".into(),
        format_profile: "model/fargan-array-bundle-f32-le@1".into(),
        precision_profile: "number/ieee754-f32-libm-reference@1".into(),
        state_schema_version: 1,
        signature_identity: signature.semantic_digest().unwrap(),
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(model_content_digest(&blob)),
            content_profile: kind_id("model/fargan-array-bundle-f32-le@1"),
            access_class: ResourceClassId::from("private-development-model/read@1"),
            extent: ResourceExtent {
                bytes: blob.len() as u64,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([1; 32]),
                expires_at: None,
            },
        },
    };
    artifact.validate(&signature).unwrap();
    // Bind immutable resource DATA by generated-array identity. This fixture
    // table is not network execution order; the checked speech source owns it.
    let mut resources = BTreeMap::new();
    for (name, array) in [
        ("gain_weights", "sig_net_cond_gain_dense_weights_float"),
        ("gain_bias", "sig_net_cond_gain_dense_bias"),
        ("conv_weights", "sig_net_fwc0_conv_weights_float"),
        ("conv_bias", "sig_net_fwc0_conv_bias"),
        ("conv_glu_weights", "sig_net_fwc0_glu_gate_weights_float"),
        ("conv_glu_bias", "sig_net_fwc0_glu_gate_bias"),
        ("pitch_gate_weights", "sig_net_gain_dense_out_weights_float"),
        ("pitch_gate_bias", "sig_net_gain_dense_out_bias"),
        ("gru1_input_weights", "sig_net_gru1_input_weights_float"),
        (
            "gru1_recurrent_weights",
            "sig_net_gru1_recurrent_weights_float",
        ),
        ("gru2_input_weights", "sig_net_gru2_input_weights_float"),
        (
            "gru2_recurrent_weights",
            "sig_net_gru2_recurrent_weights_float",
        ),
        ("gru3_input_weights", "sig_net_gru3_input_weights_float"),
        (
            "gru3_recurrent_weights",
            "sig_net_gru3_recurrent_weights_float",
        ),
        ("gru1_glu_weights", "sig_net_gru1_glu_gate_weights_float"),
        ("gru1_glu_bias", "sig_net_gru1_glu_gate_bias"),
        ("gru2_glu_weights", "sig_net_gru2_glu_gate_weights_float"),
        ("gru2_glu_bias", "sig_net_gru2_glu_gate_bias"),
        ("gru3_glu_weights", "sig_net_gru3_glu_gate_weights_float"),
        ("gru3_glu_bias", "sig_net_gru3_glu_gate_bias"),
        ("skip_weights", "sig_net_skip_dense_weights_float"),
        ("skip_bias", "sig_net_skip_dense_bias"),
        ("skip_glu_weights", "sig_net_skip_glu_gate_weights_float"),
        ("skip_glu_bias", "sig_net_skip_glu_gate_bias"),
        ("output_weights", "sig_net_sig_dense_out_weights_float"),
        ("output_bias", "sig_net_sig_dense_out_bias"),
    ] {
        let a = manifest["arrays"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == array)
            .unwrap();
        assert_eq!(a["type"], "f32le");
        let offset = a["profiles"]["f32"]["offset"].as_u64().unwrap() as usize;
        let length = a["profiles"]["f32"]["bytes"].as_u64().unwrap() as usize;
        let bytes = &blob[offset..offset + length];
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            a["sha256"].as_str().unwrap()
        );
        let shape = &schema.resource_shapes[name];
        assert_eq!(length, shape.iter().product::<u64>() as usize * 4);
        let values = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        resources.insert(
            name.into(),
            Resource::new(schema.ty(name).clone(), shape, values),
        );
    }
    assert_eq!(resources.len(), schema.resource_shapes.len());
    eprintln!("private ModelArtifact content={:02x?}, exact full-f32 bundle SHA256={BLOB_SHA}, {} selected resource descriptors; source record shape and period remain separately checked",artifact.content_identity(),resources.len());
    resources
}
pub struct OracleCase {
    pub period: u16,
    pub condition: [f32; 80],
    pub prior: State,
    pub pcm: [f32; 40],
    pub next: State,
}
pub fn oracle(root: &Path) -> Vec<OracleCase> {
    let bytes = std::fs::read(root.join("subframe-float-oracle.bin")).unwrap();
    assert_eq!(bytes.len(), 96 * 7180);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "bf52f0431af52435e775818d7e7fe65ccf8b6ff7de38e57eab4b0db9935feccf"
    );
    bytes
        .as_chunks::<7180>()
        .0
        .iter()
        .map(|record| {
            let values: Vec<_> = record[4..]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            assert!(values.iter().all(|v| v.is_finite()));
            OracleCase {
                period: u16::try_from(i32::from_le_bytes(record[..4].try_into().unwrap())).unwrap(),
                condition: values[..80].try_into().unwrap(),
                prior: State::from_oracle(&values[80..917]),
                pcm: values[917..957].try_into().unwrap(),
                next: State::from_oracle(&values[957..]),
            }
        })
        .collect()
}
