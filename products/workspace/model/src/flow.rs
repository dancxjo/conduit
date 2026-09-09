use crate::WorkspaceBody;
use alloc::{format, string::String, vec::Vec};

impl WorkspaceBody {
    /// A quiet read-only description of the foreground Form's exact Plan.
    /// Arrows describe only a proved single chain; branches remain explicit.
    pub fn foreground_flow(&self) -> String {
        let Some(partition) = self.realization().and_then(|realization| {
            realization
                .plan
                .forms
                .iter()
                .find(|partition| Some(&partition.form) == self.foreground())
        }) else {
            return "Not yet planned".into();
        };
        let fragments = &partition.plan.fragments;
        if fragments.len() != 1 {
            return format!("{} planned fragments", fragments.len());
        }
        let fragment = &fragments[0];
        let nodes = &fragment.placements;
        let edges = &fragment.connections;
        let fallback = || format!("{} Gears · {} Cords", nodes.len(), edges.len());
        if nodes.is_empty() || nodes.len() > 16 || edges.len() + 1 != nodes.len() {
            return fallback();
        }
        let roots: Vec<_> = nodes
            .iter()
            .filter(|node| {
                !edges
                    .iter()
                    .any(|edge| edge.sink_placement_id == node.placement_id)
            })
            .collect();
        if roots.len() != 1 {
            return fallback();
        }
        let mut node = roots[0];
        let mut visited = Vec::with_capacity(nodes.len());
        loop {
            if visited.contains(&node.placement_id) {
                return fallback();
            }
            visited.push(node.placement_id.clone());
            let mut next = edges
                .iter()
                .filter(|edge| edge.source_placement_id == node.placement_id);
            let Some(edge) = next.next() else {
                break;
            };
            if next.next().is_some() {
                return fallback();
            }
            let Some(sink) = nodes
                .iter()
                .find(|sink| sink.placement_id == edge.sink_placement_id)
            else {
                return fallback();
            };
            node = sink;
        }
        if visited.len() != nodes.len() {
            return fallback();
        }
        let labels: Vec<_> = visited
            .iter()
            .map(|id| {
                nodes
                    .iter()
                    .find(|node| &node.placement_id == id)
                    .expect("visited planned node")
                    .kind_id
                    .as_str()
                    .rsplit('/')
                    .next()
                    .unwrap_or("Gear")
            })
            .collect();
        let label = if labels.len() > 6 {
            format!("{} → … → {}", labels[0], labels[labels.len() - 1])
        } else {
            labels.join(" → ")
        };
        if label.len() > 256 { fallback() } else { label }
    }
}
