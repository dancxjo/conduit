#[cfg(unix)]
mod unix {
    use super::super::*;
    use crate::hosted_local_model::ollama::{
        generation_transport::test_support::*,
        work_budget::tests::{finite_adapter_fixture, placement, set},
    };
    use std::time::{Duration, Instant};

    fn fixture(endpoint: String) -> (OllamaLocalModelAdapter, PlannedGear) {
        let mut adapter = finite_adapter_fixture();
        adapter.endpoint = endpoint;
        adapter
            .offer
            .supported_profiles
            .push(LocalModelKindProfile::GenerateFlow);
        let mut flow = placement();
        flow.kind_id = conduit_ai::LLM_GENERATE_FLOW_KIND.into();
        set(&mut flow, "maximum-input-bytes", 8);
        set(&mut flow, "maximum-output-bytes", 1024);
        adapter
            .prepare_background_generation(core::slice::from_ref(&flow))
            .unwrap();
        (adapter, flow)
    }

    fn complete(adapter: &mut OllamaLocalModelAdapter) -> (LocalModelAdapterTerminal, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut output = Vec::new();
        loop {
            if let Some(terminal) = adapter.poll_generation(&mut output) {
                return (terminal, output);
            }
            assert!(
                Instant::now() < deadline,
                "generation cleanup exceeded two seconds"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn cancelled_worker_keeps_one_slot_until_cleanup_and_suppresses_late_result() {
        let server = Server::start([Response::Hold(LATE), Response::Json(FRESH)]);
        let (mut adapter, flow) = fixture(server.endpoint.clone());
        let mut output = Vec::new();
        assert_eq!(adapter.begin_generation(&flow, b"A", &mut output), None);
        server.request();
        assert_eq!(
            adapter.begin_generation(&flow, b"B", &mut output),
            Some(LocalModelAdapterTerminal::Refused)
        );
        adapter.cancel_generation();
        server.send_late();
        let (terminal, output) = complete(&mut adapter);
        assert_eq!(terminal, LocalModelAdapterTerminal::Cancelled);
        assert!(output.is_empty());
        let mut output = Vec::new();
        assert_eq!(adapter.begin_generation(&flow, b"B", &mut output), None);
        let (terminal, output) = complete(&mut adapter);
        assert_eq!(terminal, LocalModelAdapterTerminal::Produced);
        let response: ModelDerivedResult = serde_json::from_slice(&output).unwrap();
        assert_eq!(response.payload, b"FRESH_B");
    }

    #[test]
    fn replacement_joins_old_worker_and_old_token_cannot_cancel_fresh_work() {
        let server = Server::start([Response::Hold(LATE), Response::Hold(FRESH)]);
        let (mut adapter, flow) = fixture(server.endpoint.clone());
        let old = adapter
            .generation_worker
            .as_ref()
            .unwrap()
            .cancellation
            .clone();
        let mut output = Vec::new();
        assert_eq!(adapter.begin_generation(&flow, b"A", &mut output), None);
        server.request();
        let started = Instant::now();
        adapter
            .prepare_background_generation(core::slice::from_ref(&flow))
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        server.closed.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(adapter.begin_generation(&flow, b"B", &mut output), None);
        server.request();
        old.cancel();
        server.send_late();
        let (terminal, output) = complete(&mut adapter);
        assert_eq!(terminal, LocalModelAdapterTerminal::Produced);
        let response: ModelDerivedResult = serde_json::from_slice(&output).unwrap();
        assert_eq!(response.payload, b"FRESH_B");
        assert!(!adapter.offer.limits.cancellation_supported);
    }

    #[test]
    fn repeated_stalled_calls_retire_every_transport_before_reusing_slot() {
        const CALLS: usize = 16;
        let server = Server::start((0..CALLS).map(|_| Response::Hold(LATE)));
        let (mut adapter, flow) = fixture(server.endpoint.clone());
        let mut maximum = Duration::ZERO;
        for sequence in 0..CALLS {
            let mut output = Vec::new();
            assert_eq!(adapter.begin_generation(&flow, b"A", &mut output), None);
            server.request();
            let started = Instant::now();
            adapter.cancel_generation();
            assert_eq!(
                complete(&mut adapter),
                (LocalModelAdapterTerminal::Cancelled, Vec::new())
            );
            assert_eq!(
                server.closed.recv_timeout(Duration::from_secs(2)).unwrap(),
                sequence
            );
            maximum = maximum.max(started.elapsed());
            assert!(adapter.generation_worker.as_ref().unwrap().slot.is_some());
        }
        adapter.prepare_background_generation(&[]).unwrap();
        assert!(adapter.generation_worker.is_none());
        assert!(maximum < Duration::from_secs(2));
        println!(
            "cancelled-transports={CALLS}; maximum-release-ms={}",
            maximum.as_millis()
        );
    }
}
