//! Actual combined authored protocol construction, not admission or hardware proof.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
enum Selection {
    State(&'static str, &'static str),
    Merge(&'static str, &'static str),
    Zip {
        alias: &'static str,
        pair: &'static str,
        left: &'static str,
        left_bytes: u32,
        right: &'static str,
        right_bytes: u32,
        feedback: bool,
    },
}
fn selections() -> [Selection; 9] {
    let result_bytes = conduit_i2c_transaction::contract::I2C_RESULT_VALUE_BYTES as u32;
    [
        Selection::State("BmeCell", "BmeProtocolFrame"),
        Selection::State("HtCell", "Ht16k33ProtocolFrame"),
        Selection::Merge("BmeContexts", "BmeClockContext"),
        Selection::Merge("HtErrors", "Ht16k33Failure"),
        Selection::Merge("Requests", "I2cCorrelatedRequest"),
        Selection::Zip {
            alias: "BmeFeedback",
            pair: "BmeFeedbackPair",
            left: "BmeProtocolState",
            left_bytes: 4096,
            right: "BmeProtocolEvent",
            right_bytes: 4096,
            feedback: true,
        },
        Selection::Zip {
            alias: "BmeClockPairing",
            pair: "BmeClockPair",
            left: "BmeClockContext",
            left_bytes: 4096,
            right: "ClockResult",
            right_bytes: 512,
            feedback: false,
        },
        Selection::Zip {
            alias: "HtFeedback",
            pair: "HtFeedbackPair",
            left: "Ht16k33State",
            left_bytes: 4096,
            right: "I2cResult",
            right_bytes: result_bytes,
            feedback: true,
        },
        Selection::Zip {
            alias: "BusPairing",
            pair: "BusPair",
            left: "I2cCorrelation",
            left_bytes: 512,
            right: "I2cResult",
            right_bytes: result_bytes,
            feedback: false,
        },
    ]
}
fn request(selection: Selection) -> Specialization<'static> {
    let reference = |type_name, maximum_bytes| ValueReference {
        type_name,
        maximum_bytes,
    };
    match selection {
        Selection::State(_, name) => Specialization::SeededUntilInstance(reference(name, 4096)),
        Selection::Merge(_, name) => Specialization::MergeInstance(reference(name, 4096)),
        Selection::Zip {
            left,
            left_bytes,
            right,
            right_bytes,
            feedback,
            ..
        } => Specialization::ZipInstance {
            left: reference(left, left_bytes),
            right: reference(right, right_bytes),
            feedback,
        },
    }
}
fn library() -> String {
    let units = [
        include_str!("../../../../plots/device-protocols/bme280-lifecycle.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-clock-events.conduit"),
        include_str!("../../../../plots/device-protocols/main.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-capture-observation.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-protocol-frame.conduit"),
        include_str!("../../../../plots/device-protocols/bme280-exchange.conduit"),
        include_str!("../../../../plots/device-protocols/ht16k33.conduit"),
        include_str!("../../../../plots/device-protocols/ht16k33-exchanges.conduit"),
        include_str!("../../../../plots/device-protocols/i2c-correlated.conduit"),
        include_str!("../../../../plots/device-protocols/environmental-exchange.conduit"),
    ];
    // Ordinary declaration packaging: deduplicate imports, retain every library
    // body line. No legacy adapters or protocol text substitutions are used.
    let imports = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| l.starts_with("with "))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let bodies = units
        .iter()
        .flat_map(|s| s.lines())
        .filter(|l| !l.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{imports}\n{bodies}")
}
fn aliases(source: &str) -> (String, Vec<c::KindId>) {
    let mut startup = p::StartupCatalog::new();
    let mut profiles = p::ProfileCatalog::new();
    crate::protocol_catalog::install(&mut startup, &mut profiles).unwrap();
    let types = p::prepare_source_types(&p::parse_syntax_document(source), &startup).unwrap();
    let value = |name, maximum_bytes| {
        let schema = types.named_type(name).unwrap().clone();
        let contract = c::CheckedValueContract::new(
            schema.profile().unwrap().value_kind().clone(),
            maximum_bytes,
            vec![],
        )
        .unwrap();
        Value { schema, contract }
    };
    let mut imports = String::new();
    let mut ids = Vec::new();
    for selection in selections() {
        let (family, alias, pair_alias, values) = match selection {
            Selection::State(alias, name) => (
                "state/seeded/flow/until",
                alias,
                None,
                vec![value(name, 4096)],
            ),
            Selection::Merge(alias, name) => {
                ("flow/merge/finite", alias, None, vec![value(name, 4096)])
            }
            Selection::Zip {
                alias,
                pair,
                left,
                left_bytes,
                right,
                right_bytes,
                feedback,
            } => (
                if feedback {
                    "flow/zip/feedback"
                } else {
                    "flow/zip/finite"
                },
                alias,
                Some(pair),
                vec![value(left, left_bytes), value(right, right_bytes)],
            ),
        };
        let id = c::typed_kind_instance_id(family, values.iter().map(|v| (&v.schema, &v.contract)))
            .unwrap();
        imports.push_str(&format!("with {} as {alias}\n", id.as_str()));
        if let Some(pair) = pair_alias {
            imports.push_str(&format!("with {}/paired as {pair}\n", id.as_str()));
        }
        ids.push(id);
    }
    (imports, ids)
}

const CALLER: &str = include_str!("../tests/fixtures/environmental-exchange-selected.conduit");

#[test]
fn actual_environment_protocols_share_one_correlated_controller_and_clock() {
    let library = library();
    let (imports, ids) = aliases(&library);
    let source = format!("{imports}\n{library}\n{CALLER}");
    let requests = selections().map(request);
    let workset = crate::workset::compile_specialized(
        source.as_bytes(),
        &format!("{:x}", Sha256::digest(source.as_bytes())),
        &["shared-environment".into()],
        None,
        &requests,
    )
    .unwrap();
    let entry = &workset.entries[0];
    let gears = &entry.plot.expanded.gears;
    assert_eq!(
        gears
            .iter()
            .filter(|g| g.kind_id.as_str() == conduit_i2c_transaction::contract::I2C_KIND)
            .count(),
        1
    );
    assert_eq!(gears.iter().filter(|g| g.kind_id.as_str() == conduit_monotonic_wait::clock_at::contract::CLOCK_KIND).count(), 1);
    let distinct = ids.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct.len(), 9);
    for id in &ids {
        assert!(
            gears.iter().any(|g| g.kind_id == *id),
            "missing {}",
            id.as_str()
        );
    }
    // Two independent merge owners arbitrate three request lanes; neither is
    // another native bus nor an alias of initialized mutable Back storage.
    assert_eq!(gears.iter().filter(|g| g.kind_id == ids[4]).count(), 2);
    let material = crate::software::compile_with_types(
        &entry.plot.expanded,
        Some(&workset.checked_source),
        32 * 1024,
    )
    .unwrap();
    eprintln!(
        "combined protocol geometry: gears={}, cords={}, software_nodes={}, software_offer_bytes={}, generated_rust_bytes={}; not a firmware memory measurement",
        gears.len(), entry.cords, material.nodes.len(),
        material.nodes.iter().filter_map(|node|node.offer_image.as_ref()).map(Vec::len).sum::<usize>(),
        entry.rust.len(),
    );
    for node in &material.nodes {
        if let Some(offer) = &node.offer {
            assert_eq!(offer.kind_id, node.kind);
            assert!(node.offer_image.is_some());
        }
    }
    assert!(entry.rust.contains("SeededUntilStatic"));
    assert!(entry.rust.contains("MergeStatic"));
    assert!(entry.rust.contains("ZipStatic"));
    routing_transcript(&workset.checked_source);
    assert!(entry
        .plot
        .output_bindings
        .iter()
        .any(|binding| binding.front_port_id.as_str() == "unrouted"));
}

fn routing_transcript(doc: &p::CheckedSyntaxDocument) {
    let expanded = p::expand_canonical_plot_for_authoring(
        doc,
        "environmental-route-result",
        &p::ProfileCatalog::new(),
    )
    .unwrap();
    let c::ConfigurationValue::Text(program) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("routing expression")
    };
    let mut evaluator = p::PreparedPortableExpressionEvaluator::new(
        &p::PortableExpressionProgram::from_canonical_hex(program).unwrap(),
    )
    .unwrap();
    let schema = |name: &str| {
        doc.native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
            .clone()
    };
    let correlation_type = schema("I2cCorrelation");
    let result_type = schema("I2cCorrelatedResult");
    let record = |ty: &c::StructuredInfoType, fields: Vec<(&str, c::StructuredInfoValue)>| {
        c::StructuredInfoValue::record(
            ty.clone(),
            fields
                .into_iter()
                .map(|(name, value)| c::StructuredFieldValue::new(name, value).unwrap())
                .collect(),
        )
        .unwrap()
    };
    let scalar = |value: u64| {
        c::StructuredInfoValue::leaf(
            c::StructuredInfoType::leaf(c::kind_id("value/u64")).unwrap(),
            value.to_le_bytes().to_vec(),
        )
        .unwrap()
    };
    let contract = conduit_i2c_transaction::contract::I2cContract::prepare().unwrap();
    let mut codec =
        conduit_i2c_transaction::result::PreparedI2cResultEncoder::new(&contract).unwrap();
    let bus_result = codec
        .disposition(conduit_i2c_transaction::I2cDisposition::NotAcknowledged)
        .unwrap()
        .to_vec();
    for (client, sequence, expected_tag) in [
        (1, 0, "sensor"),
        (2, 0, "initialization"),
        (3, 0, "write"),
        (4, 0, "unrouted"),
        (1, 1, "unrouted"),
    ] {
        let correlation = record(
            &correlation_type,
            vec![("client", scalar(client)), ("sequence", scalar(sequence))],
        );
        let input = record(
            &result_type,
            vec![
                ("correlation", correlation),
                (
                    "result",
                    c::StructuredInfoValue::from_canonical_bytes(&bus_result).unwrap(),
                ),
            ],
        )
        .canonical_bytes()
        .unwrap();
        let output =
            c::StructuredInfoValue::from_canonical_bytes(evaluator.evaluate(&input).unwrap())
                .unwrap();
        let c::StructuredInfoValueShape::Variant { tag, payload } = output.shape() else {
            panic!("routing result")
        };
        assert_eq!(tag, expected_tag);
        assert_eq!(
            payload.canonical_bytes().unwrap(),
            if expected_tag == "unrouted" {
                &input
            } else {
                &bus_result
            }
            .as_slice()
        );
    }
}
