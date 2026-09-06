//! Native asynchronous adapter over ordinary planner and std-host control APIs.

use conduit_core::{bind_active_play, ActivePlayIdentity, HostAdvertisement, Plan};
#[cfg(test)]
use conduit_core::{BootId, HostId, OfferGeneration};
#[cfg(test)]
use conduit_std_host::StdHostComposition;
use conduit_std_host::{RunControl, RunControlRequestId, StdHost, StdHostConfig, ThreadTimer};
use patchbay_model::{admit_run, FormEditor, PatchbayRequestId, PlanDocument, PlayDocument};
mod input_plan;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

type RunResult = Result<conduit_std_host::StdRunReport, String>;
const MAX_RETAINED_PRESENTATION_BYTES: usize = 16_384;

struct ActiveRun {
    control: RunControl,
    terminal: Receiver<RunResult>,
    output: Arc<Mutex<Vec<u8>>>,
}

struct LiveOutput(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for LiveOutput {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let mut bytes = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("native presentation state is poisoned"))?;
        if bytes.len().saturating_add(buffer.len()) > MAX_RETAINED_PRESENTATION_BYTES {
            return Err(std::io::Error::other(
                "Play presentation exceeded the retained native bound",
            ));
        }
        bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct NativeControl {
    host_config: StdHostConfig,
    advertisement: HostAdvertisement,
    keyboard: Option<crate::portable_keyboard::NativeKeyboardReader>,
    host: StdHost,
    plan: Option<Plan>,
    plan_document: Option<PlanDocument>,
    play_document: Option<PlayDocument>,
    failure: Option<String>,
    presentation: Option<Vec<u8>>,
    active: Option<ActiveRun>,
    request_sequence: u64,
    actions: VecDeque<String>,
}

impl NativeControl {
    #[cfg(test)]
    pub fn new() -> Self {
        let composition = StdHostComposition::minimal()
            .with_signal()
            .with_time()
            .with_text()
            .with_state();
        Self::for_host(
            HostId::from("patchbay-native/std-realization"),
            BootId::from("patchbay-native/std-boot-1"),
            composition,
        )
    }

    #[cfg(test)]
    pub fn for_host(host_id: HostId, boot_id: BootId, composition: StdHostComposition) -> Self {
        let host_config = StdHostConfig {
            host_id,
            boot_id,
            offer_generation: OfferGeneration(1),
        };
        let host = StdHost::new_with_composition(host_config.clone(), composition);
        let advertisement = host.advertisement().clone();
        Self::from_parts(host_config, advertisement, host, None)
    }

    pub fn for_advertisement(
        advertisement: HostAdvertisement,
        keyboard: crate::portable_keyboard::NativeKeyboardReader,
    ) -> Result<Self, String> {
        let host_config = StdHostConfig {
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
        };
        let host = StdHost::from_advertisement(advertisement.clone())?;
        Ok(Self::from_parts(
            host_config,
            advertisement,
            host,
            Some(keyboard),
        ))
    }

    fn from_parts(
        host_config: StdHostConfig,
        advertisement: HostAdvertisement,
        host: StdHost,
        keyboard: Option<crate::portable_keyboard::NativeKeyboardReader>,
    ) -> Self {
        Self {
            host_config,
            advertisement,
            keyboard,
            host,
            plan: None,
            plan_document: None,
            play_document: None,
            failure: None,
            presentation: None,
            active: None,
            request_sequence: 0,
            actions: VecDeque::with_capacity(32),
        }
    }

    pub fn request_plan(&mut self, editor: &FormEditor) -> Result<(), String> {
        let identity = self.next_request("plan");
        let result = self.prepare_plan(editor, &identity);
        match &result {
            Ok(()) => {
                let plan = self.plan.as_ref().expect("accepted Plan is retained");
                self.record_action(format!(
                    "PLAN-ACTION request={identity} disposition=Accepted plan={}",
                    plan.plan_id.as_str()
                ));
            }
            Err(error) => self.record_action(format!(
                "PLAN-ACTION request={identity} disposition=Rejected reason={error}"
            )),
        }
        result
    }

    pub fn plan(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }

    pub fn plan_document(&self) -> Option<&PlanDocument> {
        self.plan_document.as_ref()
    }

    pub fn play_document(&self) -> Option<&PlayDocument> {
        self.play_document.as_ref()
    }

    pub fn planned_play_identity(&self) -> Option<ActivePlayIdentity> {
        let plan = self.plan.as_ref()?;
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == self.host_config.host_id)?;
        Some(bind_active_play(
            &plan.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            0,
        ))
    }

    fn prepare_plan(&mut self, editor: &FormEditor, identity: &str) -> Result<(), String> {
        if self.active.is_some() {
            return Err("cannot replace a Plan while its Play is active".into());
        }
        let form_name = editor.view().open_form;
        let expanded = editor
            .expand_form(&form_name)
            .map_err(|error| error.to_string())?;
        let plan = if expanded.gears.iter().any(|gear| {
            matches!(
                gear.kind_id.as_str(),
                conduit_semantic_catalog::KEYBOARD_KIND
                    | conduit_semantic_catalog::BUTTON_SOURCE_KIND
            )
        }) {
            input_plan::plan(&expanded, &self.advertisement)?
        } else {
            self.host
                .plan_expanded_local(&expanded)
                .map_err(|error| error.to_string())?
        };
        let request = PatchbayRequestId::new(identity)
            .map_err(|error| format!("Plan request identity: {error:?}"))?;
        let document = PlanDocument::from_plan(request, &plan)
            .map_err(|error| format!("Plan inspection: {error:?}"))?;
        self.plan = Some(plan);
        self.plan_document = Some(document);
        self.play_document = None;
        self.failure = None;
        self.presentation = None;
        Ok(())
    }

    pub fn run(&mut self, editor: &FormEditor) -> Result<(), String> {
        let request = self.next_request("run");
        let result = self.start_run(editor);
        match &result {
            Ok(()) => {
                let plan = self.plan.as_ref().expect("admitted Run retains its Plan");
                self.record_action(format!(
                    "RUN request={request} disposition=Accepted plan={} terminal=pending",
                    plan.plan_id.as_str()
                ));
            }
            Err(error) => self.record_action(format!(
                "RUN request={request} disposition=Rejected reason={error}"
            )),
        }
        result
    }

    fn start_run(&mut self, editor: &FormEditor) -> Result<(), String> {
        if self.active.is_some() {
            return Err("a Play is already active".into());
        }
        let plan = self
            .plan
            .clone()
            .ok_or("Run requires a current exact Plan")?;
        let source = editor
            .view()
            .checked
            .source_document_id
            .ok_or("Run requires a currently checked Form")?;
        admit_run(
            &plan,
            &source,
            std::slice::from_ref(self.host.advertisement()),
        )
        .map_err(|error| format!("Run rejected: {error:?}"))?;
        let fragment = plan
            .fragments
            .first()
            .cloned()
            .ok_or("Plan has no local fragment")?;
        let control = RunControl::default();
        let worker_control = control.clone();
        let advertisement = self.advertisement.clone();
        let mut keyboard = self.keyboard.clone();
        let (sender, terminal) = mpsc::sync_channel(1);
        let output = Arc::new(Mutex::new(Vec::with_capacity(
            MAX_RETAINED_PRESENTATION_BYTES,
        )));
        let worker_output = Arc::clone(&output);
        std::thread::spawn(move || {
            let mut host = match StdHost::from_advertisement(advertisement) {
                Ok(host) => host,
                Err(error) => {
                    let _ = sender.send(Err(error));
                    return;
                }
            };
            let mut output = LiveOutput(worker_output);
            let result = host.run_fragment_controlled_with_keyboard_to(
                fragment,
                &mut output,
                &mut ThreadTimer,
                &worker_control,
                keyboard.as_mut().map(|reader| {
                    reader as &mut dyn conduit_std_host::hosted_keyboard::HostedKeyboardAdapter
                }),
            );
            let _ = sender.send(result);
        });
        self.presentation = Some(Vec::with_capacity(MAX_RETAINED_PRESENTATION_BYTES));
        self.active = Some(ActiveRun {
            control,
            terminal,
            output,
        });
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        let identity = self.next_request("stop");
        let result = (|| {
            let request = RunControlRequestId::new(identity.clone())?;
            let active = self.active.as_ref().ok_or("Stop requires an active Play")?;
            active.control.request_stop(request).map_err(|rejected| {
                format!(
                    "Stop request {} rejected: {:?}",
                    rejected.request_id.as_str(),
                    rejected.disposition
                )
            })
        })();
        match &result {
            Ok(()) => self.record_action(format!(
                "STOP request={identity} disposition=Accepted terminal=pending"
            )),
            Err(error) => self.record_action(format!(
                "STOP request={identity} disposition=Rejected reason={error}"
            )),
        }
        result
    }

    pub fn poll(&mut self) -> Result<bool, String> {
        let Some(active) = self.active.as_ref() else {
            return Ok(false);
        };
        let live_output = active
            .output
            .lock()
            .map_err(|_| "native presentation state is poisoned")?;
        let presentation = self
            .presentation
            .as_mut()
            .expect("an active Play has retained presentation storage");
        let changed = presentation.as_slice() != live_output.as_slice();
        if changed {
            presentation.clear();
            presentation.extend_from_slice(&live_output);
        }
        drop(live_output);
        match active.terminal.try_recv() {
            Ok(Ok(report)) => {
                let plan = self
                    .plan
                    .as_ref()
                    .ok_or("completed Play lost its exact Plan")?;
                let kernel = report
                    .kernel
                    .as_ref()
                    .ok_or("completed Play omitted its kernel report")?;
                let execution = patchbay_model::PlayExecutionProjection {
                    active_play_id: kernel.active_play_id.clone(),
                    decisions: kernel.decisions,
                    kernel_events: kernel.kernel_events,
                    kernel_sign: kernel.kernel_sign.clone(),
                    observations: report.observations.clone(),
                    control_receipts: report
                        .control_receipts
                        .iter()
                        .map(|receipt| patchbay_model::ControlReceiptProjection {
                            request_id: receipt.request_id.as_str().into(),
                            disposition: format!("{:?}", receipt.disposition),
                            active_play_id: receipt.active_play_id.clone(),
                        })
                        .collect(),
                };
                self.play_document = Some(
                    PlayDocument::from_execution(plan, &execution)
                        .map_err(|error| format!("Play inspection: {error:?}"))?,
                );
                let terminal = self
                    .play_document
                    .as_ref()
                    .expect("Play document exists")
                    .terminal;
                self.record_action(format!(
                    "RUN-TERMINAL active={} plan={} terminal={terminal:?}",
                    kernel.active_play_id.as_str(),
                    plan.plan_id.as_str()
                ));
                self.active = None;
                Ok(true)
            }
            Ok(Err(error)) => {
                self.failure = Some(error);
                self.active = None;
                Ok(true)
            }
            Err(TryRecvError::Empty) => Ok(changed),
            Err(TryRecvError::Disconnected) => {
                self.active = None;
                Err("Play worker ended without a terminal report".into())
            }
        }
    }

    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(text) = self.presented_text() {
            lines.push(format!("PRESENTED TEXT {text}"));
        }
        if let Some(document) = &self.plan_document {
            lines.extend(document.lines.clone());
        }
        if self.active.is_some() {
            lines.push("PLAY active terminal=pending".into());
        }
        if let Some(document) = &self.play_document {
            lines.extend(document.lines.clone());
        }
        if let Some(error) = &self.failure {
            lines.push(format!("PLAY terminal=Failed error={error}"));
        }
        lines.extend(self.actions.iter().cloned());
        lines
    }

    pub fn is_running(&self) -> bool {
        self.active.is_some()
    }

    pub fn play_terminal(&self) -> Option<conduit_core::TerminalDisposition> {
        self.play_document
            .as_ref()
            .map(|document| document.terminal)
    }

    pub fn play_failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub fn presentation(&self) -> Option<&[u8]> {
        self.presentation.as_deref()
    }

    pub fn presented_text(&self) -> Option<String> {
        let output = std::str::from_utf8(self.presentation.as_deref()?).ok()?;
        let mut presented = String::with_capacity(
            conduit_semantic_catalog::MAX_TEXT_VALUES as usize
                * conduit_text::MAX_TEXT_BYTES as usize,
        );
        let mut decoded = Vec::with_capacity(conduit_text::MAX_TEXT_BYTES as usize);
        let mut found = false;
        for line in output.lines() {
            let Some(receipt) = line.strip_prefix("PRESENTATION-TEXT bytes=") else {
                continue;
            };
            let (length, encoded) = receipt.split_once(" hex=")?;
            let length = length.parse::<usize>().ok()?;
            if encoded.len() != length.checked_mul(2)? {
                return None;
            }
            decoded.clear();
            for pair in encoded.as_bytes().as_chunks::<2>().0 {
                let high = hex_value(pair[0])?;
                let low = hex_value(pair[1])?;
                decoded.push((high << 4) | low);
            }
            presented.push_str(std::str::from_utf8(&decoded).ok()?);
            found = true;
        }
        found.then_some(presented)
    }

    #[cfg(test)]
    pub fn host_identity(&self) -> (&HostId, &BootId) {
        (&self.host_config.host_id, &self.host_config.boot_id)
    }

    fn next_request(&mut self, action: &str) -> String {
        let sequence = self.request_sequence;
        self.request_sequence = self.request_sequence.saturating_add(1);
        format!("patchbay/{action}/{sequence}")
    }

    fn record_action(&mut self, line: String) {
        if self.actions.len() == 32 {
            self.actions.pop_front();
        }
        self.actions.push_back(line);
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
