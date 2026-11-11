//! Three ordinary Forms under one admitted installed Play; the model is fenced.
#[path = "../../../targets/std/tests/common_episode_loop/mod.rs"]
mod common;
use conduit_core::{CheckedValueContract, ConnectionTrack, PortDirection, TerminalDisposition};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{ExternalForeInput, HostedFrameBack, RunControl, StdHostComposition};

const PROGRAM: &str = include_str!("../../../plots/pete-cognition-cadenced/main.conduit");
fn contracts() -> (CheckedValueContract, CheckedValueContract) {
    (
        CheckedValueContract::new(
            conduit_chat::episode_experience_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
            vec![],
        )
        .unwrap(),
        CheckedValueContract::new(conduit_core::kind_id("value/u64"), 8, vec![]).unwrap(),
    )
}
fn harness() -> common::Harness {
    harness_with_model(common::fenced_model().0)
}
fn harness_with_model(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
) -> common::Harness {
    harness_for(model, false)
}
fn harness_for(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
    timed: bool,
) -> common::Harness {
    let back = HostedFrameBack::new(
        conduit_pete::pete_cognition_inputs_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        conduit_pete::prepare_pete_experience_input_bytes,
    )
    .unwrap();
    let mut host = conduit_std_host::StdHost::new_with_local_model_and_frame_back(
        conduit_std_host::StdHostConfig {
            host_id: "host/pete-cadence".into(),
            boot_id: "boot/pete-cadence".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::minimal().with_time(),
        model,
        back,
    )
    .unwrap();
    let (value, mut trigger) = contracts();
    if timed {
        trigger.value_kind = conduit_core::kind_id(conduit_time::TICK_VALUE_KIND);
    }
    host.install_finite_current_sampling(&value, &trigger)
        .unwrap();
    let program = if timed {
        include_str!("../../../plots/pete-cognition-cadenced/timed.conduit")
    } else {
        PROGRAM
    };
    let mut fore = vec![
        ("context", PortDirection::Input, 32768),
        (
            "frames",
            PortDirection::Input,
            conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        ),
        (
            "reactions",
            PortDirection::Output,
            conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        ),
        (
            "attention",
            PortDirection::Output,
            conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        ),
        ("episodes", PortDirection::Output, 32768),
    ];
    if !timed {
        fore.push(("reflections", PortDirection::Input, 8));
    }
    common::plan_host(
        host,
        program,
        "pete-cognition",
        &fore,
        false,
        |startup, profile| {
            if timed {
                conduit_time::install_tick_catalog(startup, profile)?;
            }
            conduit_pete::install_pete_cognition_catalog(startup, profile)?;
            conduit_semantic_catalog::install_flow_coalesce_finite_kind(&value, startup, profile)?;
            conduit_semantic_catalog::install_current_sample_finite_kind(
                &value, &trigger, startup, profile,
            )
        },
    )
}
fn frame(revision: u64, message: &str) -> ExternalForeInput {
    use conduit_pete::*;
    let now = conduit_time::NativeTemporalInstant::new(
        "clock/pete-cadence".into(),
        1,
        conduit_time::NativeTemporalScale::Milliseconds,
        revision * 100,
        0,
    )
    .unwrap();
    let frame = PeteCognitionFrame::new(
        PeteCognitionBatteryInput::Missing,
        None,
        "pete/cadence-source".into(),
        now.clone(),
        revision,
        conduit_human::ExperienceTemporalPolicy::new(10, 20).unwrap(),
        PeteCognitionUtterance::new(
            message.into(),
            now,
            matches!(revision, 1 | 8 | 16 | 24),
            PeteCognitionSign::new(format!("utterance/{revision}")).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    ExternalForeInput {
        front_port_id: "frames".into(),
        track: ConnectionTrack::Payload,
        bytes: frame.encode().unwrap(),
    }
}
fn trigger(sequence: u64) -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: "reflections".into(),
        track: ConnectionTrack::Payload,
        bytes: sequence.to_le_bytes().to_vec(),
    }
}
#[test]
fn pete_cadence_runs_three_forms_and_reflects_less_often_than_events() {
    let mut harness = harness();
    let mut incoming = common::inputs(&[]);
    incoming.extend([
        frame(1, "The hallway label is COPPER KITE."),
        trigger(1),
        frame(2, "I passed the hallway."),
        frame(3, "What was the hallway label?"),
        trigger(2),
    ]);
    let mut collector = common::Collector::default();
    let report = common::run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(common::terminal(&report), &TerminalDisposition::Completed);
    let count = |name| {
        collector
            .0
            .iter()
            .filter(|item| {
                item.front_port_id.as_str() == name && item.track == ConnectionTrack::Payload
            })
            .count()
    };
    assert_eq!(count("reactions"), 1);
    assert_eq!(count("attention"), 3);
    assert_eq!(count("episodes"), 2);
    let episodes = collector
        .0
        .iter()
        .filter(|item| item.front_port_id.as_str() == "episodes")
        .map(|item| conduit_chat::EpisodeRecord::decode(&item.bytes).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(*episodes[1].episode(), 2);
    assert_eq!(*episodes[1].recalled().get()[0].episode(), 1);
    assert_eq!(episodes[0].body_id(), episodes[1].body_id());
    assert_eq!(episodes[0].wake_id(), episodes[1].wake_id());
}

#[derive(Default)]
struct GateState {
    started: bool,
    reactions: usize,
    attention: usize,
    progressed_during_call: bool,
}
type Gate = std::sync::Arc<(std::sync::Mutex<GateState>, std::sync::Condvar)>;
struct SlowThenLostModel {
    inner: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
    gate: Gate,
    calls: usize,
    pending_since: Option<std::time::Instant>,
}
impl conduit_std_host::hosted_local_model::HostedLocalModelAdapter for SlowThenLostModel {
    fn offer(&self) -> &conduit_ai::LocalModelOffer {
        self.inner.offer()
    }
    fn current_pool_health(&self) -> conduit_core::PoolRealizationHealth {
        self.inner.current_pool_health()
    }
    fn execute(
        &mut self,
        placement: &conduit_core::PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> conduit_std_host::hosted_local_model::LocalModelAdapterTerminal {
        self.inner.execute(placement, input, output)
    }
    fn begin_generation(
        &mut self,
        placement: &conduit_core::PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> Option<conduit_std_host::hosted_local_model::LocalModelAdapterTerminal> {
        self.calls += 1;
        if self.calls == 1 {
            let (lock, changed) = &*self.gate;
            let mut state = lock.lock().unwrap();
            assert_eq!(state.reactions, 1);
            self.pending_since = Some(std::time::Instant::now());
            state.started = true;
            changed.notify_all();
            None
        } else {
            Some(self.inner.execute(placement, input, output))
        }
    }
    fn poll_generation(
        &mut self,
        _: &mut Vec<u8>,
    ) -> Option<conduit_std_host::hosted_local_model::LocalModelAdapterTerminal> {
        let mut state = self.gate.0.lock().unwrap();
        assert!(
            self.pending_since.unwrap().elapsed() < std::time::Duration::from_secs(5),
            "bounded deterministic work did not complete"
        );
        if state.reactions < 4 || state.attention < 32 {
            return None;
        }
        state.progressed_during_call = true;
        Some(conduit_std_host::hosted_local_model::LocalModelAdapterTerminal::ProviderLost)
    }
}
struct ProgressCollector {
    gate: Gate,
    outputs: common::Collector,
}
impl conduit_std_host::ExternalForeOutputAdapter for ProgressCollector {
    fn deliver(&mut self, output: conduit_std_host::ExternalForeDelivery) -> Result<(), String> {
        if output.track == ConnectionTrack::Payload {
            let (lock, changed) = &*self.gate;
            let mut state = lock.lock().unwrap();
            match output.front_port_id.as_str() {
                "reactions" => state.reactions += 1,
                "attention" => state.attention += 1,
                _ => {}
            }
            changed.notify_all();
            drop(state);
        }
        self.outputs.0.push(output);
        Ok(())
    }
}
#[test]
fn pete_cadence_keeps_reacting_and_attending_during_slow_provider_loss() {
    let gate = Gate::default();
    let model = SlowThenLostModel {
        inner: common::fenced_model().0,
        gate: gate.clone(),
        calls: 0,
        pending_since: None,
    };
    let mut harness = harness_for(Box::new(model), true);
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
    let observed = gate.clone();
    let producer = std::thread::spawn(move || {
        input.wait_until_play_started().unwrap();
        assert!(matches!(
            input
                .submit(&frame(1, "The hallway label is COPPER KITE.").bytes)
                .unwrap(),
            conduit_std_host::BodyLiveForeAdmission::Accepted { sequence: 0 }
        ));
        let (lock, changed) = &*observed;
        let (state, timed) = changed
            .wait_timeout_while(
                lock.lock().unwrap(),
                std::time::Duration::from_secs(5),
                |s| !s.started,
            )
            .unwrap();
        assert!(!timed.timed_out(), "first reflection did not start");
        drop(state);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        for revision in 2..=32 {
            let bytes = frame(revision, "The hallway remains visible.").bytes;
            loop {
                match input.submit(&bytes).unwrap() {
                    conduit_std_host::BodyLiveForeAdmission::Accepted { .. } => break,
                    conduit_std_host::BodyLiveForeAdmission::Full => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "bounded ingress did not drain"
                        );
                        std::thread::yield_now();
                    }
                }
            }
        }
        input.close().unwrap();
    });
    let mut collector = ProgressCollector {
        gate: gate.clone(),
        outputs: common::Collector::default(),
    };
    let report = harness
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
    producer.join().unwrap();
    assert_eq!(common::terminal(&report), &TerminalDisposition::Completed);
    let state = gate.0.lock().unwrap();
    assert!(state.progressed_during_call);
    assert_eq!(state.reactions, 4);
    assert_eq!(state.attention, 32);
    drop(state);
    let episodes = collector
        .outputs
        .0
        .iter()
        .filter(|item| item.front_port_id.as_str() == "episodes")
        .map(|item| conduit_chat::EpisodeRecord::decode(&item.bytes).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(episodes.len(), 2);
    assert_eq!(episodes[0].observation().identity(), "utterance/1");
    assert_eq!(episodes[1].observation().identity(), "utterance/32");
    assert_eq!(*episodes[1].recalled().get()[0].episode(), 1);
    assert!(matches!(
        episodes[0].outcome(),
        conduit_chat::EpisodeOutcome::ProviderLost(_)
    ));
    assert!(matches!(
        episodes[1].outcome(),
        conduit_chat::EpisodeOutcome::Produced(_)
    ));
    assert_eq!(episodes[0].body_id(), episodes[1].body_id());
    assert_eq!(episodes[0].wake_id(), episodes[1].wake_id());
    assert_eq!(
        harness
            .fragment
            .connections
            .iter()
            .filter(
                |cord| cord.pressure_policy == conduit_core::DeliveryPressurePolicy::CoalesceLatest
            )
            .count(),
        1
    );
}

#[path = "pete_cadence/cancellation.rs"]
mod cancellation;
