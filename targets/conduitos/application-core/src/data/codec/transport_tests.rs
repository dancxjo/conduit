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
