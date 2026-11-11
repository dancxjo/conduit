//! One finite scripted proof using the ordinary continuing Body runner.
use super::{evidence, plans};
use conduit_ai::{LocalModelOffer, ModelDerivedResult};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::PlannedGear;
use conduit_human::{ExperienceAutobiographyState, ExperienceEvidenceRequest};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    body_execution::{BodyForeOutputAdapter, BodyRunRequest},
    hosted_local_model::{
        HostedLocalModelAdapter, LocalModelAdapterTerminal, LocalModelRuntimeState,
        LocalModelStreamStep, OllamaLocalModelAdapter, StreamingChunkDisposition,
    },
    ExternalForeDelivery, RunControl, RunControlRequestId, StdHost, StdHostComposition,
    StdHostConfig, TimerAdapter,
};
pub(super) use evidence::Receipt;
use std::{
    error::Error,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub(super) struct ProviderCapture {
    pub requests: [Vec<u8>; 2],
    pub terminals: [Option<LocalModelAdapterTerminal>; 2],
    pub calls: usize,
}
struct CapturingProvider {
    inner: Box<dyn HostedLocalModelAdapter>,
    capture: Arc<Mutex<ProviderCapture>>,
}
impl HostedLocalModelAdapter for CapturingProvider {
    fn offer(&self) -> &LocalModelOffer {
        self.inner.offer()
    }
    fn current_pool_health(&self) -> conduit_core::PoolRealizationHealth {
        self.inner.current_pool_health()
    }
    fn current_runtime_state(&self) -> Result<LocalModelRuntimeState, String> {
        self.inner.current_runtime_state()
    }
    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        let index = {
            let mut capture = self.capture.lock().unwrap();
            if capture.calls == 2 || input.len() > 4096 {
                output.clear();
                return LocalModelAdapterTerminal::InvalidStructuredResult;
            }
            let index = capture.calls;
            capture.requests[index].extend_from_slice(input);
            capture.calls += 1;
            index
        };
        let terminal = self.inner.execute(placement, input, output);
        self.capture.lock().unwrap().terminals[index] = Some(terminal);
        terminal
    }
    fn execute_stream(
        &mut self,
        gear: &PlannedGear,
        input: &[u8],
        sink: &mut dyn FnMut(&conduit_ai::GeneratedTextChunk) -> StreamingChunkDisposition,
    ) -> conduit_ai::GeneratedTextFlowEvidence {
        self.inner.execute_stream(gear, input, sink)
    }
    fn execute_stream_step(&mut self, gear: &PlannedGear, input: &[u8]) -> LocalModelStreamStep {
        self.inner.execute_stream_step(gear, input)
    }
    fn cancel_stream(&mut self) {
        self.inner.cancel_stream();
    }
}
pub(super) struct Capture {
    pub situations: Vec<Vec<u8>>,
    pub responses: Vec<Vec<u8>>,
    pub states: Vec<Vec<u8>>,
    pub losses: Vec<Vec<u8>>,
    progress: Arc<AtomicBool>,
}
impl BodyForeOutputAdapter for Capture {
    fn deliver(&mut self, output: &ExternalForeDelivery) -> Result<(), String> {
        let (values, maximum, bytes) = match output.front_port_id.as_str() {
            "situations" => (&mut self.situations, 2, 65_536),
            "responses" => (&mut self.responses, 2, 16_384),
            "autobiography" => (&mut self.states, 3, 65_536),
            "losses" => (&mut self.losses, 1, 34),
            name => return Err(format!("unexpected cognition Fore {name}")),
        };
        if values.len() == maximum || output.bytes.len() > bytes {
            return Err("cognition proof capture exceeded its admitted finite extent".into());
        }
        values.push(output.bytes.to_vec());
        let completed = self
            .states
            .last()
            .and_then(|bytes| ExperienceAutobiographyState::decode(bytes).ok())
            .is_some_and(|state| *state.completed() == 2);
        if completed && (self.responses.len() == 2 || !self.losses.is_empty()) {
            self.progress.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}
pub(super) struct Clock {
    start: Instant,
    pub waits: usize,
    pub after_feedback: usize,
    pub timed_out: bool,
    requested: bool,
    control: RunControl,
    progress: Arc<AtomicBool>,
}
impl TimerAdapter for Clock {
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        Some(self.start.elapsed().as_millis().min(u64::MAX as u128) as u64)
    }
    fn wait(&mut self, duration: Duration) {
        self.waits += 1;
        if self.progress.load(Ordering::SeqCst) {
            self.after_feedback += 1;
        }
        self.timed_out |= self.start.elapsed() >= Duration::from_secs(300);
        if !self.requested && (self.after_feedback >= 3 || self.timed_out) {
            self.control
                .request_stop(RunControlRequestId::new("stop-finite-live-cognition-proof").unwrap())
                .unwrap();
            self.requested = true;
        }
        if !self.requested {
            std::thread::sleep(duration.min(Duration::from_secs(1)));
        }
    }
}
pub(super) fn run(
    adapter: OllamaLocalModelAdapter,
    output: &Path,
) -> Result<Receipt, Box<dyn Error>> {
    let run_identity = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("live cognition proof needs an exact run directory identity")?;
    let model = adapter.offer().clone();
    let provider = Arc::new(Mutex::new(ProviderCapture {
        requests: std::array::from_fn(|_| Vec::with_capacity(4096)),
        terminals: [None; 2],
        calls: 0,
    }));
    let captured = CapturingProvider {
        inner: Box::new(adapter),
        capture: provider.clone(),
    };
    let initial = conduit_semantic_catalog::initial_experience_autobiography_state()
        .into_structured()
        .map_err(|error| format!("initial autobiography: {error:?}"))?;
    let mut host = StdHost::new_with_local_model_and_large_state(
        StdHostConfig {
            host_id: format!("host/pete-live/{run_identity}").into(),
            boot_id: format!("boot/pete-live/{run_identity}").into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        StdHostComposition::reference(),
        Box::new(captured),
        "ExperienceAutobiographyState",
        initial.value_type(),
        &initial,
    )?;
    let partitions = [plans::plan(&host, true)?, plans::plan(&host, false)?];
    let mut body = Body::born(
        partitions[0].source_document_id.clone(),
        partitions[0].checked_plot_id.clone(),
        1,
        format!("sign/live-cognition-born/{run_identity}").into(),
    )
    .map_err(|e| format!("birth live proof Body: {e:?}"))?;
    body = body
        .admit_plot(
            ResidentPlot::new(
                partitions[1].source_document_id.clone(),
                partitions[1].checked_plot_id.clone(),
            ),
            format!("sign/live-cognition-clock-admitted/{run_identity}").into(),
        )
        .map_err(|e| format!("admit live proof clock: {e:?}"))?;
    let wake = body
        .wake(1, format!("sign/live-cognition-wake/{run_identity}").into())
        .map_err(|e| format!("wake live proof Body: {e:?}"))?
        .1;
    let plan = BodyPlan::seal(
        &wake,
        partitions
            .into_iter()
            .map(|plan| BodyPlotPlan {
                plot: ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan,
            })
            .collect(),
    )
    .map_err(|e| format!("seal live proof Body Plan: {e:?}"))?;
    let inputs = plans::inputs()?;
    evidence::write_preparation(output, &plan, &inputs, &model)?;
    let progress = Arc::new(AtomicBool::new(false));
    let control = RunControl::default();
    let mut capture = Capture {
        situations: Vec::with_capacity(2),
        responses: Vec::with_capacity(2),
        states: Vec::with_capacity(3),
        losses: Vec::with_capacity(1),
        progress: progress.clone(),
    };
    let mut clock = Clock {
        start: Instant::now(),
        waits: 0,
        after_feedback: 0,
        timed_out: false,
        requested: false,
        control: control.clone(),
        progress,
    };
    let mut clock_output = Vec::new();
    let result = host.run_body_plan_with_fore_to(
        BodyRunRequest {
            wake: &wake,
            plan: &plan,
            control: &control,
            keyboard: None,
        },
        &inputs,
        true,
        &mut capture,
        &mut clock_output,
        &mut clock,
    );
    let report = result.as_ref().ok();
    let failure = result
        .as_ref()
        .err()
        .cloned()
        .or_else(|| report.and_then(|r| r.failure.clone()));
    let provider = provider
        .lock()
        .map_err(|_| "cognition provider capture poisoned")?;
    let criteria = verify(&plan, report, &capture, &provider, &clock, &clock_output);
    let receipt = evidence::write_run(
        output,
        &plan,
        report,
        &model,
        &capture,
        &provider,
        &clock,
        &clock_output,
        failure,
        criteria,
    )?;
    if !receipt.accepted {
        return Err(format!(
            "live cognition proof did not meet acceptance; retained {}/receipt.json",
            output.display()
        )
        .into());
    }
    Ok(receipt)
}
fn verify(
    plan: &BodyPlan,
    report: Option<&conduit_std_host::body_execution::BodyRunReport>,
    capture: &Capture,
    provider: &ProviderCapture,
    clock: &Clock,
    clock_output: &[u8],
) -> Vec<evidence::Criterion> {
    let mut criteria = Vec::with_capacity(12);
    let mut check = |name: &str, passed| {
        criteria.push(evidence::Criterion {
            name: name.into(),
            passed,
        })
    };
    check(
        "actual sealed Body/Wake/Play",
        report.is_some_and(|report| {
            report.play.validate_for(plan)
                && report.wake_at_start.body_id == plan.body_id
                && report.wake_at_start.wake_id == plan.wake_id
                && matches!(
                    report.terminal,
                    conduit_core::TerminalDisposition::Cancelled { .. }
                )
                && report.partitions.len() == 2
                && report.failure.is_none()
                && report.cleanup_failure.is_none()
                && report.kernel_failure.is_none()
        }),
    );
    check(
        "two actual model calls and responses",
        provider.calls == 2 && capture.responses.len() == 2,
    );
    check("no provider-loss outcome", capture.losses.is_empty());
    check(
        "finite proof clock continued",
        clock.after_feedback >= 3
            && String::from_utf8_lossy(clock_output)
                .matches("Clock tick")
                .count()
                >= 3
            && !clock.timed_out,
    );
    let prompts: Vec<serde_json::Value> = provider.requests[..provider.calls]
        .iter()
        .filter_map(|bytes| serde_json::from_slice(bytes).ok())
        .collect();
    let items = |index: usize| {
        prompts
            .get(index)
            .and_then(|value| value["evidence"]["items"].as_array())
    };
    check(
        "first context contains no recalled history",
        items(0).is_some_and(|items| !items.iter().any(|item| item["origin"] == "remembered")),
    );
    let memory =
        items(1).and_then(|items| items.iter().find(|item| item["origin"] == "remembered"));
    check(
        "second context recalls episode one as historical",
        memory.is_some_and(|item| {
            item["temporal_role"] == "historical"
                && item["sources"]
                    .to_string()
                    .contains("autobiography/episode/1")
        }),
    );
    let original =
        items(0).and_then(|items| items.iter().find(|item| item["origin"] == "human_reported"));
    check(
        "original human source and time remain explicit",
        original.zip(memory).is_some_and(|(original, memory)| {
            original["sources"].as_array().is_some_and(|sources| {
                sources.iter().all(|source| {
                    memory["sources"]
                        .as_array()
                        .is_some_and(|remembered| remembered.contains(source))
                })
            })
        }),
    );
    let responses: Vec<ModelDerivedResult> = capture
        .responses
        .iter()
        .filter_map(|bytes| serde_json::from_slice(bytes).ok())
        .collect();
    check(
        "complete actual responses differ and second mentions blue (lexical check; review text)",
        responses.len() == 2
            && responses.iter().all(|response| {
                response.disposition == conduit_ai::ModelResultDisposition::Produced
            })
            && responses[0].payload != responses[1].payload
            && std::str::from_utf8(&responses[1].payload)
                .is_ok_and(|text| text.to_lowercase().contains("blue")),
    );
    let last = capture
        .states
        .last()
        .and_then(|bytes| ExperienceAutobiographyState::decode(bytes).ok());
    check(
        "two completed retained episodes",
        last.as_ref().is_some_and(|state| {
            *state.completed() == 2 && state.records().get().as_slice().len() == 2
        }),
    );
    let originals: Vec<ExperienceEvidenceRequest> = capture
        .situations
        .iter()
        .filter_map(|bytes| ExperienceEvidenceRequest::decode(bytes).ok())
        .collect();
    check(
        "retained observations and actual envelopes are byte exact",
        last.as_ref().is_some_and(|state| {
            let records = state.records().get().as_slice();
            records.len() == 2
                && originals.len() == 2
                && capture.responses.len() == 2
                && records.iter().enumerate().all(|(index, record)| {
                    record.observation() == &originals[index]
                        && conduit_plot::rust_binding::BoundedBytes::new(&capture.responses[index])
                            .and_then(|bytes| {
                                conduit_human::ExperienceAutobiographyResponse::model_derived(bytes)
                                    .ok()
                            })
                            .is_some_and(|response| record.response() == &response)
                })
        }),
    );
    criteria
}
