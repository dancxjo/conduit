//! Existing-pool admission, not a self-created provider memory budget.
use super::*;

#[derive(Default)]
struct AdmissionTimer {
    started: bool,
}
impl TimerAdapter for AdmissionTimer {
    fn wait(&mut self, _: Duration) {}
    fn observe_kernel_phase(&mut self, phase: crate::KernelTimingPhase) {
        if matches!(phase, crate::KernelTimingPhase::PlayStarted) {
            self.started = true;
        }
    }
}
#[derive(Default)]
struct NoDelivery(usize);
impl ExternalForeOutputAdapter for NoDelivery {
    fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
        self.0 += 1;
        Ok(())
    }
}

#[test]
fn missing_or_undersized_existing_memory_pool_refuses_before_play_and_effects() {
    for missing in [true, false] {
        let root = tempfile::tempdir().unwrap();
        let snapshots = root.path().join("snapshots");
        std::fs::create_dir(&snapshots).unwrap();
        let state = root.path().join("state");
        let (mut host, fragment, selected) =
            fixture(&snapshots, "boot/memory-refusal", Mode::Publish);
        let placement = fragment
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str()
                    == conduit_std_offers::JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION
            })
            .unwrap();
        let memory = placement
            .resources
            .iter()
            .find(|resource| resource.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
            .unwrap();
        assert_eq!(
            memory.units,
            conduit_std_offers::JSON_SNAPSHOT_PREPARATION_BYTES
        );
        assert!(
            host.advertisement().resources.iter().any(|offer| {
                offer.pool_id == memory.pool_id
                    && offer.class_id == memory.class_id
                    && offer.capacity_units >= memory.units
            }),
            "fixture must start with a real offered preparation pool"
        );
        let baseline = StdHost::new_with_config(StdHostConfig {
            host_id: host.advertisement().host_id.clone(),
            boot_id: host.advertisement().boot_id.clone(),
            offer_generation: host.advertisement().offer_generation,
        });
        let selected_memory = host
            .advertisement()
            .resources
            .iter()
            .find(|offer| offer.pool_id == memory.pool_id)
            .unwrap();
        assert!(
            baseline.advertisement().resources.contains(selected_memory),
            "installing JSON must not invent or enlarge a memory pool"
        );
        let pool = memory.pool_id.clone();
        if missing {
            host.advertisement
                .resources
                .retain(|offer| offer.pool_id != pool);
        } else {
            let offered = host
                .advertisement
                .resources
                .iter_mut()
                .find(|offer| offer.pool_id == pool)
                .unwrap();
            offered.capacity_units = memory.units - 1;
        }
        // The sealed fragment still names the original exact binding. Refresh
        // only the actual available resource ledger; never modify the Plan.
        host.kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(host.advertisement()).unwrap();
        let next_play = host.next_kernel_play_sequence;
        let mut timer = AdmissionTimer::default();
        let mut deliveries = NoDelivery::default();
        let result = host.run_body_durable_json_snapshot_to(
            &body(),
            &state,
            &selected,
            fragment,
            &[ExternalForeInput {
                front_port_id: port_id("json"),
                track: ConnectionTrack::Payload,
                bytes: JSON.to_vec(),
            }],
            2,
            &mut deliveries,
            &mut Vec::new(),
            &mut timer,
            &RunControl::default(),
        );
        assert!(
            result.is_err(),
            "missing={missing}: unavailable memory returned a successful run"
        );
        assert!(
            !timer.started,
            "missing={missing}: refusal must precede PlayStarted"
        );
        assert_eq!(
            host.next_kernel_play_sequence, next_play,
            "memory admission must precede issuing Play"
        );
        assert_eq!(deliveries.0, 0);
        assert_eq!(
            std::fs::read_dir(&snapshots).unwrap().count(),
            0,
            "no candidate or published file may appear"
        );
        assert!(
            !state.exists(),
            "no State recovery/commit residence may be opened"
        );
    }
}

#[test]
fn invalid_history_capacity_refuses_before_play_without_changing_queue_capacity() {
    for capacity in [0, 65] {
        let root = tempfile::tempdir().unwrap();
        let snapshots = root.path().join("snapshots");
        std::fs::create_dir(&snapshots).unwrap();
        let state = root.path().join("state");
        let (mut host, fragment, selected) =
            fixture(&snapshots, "boot/history-refusal", Mode::Publish);
        assert!(fragment
            .fore_ports
            .iter()
            .all(|port| port.item_capacity == 1));
        let mut timer = AdmissionTimer::default();
        let mut deliveries = NoDelivery::default();
        let next_play = host.next_kernel_play_sequence;
        let result = host.run_body_durable_json_snapshot_to(
            &body(),
            &state,
            &selected,
            fragment,
            &[ExternalForeInput {
                front_port_id: port_id("json"),
                track: ConnectionTrack::Payload,
                bytes: JSON.to_vec(),
            }],
            capacity,
            &mut deliveries,
            &mut Vec::new(),
            &mut timer,
            &RunControl::default(),
        );
        assert!(result.is_err());
        assert!(!timer.started);
        assert_eq!(host.next_kernel_play_sequence, next_play);
        assert_eq!(deliveries.0, 0);
        assert_eq!(std::fs::read_dir(&snapshots).unwrap().count(), 0);
        assert!(!state.exists());
    }
}
