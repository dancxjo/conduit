//! Every external request route must authorize before reaching physical work.
pub(super) fn authorized_route<'a>(
    ingress: &'a str,
    authorize: &str,
    placements: &[(&str, &str, bool)],
    connections: &[(&'a str, &'a str)],
) -> bool {
    authorized_routes(ingress, &[authorize], placements, connections)
}

pub(super) fn authorized_routes<'a>(
    ingress: &'a str,
    authorizers: &[&str],
    placements: &[(&str, &str, bool)],
    connections: &[(&'a str, &'a str)],
) -> bool {
    if authorizers.is_empty()
        || authorizers.len() > 2
        || authorizers
            .iter()
            .enumerate()
            .any(|(index, id)| id.is_empty() || authorizers[..index].contains(id))
    {
        return false;
    }
    let mut pending = alloc::vec![ingress];
    let mut visited = alloc::vec::Vec::new();
    let mut reaches_authorize = false;
    while let Some(id) = pending.pop() {
        if visited.contains(&id) {
            continue;
        }
        visited.push(id);
        if authorizers.contains(&id) {
            reaches_authorize = true;
            continue;
        }
        let Some((_, implementation, possesses_authority)) =
            placements.iter().find(|(placement, _, _)| *placement == id)
        else {
            return false;
        };
        if *possesses_authority
            || !matches!(
                *implementation,
                crate::expression_host_call::IMPLEMENTATION
                    | crate::structured_selector_host_call::IMPLEMENTATION
                    | crate::seeded_state::IMPLEMENTATION
                    | crate::flow_zip::IMPLEMENTATION
            )
        {
            return false;
        }
        for (source, sink) in connections {
            if *source == id {
                pending.push(*sink);
            }
        }
    }
    reaches_authorize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expression_host_call::IMPLEMENTATION as PURE, seeded_state::IMPLEMENTATION as STATE,
    };

    #[test]
    fn direct_and_cyclic_producer_routes_authorize_before_effects() {
        let nodes = [
            ("start", PURE, false),
            ("state", STATE, false),
            ("usb", "physical-usb", true),
        ];
        let edges = [
            ("start", "state"),
            ("state", "state"),
            ("state", "authorize"),
            ("authorize", "usb"),
        ];
        assert!(authorized_route("authorize", "authorize", &nodes, &edges));
        assert!(authorized_route("start", "authorize", &nodes, &edges));
    }

    #[test]
    fn physical_bypass_is_rejected_even_with_an_authorized_branch() {
        for implementation in ["usb-control", "usb-read", "usb-write", "storage-settle"] {
            let nodes = [("start", PURE, false), ("effect", implementation, false)];
            let edges = [("start", "authorize"), ("start", "effect")];
            assert!(!authorized_route("start", "authorize", &nodes, &edges));
            assert!(!authorized_route("effect", "authorize", &nodes, &edges));
        }
    }

    #[test]
    fn authority_unknown_missing_and_unconnected_routes_are_rejected() {
        for node in [("start", PURE, true), ("start", "uninstalled", false)] {
            assert!(!authorized_route(
                "start",
                "authorize",
                &[node],
                &[("start", "authorize")]
            ));
        }
        assert!(!authorized_route("missing", "authorize", &[], &[]));
        assert!(!authorized_route(
            "start",
            "authorize",
            &[("start", STATE, false)],
            &[("start", "start")]
        ));
    }
    #[test]
    fn independently_authorized_branches_still_reject_any_physical_bypass() {
        let nodes = [
            ("start", PURE, false),
            ("later", STATE, false),
            ("usb", "usb-read", true),
        ];
        let edges = [("start", "first"), ("start", "later"), ("later", "second")];
        assert!(authorized_routes(
            "start",
            &["first", "second"],
            &nodes,
            &edges
        ));
        let mut bypass = edges.to_vec();
        bypass.push(("later", "usb"));
        assert!(!authorized_routes(
            "start",
            &["first", "second"],
            &nodes,
            &bypass
        ));
        assert!(!authorized_routes(
            "start",
            &["first", "first"],
            &nodes,
            &edges
        ));
        assert!(!authorized_routes("start", &[], &nodes, &edges));
    }
}
