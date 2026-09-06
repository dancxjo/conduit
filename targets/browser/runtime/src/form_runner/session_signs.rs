//! Preserve the existing mandatory kernel log, without inventing causal edges.
use super::{TourReceipt, TourSession};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct KernelSignEvidence {
    schema: &'static str,
    host_id: String,
    boot_id: String,
    active_play_id: String,
    item_capacity: u16,
    placements: Vec<PlacementBinding>,
    events: Vec<KernelEventEvidence>,
}

#[derive(Debug, Serialize)]
struct PlacementBinding {
    node: u16,
    plan_id: String,
    fragment_id: String,
    placement_id: String,
}

#[derive(Debug, Serialize)]
struct KernelEventEvidence {
    sequence: u32,
    node: u16,
    port: Option<u16>,
    request: Option<u32>,
    kind: String,
}

impl TourSession {
    pub(super) fn with_kernel_signs(&self, mut receipt: TourReceipt) -> TourReceipt {
        use conduit_kernel::SignSink;
        let log = self.scheduler.signs();
        // Both collections inherit already-admitted kernel bounds: at most
        // MAXIMUM_BROWSER_GEARS placements and BROWSER_SIGN_ITEMS events.
        let placements = self
            .fragments
            .iter()
            .flat_map(|fragment| {
                fragment
                    .placements
                    .iter()
                    .map(move |placement| (fragment, placement))
            })
            .enumerate()
            .map(|(node, (fragment, placement))| PlacementBinding {
                node: node as u16,
                plan_id: fragment.plan_id.as_str().into(),
                fragment_id: fragment.fragment_id.as_str().into(),
                placement_id: placement.placement_id.as_str().into(),
            })
            .collect();
        let events = log
            .events()
            .map(|event| KernelEventEvidence {
                sequence: event.sequence,
                node: event.node.0,
                port: event.port.map(|port| port.0),
                request: event.request.map(|request| request.0),
                kind: format!("{:?}", event.kind),
            })
            .collect();
        receipt.kernel_signs = Some(KernelSignEvidence {
            schema: "conduit.browser/kernel-sign-evidence@1",
            host_id: self.host_id.as_str().into(),
            boot_id: self.boot_id.as_str().into(),
            active_play_id: self.active_play_id.as_str().into(),
            item_capacity: log.item_capacity(),
            placements,
            events,
        });
        receipt
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn receipt_preserves_exact_kernel_records_and_partition_bindings() {
        let request = super::super::body_start::tests::request();
        let (session, started) = super::super::body_start::prepare(request).unwrap();
        let before = session.scheduler.signs().events().collect::<Vec<_>>();
        let bindings = session
            .fragments
            .iter()
            .flat_map(|fragment| {
                fragment.placements.iter().map(|placement| {
                    (
                        fragment.plan_id.as_str().to_owned(),
                        placement.placement_id.as_str().to_owned(),
                    )
                })
            })
            .collect::<Vec<_>>();
        let receipt = session.cancel().unwrap();
        let evidence = receipt.kernel_signs.as_ref().unwrap();
        assert_eq!(
            evidence.active_play_id,
            started.play.active_play_id.as_str()
        );
        assert!(evidence.events.len() <= usize::from(evidence.item_capacity));
        for (actual, original) in evidence.events.iter().zip(&before) {
            assert_eq!(actual.sequence, original.sequence);
            assert_eq!(actual.node, original.node.0);
            assert_eq!(actual.port, original.port.map(|port| port.0));
            assert_eq!(actual.request, original.request.map(|request| request.0));
            assert_eq!(actual.kind, format!("{:?}", original.kind));
        }
        assert!(evidence.events.len() >= before.len());
        assert!(evidence
            .events
            .iter()
            .any(|event| event.kind == "RunCancelled"));
        for (node, (plan, placement)) in bindings.iter().enumerate() {
            assert_eq!(evidence.placements[node].node, node as u16);
            assert_eq!(&evidence.placements[node].plan_id, plan);
            assert_eq!(&evidence.placements[node].placement_id, placement);
        }
        assert!(serde_json::to_vec(&receipt).unwrap().len() < super::super::abi::OUTPUT_BYTES);
    }
}
