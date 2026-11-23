//! Actual curl retirement under a fenced HTTP peer; no model service or robot.
use super::*;
use std::time::{Duration, Instant};

#[path = "../../../../targets/std/src/hosted_local_model/ollama/generation_test_support.rs"]
mod transport_fixture;
use transport_fixture::{Response, Server, FRESH, LATE, READY};

#[test]
fn real_transport_cancellation_preserves_cadence_and_fresh_play_custody() {
    use conduit_std_host::hosted_local_model::OllamaDiscovery;
    let server = Server::start([
        Response::Json(READY),
        Response::Hold(LATE),
        Response::Json(FRESH),
        Response::Json(FRESH),
    ]);
    let adapter = OllamaDiscovery {
        endpoint: server.endpoint.clone(),
        runtime_version: "fenced-http-fixture".into(),
        model_name: "fenced-http-fixture".into(),
        model_content_identity: "fenced-http-fixture@1".into(),
        model_bytes: 1,
        architecture: "fixture".into(),
        parameter_profile: "fixture".into(),
        quantization: "fixture".into(),
        context_length: 4096,
        completion_supported: true,
        embedding_supported: false,
        vision_supported: false,
    }
    .initialize(1, vec![conduit_ai::LocalModelKindProfile::GenerateFlow])
    .unwrap();
    server.request(); // The existing initialization probe, not a cognition call.
    let mut harness = harness_for(Box::new(adapter), true);
    let gate = Gate::default();
    let control = RunControl::default();
    let seed = common::inputs(&[]).remove(0);
    let queue = conduit_std_host::BodyLiveForeQueue::for_local_flow_fragment(
        &harness.fragment,
        "frames",
        seed,
        control.clone(),
        8,
        32,
    )
    .unwrap();
    let input = queue.clone();
    let stopping = control.clone();
    let progress = gate.clone();
    let producer = std::thread::spawn(move || {
        struct CloseOnDrop(conduit_std_host::BodyLiveForeQueue, RunControl);
        impl Drop for CloseOnDrop {
            fn drop(&mut self) {
                let _ = self.1.request_stop(
                    conduit_std_host::RunControlRequestId::new("transport-fixture-cleanup")
                        .unwrap(),
                );
                let _ = self.0.close();
            }
        }
        let _cleanup = CloseOnDrop(input.clone(), stopping.clone());
        input.wait_until_play_started().unwrap();
        assert!(matches!(
            input.submit(&frame(1, "First.").bytes).unwrap(),
            conduit_std_host::BodyLiveForeAdmission::Accepted { .. }
        ));
        server.request(); // curl is blocked partway through the response.
        let deadline = Instant::now() + Duration::from_secs(5);
        for revision in 2..=32 {
            let bytes = frame(revision, "Still attending.").bytes;
            loop {
                match input.submit(&bytes).unwrap() {
                    conduit_std_host::BodyLiveForeAdmission::Accepted { .. } => break,
                    conduit_std_host::BodyLiveForeAdmission::Full => {
                        assert!(Instant::now() < deadline, "attention ingress did not drain");
                        std::thread::yield_now();
                    }
                }
            }
        }
        let (lock, changed) = &*progress;
        let (state, timeout) = changed
            .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(5), |state| {
                state.attention < 32 || state.reactions < 4
            })
            .unwrap();
        assert!(
            !timeout.timed_out(),
            "deterministic work stalled behind curl"
        );
        drop(state);
        let cancelled_at = Instant::now();
        stopping
            .request_stop(conduit_std_host::RunControlRequestId::new("stop-stalled-curl").unwrap())
            .unwrap();
        input.close().unwrap();
        (server, cancelled_at)
    });
    let mut collector = ProgressCollector {
        gate: gate.clone(),
        outputs: common::Collector::default(),
    };
    let first = harness
        .host
        .run_external_plot_live_to(
            harness.fragment.clone(),
            &queue,
            &mut collector,
            &mut Vec::new(),
            &mut conduit_std_host::ThreadTimer,
            &control,
        )
        .unwrap();
    let (server, cancelled_at) = producer.join().unwrap();
    assert!(matches!(
        common::terminal(&first),
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(gate.0.lock().unwrap().attention, 32);
    assert_eq!(gate.0.lock().unwrap().reactions, 4);
    assert!(!collector
        .outputs
        .0
        .iter()
        .any(|output| output.front_port_id.as_str() == "episodes"));

    // Reusing the same Host retires and joins A's worker before preparing B.
    // A's delayed response cannot become a response of B, even with equal input.
    server.send_late();
    let mut incoming = common::inputs(&[]);
    incoming.push(frame(1, "First."));
    let mut fresh = common::Collector::default();
    let second = harness
        .host
        .run_external_plot_sequence_controlled_to(
            harness.fragment.clone(),
            &incoming,
            &mut fresh,
            &mut Vec::new(),
            &mut conduit_std_host::ThreadTimer,
            &RunControl::default(),
        )
        .unwrap();
    assert!(cancelled_at.elapsed() < Duration::from_secs(2));
    assert_eq!(common::terminal(&second), &TerminalDisposition::Completed);
    assert_ne!(
        first.kernel.as_ref().unwrap().active_play_id,
        second.kernel.as_ref().unwrap().active_play_id
    );
    let episodes: Vec<_> = fresh
        .0
        .iter()
        .filter(|output| output.front_port_id.as_str() == "episodes")
        .map(|output| conduit_chat::EpisodeRecord::decode(&output.bytes).unwrap())
        .collect();
    assert_eq!(episodes.len(), 2);
    assert!(episodes
        .iter()
        .all(|episode| episode.response().unwrap().content() == "FRESH_B"));
    println!("real-curl cancellation: 4 reactions, 32 attention updates, no A episode, fresh B Play; cancellation-to-B-completion-ms={}", cancelled_at.elapsed().as_millis());
}
