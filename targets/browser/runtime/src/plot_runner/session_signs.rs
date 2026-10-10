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
    prepared_storage: PreparedStorageEvidence,
    retention_gap: Option<KernelSignGap>,
    placements: Vec<PlacementBinding>,
    events: Vec<KernelEventEvidence>,
    host_completions: super::host_outcomes::HostOutcomeEvidence,
}

/// Preparation evidence distinguishes allocated backing from the live-byte quota.
#[derive(Debug, Serialize)]
struct PreparedStorageEvidence {
    partitions: Vec<StoragePartition>,
    slot_capacities: Vec<u32>,
    reserved_storage_bytes: usize,
    live_byte_quota: u32,
    data_memory_reservation: u64,
    memory_pool_bytes: u32,
}

#[derive(Debug, Serialize)]
struct StoragePartition {
    plan_id: String,
    fragment_id: String,
}

#[derive(Debug, Serialize)]
struct KernelSignGap {
    first_sequence: u32,
    last_sequence: u32,
    entries: u32,
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
        receipt.kernel_signs = Some(self.kernel_signs());
        receipt
    }
    pub(super) fn kernel_signs(&self) -> KernelSignEvidence {
        use conduit_kernel::{SignQuery, SignSink};
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
        KernelSignEvidence {
            schema: "conduit.browser/kernel-sign-evidence@1",
            host_id: self.host_id.as_str().into(),
            boot_id: self.boot_id.as_str().into(),
            active_play_id: self.active_play_id.as_str().into(),
            item_capacity: log.item_capacity(),
            prepared_storage: {
                let storage = &self.scheduler._prepared_storage;
                PreparedStorageEvidence {
                    partitions: storage
                        .identities
                        .iter()
                        .map(|(plan, fragment)| StoragePartition {
                            plan_id: plan.as_str().into(),
                            fragment_id: fragment.as_str().into(),
                        })
                        .collect(),
                    slot_capacities: storage.slot_capacities.clone(),
                    reserved_storage_bytes: storage.reserved_storage_bytes,
                    live_byte_quota: storage.live_byte_quota,
                    data_memory_reservation: storage.data_memory_reservation,
                    memory_pool_bytes:
                        crate::installed_browser::measurement_limits::MEMORY_POOL_BYTES,
                }
            },
            retention_gap: log.retention_gap().map(|gap| KernelSignGap {
                first_sequence: gap.first_sequence,
                last_sequence: gap.last_sequence,
                entries: gap.entries,
            }),
            placements,
            events,
            host_completions: self.host_outcomes.evidence(),
        }
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
