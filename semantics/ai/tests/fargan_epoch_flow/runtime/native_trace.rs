//! Optional bounded development capture of the original committed63-epoch route.
//! Full Native and model custody remain in the admitted private session basis.
use super::super::custody::RetainedSignalModel;
use super::*;

pub(super) fn prepare_source(
    context: super::super::EpochProfiles,
    ids: &BTreeMap<String, String>,
    tail: &str,
) -> (
    super::super::EpochProfiles,
    String,
    &'static str,
    Option<std::path::PathBuf>,
) {
    match std::env::var("CONDUIT_FARGAN_NATIVE_TRACE") {
        Ok(directory) => {
            let (context, identity) = super::super::trace_cycle::prepare(context);
            let source = super::super::trace_cycle::native_utterance_source(ids, tail, &identity);
            (
                context,
                source,
                "speech/flow-fargan-native-utterance-traced",
                Some(directory.into()),
            )
        }
        Err(_) => (
            context,
            super::super::feature_cycle::native_utterance_source(ids, tail),
            "speech/flow-fargan-native-utterance",
            None,
        ),
    }
}
fn receipt(ty: &StructuredInfoType, bytes: &[u8; 32]) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), receipt(representation, bytes)).unwrap()
        }
        StructuredInfoTypeShape::Collection { element, length } => {
            assert_eq!(length, 32);
            StructuredInfoValue::collection(
                ty.clone(),
                bytes
                    .iter()
                    .map(|byte| StructuredInfoValue::leaf(element.clone(), vec![*byte]).unwrap())
                    .collect(),
            )
            .unwrap()
        }
        _ => panic!("exact checked Source receipt shape"),
    }
}
pub(super) fn sinks(
    context: &super::super::EpochProfiles,
    source: &str,
    model: &RetainedSignalModel,
    basis_identity: [u8; 32],
) -> trace_hooks::TraceSinks {
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(source),
        &context.startup,
    )
    .unwrap();
    let ty = |name: &str| {
        &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let anchor_type = ty("FarganModelFrameAnchor");
    let StructuredInfoTypeShape::Record { fields, .. } = anchor_type.shape() else {
        panic!("Source anchor")
    };
    let material = BTreeMap::from([
        (
            "artifact_identity",
            model.model.artifact().content_identity(),
        ),
        (
            "model_descriptor_identity",
            model.model.descriptor_identity(),
        ),
        ("session_basis_identity", basis_identity),
    ]);
    let anchor = StructuredInfoValue::record(
        anchor_type.clone(),
        fields
            .iter()
            .map(|field| {
                let value = match material.get(field.name()) {
                    Some(bytes) => receipt(field.value_type(), bytes),
                    None => {
                        assert_eq!(field.name(), "precision");
                        super::super::declarations::fixture_value(field.value_type())
                    }
                };
                StructuredFieldValue::new(field.name(), value).unwrap()
            })
            .collect(),
    )
    .unwrap();
    let types = [
        ("feature_trace", "FarganFeatureConditionTrace"),
        ("history_trace", "FarganConditionHistoryTrace"),
        ("pcm_trace", "FarganCommittedResultTrace"),
    ]
    .into_iter()
    .map(|(port, name)| (port.to_owned(), ty(name).clone()))
    .collect::<Vec<_>>();
    trace_hooks::TraceSinks::prepare(&types, &anchor, 1, 64).unwrap()
}
pub(super) fn write(
    directory: &std::path::Path,
    traces: &trace_hooks::TraceSinks,
    proposal: &StructuredInfoValue,
    warm: &[StructuredInfoValue],
) {
    use super::super::case_state::{field, floats, State};
    std::fs::create_dir_all(directory).unwrap();
    let features = traces.rows("feature_trace");
    let histories = traces.rows("history_trace");
    let pcm = traces.rows("pcm_trace");
    for (name, rows) in [
        ("feature-condition", &features),
        ("conditioning-history", &histories),
        ("committed-result", &pcm),
    ] {
        assert_eq!(rows.len(), 64);
        std::fs::write(
            directory.join(format!("{name}-canonical.json")),
            serde_json::to_vec(rows).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(
        directory.join("first-proposal.canonical"),
        proposal.canonical_bytes().unwrap(),
    )
    .unwrap();
    let warm: Vec<_> = warm
        .iter()
        .map(|value| value.canonical_bytes().unwrap())
        .collect();
    std::fs::write(
        directory.join("warm-canonical.json"),
        serde_json::to_vec(&warm).unwrap(),
    )
    .unwrap();
    let mut input = floats(field(proposal, "features"))
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    let mut conditions = Vec::new();
    let mut history = Vec::new();
    let mut states = Vec::new();
    for index in 0..64 {
        let feature = StructuredInfoValue::from_canonical_bytes(&features[index]).unwrap();
        let result = StructuredInfoValue::from_canonical_bytes(&pcm[index]).unwrap();
        let result = field(&result, "result");
        assert_eq!(field(&feature, "epoch"), field(result, "epoch"));
        let StructuredInfoValueShape::Leaf(epoch) = field(&feature, "epoch").shape() else {
            panic!("epoch")
        };
        input.extend_from_slice(epoch);
        let StructuredInfoValueShape::Leaf(period) = field(result, "next_period").shape() else {
            panic!("period")
        };
        input.extend_from_slice(
            &i32::from(u16::from_le_bytes(period.try_into().unwrap())).to_le_bytes(),
        );
        input.extend(
            floats(field(&feature, "features"))
                .into_iter()
                .flat_map(f32::to_le_bytes),
        );
        conditions.extend(
            floats(field(&feature, "condition"))
                .into_iter()
                .flat_map(f32::to_le_bytes),
        );
        let row = StructuredInfoValue::from_canonical_bytes(&histories[index]).unwrap();
        assert_eq!(field(&row, "epoch"), field(result, "epoch"));
        history.extend(
            floats(field(&row, "next_history"))
                .into_iter()
                .flat_map(f32::to_le_bytes),
        );
        states.extend(
            State::from_result(result)
                .flattened()
                .into_iter()
                .flat_map(f32::to_le_bytes),
        );
    }
    for (name, bytes) in [
        ("retained-model-input.bin", input),
        ("source-condition.f32le", conditions),
        ("source-history.f32le", history),
        ("source-state.f32le", states),
        ("source-pcm.i16le", pcm_bytes(&pcm)),
    ] {
        std::fs::write(directory.join(name), bytes).unwrap();
    }
}

fn pcm_bytes(rows: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(rows.len() * 160 * 2);
    for row in rows {
        let value = StructuredInfoValue::from_canonical_bytes(row).unwrap();
        let result = super::super::case_state::field(&value, "result");
        let pcm = super::super::case_state::field(result, "pcm_i16");
        let StructuredInfoValueShape::Collection(samples) = pcm.shape() else {
            panic!("retained exact PCM160 collection")
        };
        assert_eq!(samples.len(), 160);
        for sample in samples {
            let StructuredInfoValueShape::Leaf(raw) = sample.shape() else {
                panic!("retained exact I16 leaf")
            };
            assert_eq!(raw.len(), 2);
            bytes.extend_from_slice(raw);
        }
    }
    bytes
}
#[test]
#[ignore = "requires an already drained private Source trace capture"]
fn export_retained_committed_pcm_without_model_reexecution() {
    let directory = std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_NATIVE_TRACE").unwrap());
    let rows: Vec<Vec<u8>> = serde_json::from_slice(
        &std::fs::read(directory.join("committed-result-canonical.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(rows.len(), 64);
    std::fs::write(directory.join("source-pcm.i16le"), pcm_bytes(&rows)).unwrap();
}
