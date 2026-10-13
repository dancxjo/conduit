//! Actual expanded Source boundary routes, independent of phase predicates.
use super::*;

#[test]
fn actual_external_request_graph_cuts_at_authorizers_before_state_or_transport() {
    let source = source();
    let entry = source.expand("bot-storage-read-reentry").unwrap();
    let graph = &entry.expanded;
    let cuts = graph
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == "storage/block-read/authorize")
        .map(|gear| gear.gear_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(cuts.len(), 2);
    let mut pending = vec![entry.input_bindings[0].gear_id.clone()];
    let mut visited = Vec::new();
    let mut reached = Vec::new();
    while let Some(id) = pending.pop() {
        if visited.contains(&id) {
            continue;
        }
        visited.push(id.clone());
        if cuts.contains(&id) {
            reached.push(id);
            continue;
        }
        let gear = graph.gears.iter().find(|gear| gear.gear_id == id).unwrap();
        let kind = gear.kind_id.as_str();
        assert!(
            !kind.starts_with("machine/usb/")
                && kind != "storage/block-read/settle"
                && !kind.starts_with("flow/merge/finite")
                && kind != "flow/concat/finite"
                && kind != "flow/singleton"
                && kind != "bytes/literal"
                && !kind.starts_with("state/seeded/"),
            "raw input bypasses authorizer through {kind}"
        );
        pending.extend(
            graph
                .connections
                .iter()
                .filter(|edge| edge.source_gear_id == id)
                .map(|edge| edge.sink_gear_id.clone()),
        );
    }
    assert_eq!(
        reached.len(),
        2,
        "both independently selected authorizer cuts must be reachable"
    );
}
