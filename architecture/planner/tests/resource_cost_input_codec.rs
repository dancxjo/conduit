//! Real domain-owned canonical decoding, without a Planner dependency on AI.
use conduit_ai::{install_model_work_catalog, ModelWorkOperation, ModelWorkRequest};
use conduit_core::*;
use conduit_planner::resource_cost::*;

fn model_request(bytes: &[u8]) -> Result<(), CostEvidenceRefusal> {
    ModelWorkRequest::decode(bytes)
        .map(|_| ())
        .map_err(|_| CostEvidenceRefusal::InvalidWorkInput)
}
fn gear() -> conduit_plot::CheckedGear {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_model_work_catalog(&mut startup, &mut profile).unwrap();
    conduit_plot::parse_with_startup("plot work {\n model: model/work\n}\n", &startup, &profile)
        .unwrap()
        .gears
        .remove(0)
}
fn value() -> ValuePayload {
    ValuePayload {
        value_kind: "model/work-request@1".into(),
        encoded: ModelWorkRequest {
            request_identity: [1; 32],
            session_identity: [2; 32],
            expected_generation: 3,
            operation: ModelWorkOperation::Export,
        }
        .encode()
        .unwrap(),
    }
}
#[test]
fn owner_canonical_decoder_admits_model_demand_and_fences_codec_revision() {
    let gear = gear();
    let kind = KindId::from("model/work-request@1");
    let revision = ArtifactId::from("conduit.ai/model-work-codec@1");
    let next_revision = ArtifactId::from("conduit.ai/model-work-codec@2");
    let codecs = [CostInputCodec::admit(&kind, &revision, 4096, model_request).unwrap()];
    let next_codecs = [CostInputCodec::admit(&kind, &next_revision, 4096, model_request).unwrap()];
    let value = value();
    let inputs = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    assert!(matches!(
        CostDemand::admit(&gear, &inputs),
        Err(CostEvidenceRefusal::InvalidWorkInput)
    ));
    let demand = CostDemand::admit_with_catalog(&gear, &inputs, &[], &codecs).unwrap();
    assert_eq!(demand.input_bytes(), value.encoded.len());
    assert_eq!(demand.validate_match(demand), Ok(()));
    let next = CostDemand::admit_with_catalog(&gear, &inputs, &[], &next_codecs).unwrap();
    assert_eq!(
        demand.validate_match(next),
        Err(CostEvidenceRefusal::InputCodecChanged)
    );
    assert_eq!(codecs[0].revision(), &revision);
}
#[test]
fn malformed_noncanonical_and_invalid_identity_model_requests_are_refused() {
    let gear = gear();
    let kind = KindId::from("model/work-request@1");
    let revision = ArtifactId::from("conduit.ai/model-work-codec@1");
    let codecs = [CostInputCodec::admit(&kind, &revision, 4096, model_request).unwrap()];
    let mut noncanonical = value();
    noncanonical.encoded.push(b' ');
    // Serialize the owner's actual type to retain canonical field order. This
    // isolates invalid identity from malformed or noncanonical spelling.
    let identity = ModelWorkRequest {
        request_identity: [0; 32],
        session_identity: [2; 32],
        expected_generation: 3,
        operation: ModelWorkOperation::Export,
    };
    let identity = ValuePayload {
        value_kind: kind.clone(),
        encoded: serde_json::to_vec(&identity).unwrap(),
    };
    assert_eq!(
        ModelWorkRequest::decode(&identity.encoded),
        Err(conduit_ai::ModelWorkCodecRefusal::Identity)
    );
    for value in [
        noncanonical,
        identity,
        ValuePayload {
            value_kind: kind.clone(),
            encoded: b"not JSON".to_vec(),
        },
    ] {
        let input = [CostInput {
            port: &gear.inputs[0].port_id,
            value: &value,
        }];
        assert!(matches!(
            CostDemand::admit_with_catalog(&gear, &input, &[], &codecs),
            Err(CostEvidenceRefusal::InvalidWorkInput)
        ));
    }
    let primitive = KindId::from(COUNT_INFO_ID);
    assert!(matches!(
        CostInputCodec::admit(&primitive, &revision, 4096, model_request),
        Err(CostEvidenceRefusal::InvalidWorkInput)
    ));
    let ambiguous = [
        CostInputCodec::admit(&kind, &revision, 4096, model_request).unwrap(),
        CostInputCodec::admit(&kind, &revision, 4096, model_request).unwrap(),
    ];
    assert!(matches!(
        CostDemand::admit_with_catalog(&gear, &[], &[], &ambiguous),
        Err(CostEvidenceRefusal::AmbiguousEvidence)
    ));
    let tiny = [CostInputCodec::admit(&kind, &revision, 1, model_request).unwrap()];
    let value = value();
    let input = [CostInput {
        port: &gear.inputs[0].port_id,
        value: &value,
    }];
    assert!(matches!(
        CostDemand::admit_with_catalog(&gear, &input, &[], &tiny),
        Err(CostEvidenceRefusal::InputLimit)
    ));
}
