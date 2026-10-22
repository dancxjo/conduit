//! Production typed entrances retain canonical custody across private framing.
use super::*;
use crate::data::{InitManifest, Initial};

fn command_roundtrip(workspace: &mut Workspace, command: &Command) {
    let custody = encode(command).unwrap();
    let wire = encode_command(command).unwrap();
    let returned = workspace.decode_command(&wire).unwrap();
    assert_eq!(encode(&returned).unwrap(), custody);
    assert!(workspace.decode_command(&custody).is_err());
}

fn reply_roundtrip(workspace: &mut Workspace, reply: &Reply) {
    let custody = encode(reply).unwrap();
    let mut output = buffer(MAX_BLOB).unwrap();
    workspace.encode_reply_into(reply, &mut output).unwrap();
    let returned = decode_reply(&output).unwrap();
    assert_eq!(encode(&returned).unwrap(), custody);
    assert!(decode_reply(&custody).is_err());
}

#[test]
fn real_staged_targets_and_graph_replies_keep_exact_canonical_custody() {
    // Only fixture ingestion recognizes the historical aggregate Init shape.
    #[derive(serde::Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; crate::SLOTS],
        },
    }
    let Fixture::Init { applications } =
        decode(include_bytes!("../../../tests/fixtures/startup.catz")).unwrap();
    let mut workspace = Workspace::new().unwrap();
    command_roundtrip(
        &mut workspace,
        &Command::PrepareInit {
            manifest: InitManifest::from_initial(&applications),
        },
    );
    let Some(Initial::Patchbay { targets, .. }) = &applications[0] else {
        panic!("retained Patchbay fixture");
    };
    for (target, value) in targets.iter().enumerate() {
        if let Some(value) = value {
            command_roundtrip(
                &mut workspace,
                &Command::PrepareTarget {
                    slot: 0,
                    target: target as u8,
                    value: value.clone(),
                },
            );
            reply_roundtrip(
                &mut workspace,
                &Reply::Graph {
                    graph: Some(value.graph.clone()),
                },
            );
        }
    }
    command_roundtrip(&mut workspace, &Command::FinishInit);
    command_roundtrip(
        &mut workspace,
        &Command::Apply {
            slot: 0,
            input: (0..=255).collect(),
        },
    );
    reply_roundtrip(&mut workspace, &Reply::Prepared);
    reply_roundtrip(&mut workspace, &Reply::Initialized);
    reply_roundtrip(&mut workspace, &Reply::Graph { graph: None });
}

#[test]
fn staged_borrowed_graph_reply_preserves_real_graph_custody_with_finite_progress() {
    #[derive(serde::Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; crate::SLOTS],
        },
    }
    let Fixture::Init { applications } =
        decode(include_bytes!("../../../tests/fixtures/startup.catz")).unwrap();
    let Some(Initial::Patchbay { targets, .. }) = &applications[0] else {
        panic!("fixture");
    };
    let mut workspace = Workspace::new().unwrap();
    let mut graphs = targets
        .iter()
        .filter_map(|target| target.as_ref().map(|value| Some(&value.graph)))
        .collect::<Vec<_>>();
    graphs.push(None);
    let mut compressed = false;
    for graph in graphs {
        let length = workspace.serialize_graph(graph).unwrap();
        compressed |= length > MAX_BLOB;
        let mut packing = workspace.validate_graph(length).unwrap();
        let mut output = buffer(MAX_BLOB).unwrap();
        let turns = length.div_ceil(GRAPH_PACK_BYTES);
        for turn in 0..turns {
            assert_eq!(
                workspace
                    .pack_graph(&mut packing, turn * GRAPH_PACK_BYTES, &mut output)
                    .unwrap(),
                turn + 1 == turns
            );
        }
        let decoded = decode_reply(&output).unwrap();
        let expected = Reply::Graph {
            graph: graph.cloned(),
        };
        assert_eq!(encode(&decoded).unwrap(), encode(&expected).unwrap());
        let mut original = buffer(MAX_BLOB).unwrap();
        workspace
            .encode_reply_into(&expected, &mut original)
            .unwrap();
        // Compression chunk boundaries are private; decoded sequence bytes
        // and canonical graph custody must remain byte-for-byte identical.
        let mut left = buffer(MAX_DECODED_JSON_BYTES).unwrap();
        let mut right = buffer(MAX_DECODED_JSON_BYTES).unwrap();
        assert_eq!(
            decoded_json(&output, &mut left).unwrap(),
            decoded_json(&original, &mut right).unwrap()
        );
    }
    assert!(
        compressed,
        "real graph fixture must exercise incremental compression"
    );
}

#[test]
fn real_projection_preflight_preserves_compressed_graph_and_scope_custody() {
    #[derive(serde::Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; crate::SLOTS],
        },
    }
    let Fixture::Init { applications } =
        decode(include_bytes!("../../../tests/fixtures/startup.catz")).unwrap();
    let Some(Initial::Patchbay { targets, .. }) = &applications[0] else {
        panic!("fixture")
    };
    let Some(Initial::Tutorial { evidence }) = &applications[1] else {
        panic!("fixture Body")
    };
    let mut workspace = Workspace::new().unwrap();
    let mut compressed = false;
    for target in targets.iter().flatten() {
        let scope = crate::graph_facts::Scope::new(
            "host/preflight".into(),
            "boot/preflight".into(),
            evidence.body_id.clone(),
            1,
            &target.graph,
            false,
        )
        .unwrap();
        let command = Command::PrepareGraphProjection {
            graph: target.graph.clone(),
            scope,
        };
        let input = encode_command(&command).unwrap();
        compressed |= input.starts_with(MAGIC);
        let length = workspace.preflight_command(&input).unwrap();
        // Commit consumes the frozen decoded bytes, without a second inflate
        // or framing pass and without accepting a separately supplied payload.
        let returned = workspace.decode_preflighted_command(length).unwrap();
        assert_eq!(encode(&returned).unwrap(), encode(&command).unwrap());
    }
    assert!(
        compressed,
        "real graph projection must exercise compressed preparation"
    );
}
