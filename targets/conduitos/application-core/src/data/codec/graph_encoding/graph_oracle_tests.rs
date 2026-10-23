//! Exact populated production reply bytes and finite work, before packing.
use super::super::{MAX_DECODED_JSON_BYTES, decode, transport_codec};
use super::*;
use crate::data::Initial;
use alloc::vec;
use patchbay_graph::PatchbayGraph;
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
enum BorrowedReply<'a> {
    Graph { graph: Option<&'a PatchbayGraph> },
}
fn prove(graph: Option<&PatchbayGraph>) -> usize {
    let mut expected = vec![0; MAX_DECODED_JSON_BYTES];
    let length =
        transport_codec::encode_unvalidated(&BorrowedReply::Graph { graph }, &mut expected)
            .unwrap();
    let mut actual = vec![0; MAX_DECODED_JSON_BYTES];
    let mut encoder = GraphEncoding::new();
    let mut turns = 0;
    loop {
        let before = encoder.cursor();
        let complete = encoder.step(graph, &mut actual).unwrap();
        turns += 1;
        assert!(turns <= MAX_TURNS);
        let work = encoder.cursor() - before;
        assert!(work > 0 && work <= u64::from(STEP_WORK));
        if complete {
            break;
        }
        assert!(work >= u64::from(STEP_WORK - RESOLVE_RESERVE));
    }
    assert_eq!(encoder.length(), length);
    assert_eq!(&actual[..length], &expected[..length]);
    assert_eq!(encoder.step(graph, &mut actual), Err(Refusal::Order));
    turns as usize
}
#[test]
fn all_real_populated_graphs_and_absence_match_exact_existing_encoder() {
    #[derive(Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; crate::SLOTS],
        },
    }
    let Fixture::Init { applications } =
        decode(include_bytes!("../../../../tests/fixtures/startup.catz")).unwrap();
    let Some(Initial::Patchbay { targets, .. }) = &applications[0] else {
        panic!("actual populated fixture")
    };
    let mut populated = 0;
    for target in targets.iter().flatten() {
        assert!(!target.graph.gears.is_empty());
        assert!(prove(Some(&target.graph)) > 1);
        populated += 1;
    }
    assert!(populated > 0);
    assert!(prove(None) >= 1);
}
#[test]
fn capacity_and_terminal_failures_never_restart_or_publish() {
    let mut encoder = GraphEncoding::new();
    let mut output = [0; 1];
    assert_eq!(encoder.step(None, &mut output), Err(Refusal::Capacity));
    assert_eq!(encoder.step(None, &mut output), Err(Refusal::Order));
}

#[test]
fn oversized_output_slice_cannot_enlarge_the_decoded_ceiling() {
    let text = "x".repeat(MAX_DECODED_JSON_BYTES);
    let mut encoder = GraphEncoding::for_node();
    let mut output = vec![0; MAX_DECODED_JSON_BYTES + 128];
    let mut refused = false;
    for _ in 0..MAX_TURNS {
        match encoder.step_node(Node::Str(&text), &mut output) {
            Ok(false) => {}
            Err(Refusal::Capacity) => {
                refused = true;
                break;
            }
            result => panic!("oversized output must refuse, got {result:?}"),
        }
    }
    assert!(refused);
    assert_eq!(encoder.length(), MAX_DECODED_JSON_BYTES);
    assert_eq!(
        encoder.step_node(Node::Str(&text), &mut output),
        Err(Refusal::Order)
    );
}
