//! Native asynchronous adapter over ordinary planner and std-host control APIs.

use conduit_core::{BootId, HostId, OfferGeneration, Plan};
use conduit_std_host::{
    RunControl, RunControlRequestId, StdHost, StdHostComposition, StdHostConfig, ThreadTimer,
};
use patchbay_model::{admit_run, FormEditor, PatchbayRequestId, PlanDocument, PlayDocument};
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, TryRecvError};

type RunResult = Result<(conduit_std_host::StdRunReport, Vec<u8>), String>;

pub struct NativeControl {
    host_config: StdHostConfig,
    composition: StdHostComposition,
    host: StdHost,
    plan: Option<Plan>,
    plan_document: Option<PlanDocument>,
    play_document: Option<PlayDocument>,
    failure: Option<String>,
    active: Option<(RunControl, Receiver<RunResult>)>,
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

    pub fn for_host(host_id: HostId, boot_id: BootId, composition: StdHostComposition) -> Self {
        let host_config = StdHostConfig {
            host_id,
            boot_id,
            offer_generation: OfferGeneration(1),
        };
        let host = StdHost::new_with_composition(host_config.clone(), composition);
        Self {
            host_config,
            composition,
            host,
            plan: None,
            plan_document: None,
            play_document: None,
            failure: None,
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

    fn prepare_plan(&mut self, editor: &FormEditor, identity: &str) -> Result<(), String> {
        if self.active.is_some() {
            return Err("cannot replace a Plan while its Play is active".into());
        }
        let form_name = editor.view().open_form;
        let expanded = editor
            .expand_form(&form_name)
            .map_err(|error| error.to_string())?;
        let plan = self
            .host
            .plan_expanded_local(&expanded)
            .map_err(|error| error.to_string())?;
        let request = PatchbayRequestId::new(identity)
            .map_err(|error| format!("Plan request identity: {error:?}"))?;
        let document = PlanDocument::from_plan(request, &plan)
            .map_err(|error| format!("Plan inspection: {error:?}"))?;
        self.plan = Some(plan);
        self.plan_document = Some(document);
        self.play_document = None;
        self.failure = None;
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
        let config = self.host_config.clone();
        let composition = self.composition;
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut host = StdHost::new_with_composition(config, composition);
            let mut output = Vec::with_capacity(4096);
            let result = host
                .run_fragment_controlled_to(
                    fragment,
                    &mut output,
                    &mut ThreadTimer,
                    &worker_control,
                )
                .map(|report| (report, output));
            let _ = sender.send(result);
        });
        self.active = Some((control, receiver));
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        let identity = self.next_request("stop");
        let result = (|| {
            let request = RunControlRequestId::new(identity.clone())?;
            let (control, _) = self.active.as_ref().ok_or("Stop requires an active Play")?;
            control.request_stop(request).map_err(|rejected| {
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
        let Some((_, receiver)) = self.active.as_ref() else {
            return Ok(false);
        };
        match receiver.try_recv() {
            Ok(Ok((report, _output))) => {
                let plan = self
                    .plan
                    .as_ref()
                    .ok_or("completed Play lost its exact Plan")?;
                self.play_document = Some(
                    PlayDocument::from_report(plan, &report)
                        .map_err(|error| format!("Play inspection: {error:?}"))?,
                );
                let kernel = report
                    .kernel
                    .as_ref()
                    .expect("Play document required a kernel report");
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
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => {
                self.active = None;
                Err("Play worker ended without a terminal report".into())
            }
        }
    }

    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
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

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
