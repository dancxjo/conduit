//! One finite typed Fore queue shared by external action producers and a Body Play.
//! Producers do not own a scheduler or reconstruct Plot state.

use crate::RunControl;
use conduit_core::{
    verify_plan, ConnectionTrack, Plan, PlanId, PlannedActivationEntry, PlannedForePort,
    PortDirection, PortTemporal,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const MAX_LIVE_FORE_SLOTS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyLiveForeAdmission {
    /// Accepted into finite Host staging, before kernel ingress or Todo state.
    Accepted {
        sequence: u64,
    },
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyLiveForeStatus {
    pub queue_accepted: u16,
    pub kernel_admitted: u16,
    pub queued: u16,
    pub close_requested: bool,
    pub kernel_closed: bool,
    pub play_terminal: bool,
}

#[derive(Debug, Clone)]
pub struct BodyLiveForeQueue {
    plan_id: PlanId,
    port: PlannedForePort,
    maximum_items: u16,
    control: RunControl,
    initial: Option<(PlannedForePort, Vec<u8>)>,
    wait_timeout: Option<Duration>,
    state: Arc<Mutex<QueueState>>,
}

#[derive(Debug)]
struct QueueState {
    slots: Vec<Vec<u8>>,
    free: Vec<usize>,
    queued: VecDeque<usize>,
    submitted: u16,
    kernel_admitted: u16,
    close_requested: bool,
    kernel_closed: bool,
    play_terminal: bool,
    play_started: bool,
    started_at: Option<Instant>,
}

impl BodyLiveForeQueue {
    /// Admit a queue for the exact sealed local Todo Flow before Play. Its
    /// finite slots are allocated here, never while advancing the scheduler.
    pub fn for_todo_plan(
        plan: &Plan,
        control: RunControl,
        queue_slots: usize,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 || plan.activations.len() != 1 {
            return Err("live Todo Fore requires one complete sealed local scan Plan".into());
        }
        let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
            return Err("live Todo Fore requires a scan activation".into());
        };
        let maximum_items = scan.limits.maximum_items;
        if maximum_items == 0
            || maximum_items > 64
            || queue_slots == 0
            || queue_slots > MAX_LIVE_FORE_SLOTS
            || queue_slots > usize::from(maximum_items)
        {
            return Err("live Todo Fore queue exceeds its admitted bounds".into());
        }
        let inputs = plan.fragments[0]
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        let Some(port) = inputs.first().filter(|_| inputs.len() == 1) else {
            return Err("live Todo Fore requires one exact input port".into());
        };
        if port.track != ConnectionTrack::Payload
            || !matches!(port.temporal, PortTemporal::Flow { closes: true })
            || port.selected_line.is_some()
            || port.abnormal_kind.is_some()
            || port.byte_capacity == 0
            || port.byte_capacity > 75
        {
            return Err("live Todo Fore requires a local bounded Value Flow".into());
        }
        let mut slots = Vec::with_capacity(queue_slots);
        let mut free = Vec::with_capacity(queue_slots);
        for index in 0..queue_slots {
            slots.push(Vec::with_capacity(port.byte_capacity as usize));
            free.push(index);
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            port: (*port).clone(),
            maximum_items,
            control,
            initial: None,
            wait_timeout: None,
            state: Arc::new(Mutex::new(QueueState {
                slots,
                free,
                queued: VecDeque::with_capacity(queue_slots),
                submitted: 0,
                kernel_admitted: 0,
                close_requested: false,
                kernel_closed: false,
                play_terminal: false,
                play_started: false,
                started_at: None,
            })),
        })
    }

    /// Queue typed controls for one finite ordinary Thermostat Plot Play.
    pub fn for_thermostat_plan(
        plan: &Plan,
        control: RunControl,
        queue_slots: usize,
    ) -> Result<Self, String> {
        if !verify_plan(plan) || plan.fragments.len() != 1 || plan.activations.len() != 1 {
            return Err("live Thermostat Fore requires one complete sealed local scan Plan".into());
        }
        let PlannedActivationEntry::Scan(scan) = &plan.activations[0] else {
            return Err("live Thermostat Fore requires a scan activation".into());
        };
        let owner = plan.fragments[0]
            .placements
            .iter()
            .find(|p| p.placement_id == scan.owner_placement_id)
            .ok_or("Thermostat scan has no owner")?;
        crate::flow_activation::validate_planned_thermostat_scan(owner, scan)?;
        crate::flow_activation::maximum_scan_child_steps(scan)?;
        let maximum_items = scan.limits.maximum_items;
        if maximum_items == 0
            || maximum_items > 256
            || queue_slots == 0
            || queue_slots > MAX_LIVE_FORE_SLOTS
            || queue_slots > usize::from(maximum_items)
        {
            return Err("live Thermostat Fore queue exceeds its admitted bounds".into());
        }
        let inputs = plan.fragments[0]
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        let Some(port) = inputs.first().filter(|_| inputs.len() == 1) else {
            return Err("live Thermostat Fore requires one exact input port".into());
        };
        if port.track != ConnectionTrack::Payload
            || !matches!(port.temporal, PortTemporal::Flow { closes: true })
            || port.selected_line.is_some()
            || port.abnormal_kind.is_some()
            || port.byte_capacity == 0
            || port.byte_capacity != conduit_thermostat_plot::COMMAND_BYTES as u32
            || port.value_kind.as_str() != conduit_thermostat_plot::COMMAND_KIND
        {
            return Err("live Thermostat Fore requires a local bounded Value Flow".into());
        }
        let mut slots = Vec::with_capacity(queue_slots);
        let mut free = Vec::with_capacity(queue_slots);
        for index in 0..queue_slots {
            slots.push(Vec::with_capacity(port.byte_capacity as usize));
            free.push(index);
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            port: (*port).clone(),
            maximum_items,
            control,
            initial: None,
            wait_timeout: None,
            state: Arc::new(Mutex::new(QueueState {
                slots,
                free,
                queued: VecDeque::with_capacity(queue_slots),
                submitted: 0,
                kernel_admitted: 0,
                close_requested: false,
                kernel_closed: false,
                play_terminal: false,
                play_started: false,
                started_at: None,
            })),
        })
    }

    /// One selected checkpoint Play: current is fixed before start, while its
    /// sole command may arrive after the Play has been durably announced.
    pub fn for_todo_checkpoint_plan(
        plan: &Plan,
        control: RunControl,
        current: Vec<u8>,
        wait_timeout: Duration,
    ) -> Result<Self, String> {
        if wait_timeout.is_zero() || wait_timeout > Duration::from_secs(300) {
            return Err("waiting checkpoint timeout must be within 1ns..=300s".into());
        }
        if !verify_plan(plan) || plan.fragments.len() != 1 || !plan.activations.is_empty() {
            return Err("waiting checkpoint requires one ordinary sealed local Plan".into());
        }
        let inputs = plan.fragments[0]
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
            .collect::<Vec<_>>();
        if inputs.len() != 2 {
            return Err("waiting checkpoint requires current and command Fores".into());
        }
        let current_port = inputs
            .iter()
            .find(|port| port.front_port_id.as_str() == "current")
            .ok_or("waiting checkpoint has no current Fore")?;
        let command = inputs
            .iter()
            .find(|port| port.front_port_id.as_str() == "command")
            .ok_or("waiting checkpoint has no command Fore")?;
        if current_port.value_kind.as_str() != conduit_todo_plot::TODO_STATE_INFO_ID
            || command.value_kind.as_str() != conduit_todo_plot::TODO_COMMAND_INFO_ID
            || !plan.fragments[0].fore_ports.iter().any(|port| {
                port.direction == PortDirection::Output
                    && port.front_port_id.as_str() == "committed"
                    && port.value_kind.as_str() == conduit_todo_plot::TODO_STATE_INFO_ID
            })
            || !plan.fragments[0].placements.iter().any(|placement| {
                placement.kind_id.as_str() == conduit_todo_plot::TODO_CHECKPOINT_KIND
            })
        {
            return Err("waiting checkpoint requires the exact Todo Fore and publisher".into());
        }
        for port in [*current_port, *command] {
            if port.track != ConnectionTrack::Payload
                || port.selected_line.is_some()
                || port.abnormal_kind.is_some()
                || port.temporal != PortTemporal::Value
                || port.item_capacity != 1
                || port.byte_capacity == 0
            {
                return Err("waiting checkpoint requires two bounded local Values".into());
            }
        }
        if current_port.byte_capacity > conduit_todo_plot::STATE_MAX_BYTES as u32
            || command.byte_capacity > conduit_todo_plot::COMMAND_MAX_BYTES as u32
        {
            return Err("waiting checkpoint Fore byte bound exceeds Todo contract".into());
        }
        if current.len() > current_port.byte_capacity as usize
            || current_port.value_contract.as_ref().map_or_else(
                || {
                    conduit_core::validate_primitive_info(
                        current_port.value_kind.as_str(),
                        &current,
                    )
                    .is_err()
                },
                |contract| contract.validate(&current).is_err(),
            )
        {
            return Err("waiting checkpoint current violates sealed contract".into());
        }
        conduit_todo_plot::TodoState::decode_info(&current)
            .map_err(|error| format!("waiting checkpoint current: {error:?}"))?;
        let slots = vec![Vec::with_capacity(command.byte_capacity as usize)];
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            port: (*command).clone(),
            maximum_items: 1,
            control,
            initial: Some(((*current_port).clone(), current)),
            wait_timeout: Some(wait_timeout),
            state: Arc::new(Mutex::new(QueueState {
                slots,
                free: vec![0],
                queued: VecDeque::with_capacity(1),
                submitted: 0,
                kernel_admitted: 0,
                close_requested: false,
                kernel_closed: false,
                play_terminal: false,
                play_started: false,
                started_at: None,
            })),
        })
    }

    pub fn submit(&self, canonical: &[u8]) -> Result<BodyLiveForeAdmission, String> {
        if self.control.stop_requested() {
            return Err("live Todo Fore Play is cancelling".into());
        }
        if canonical.len() > self.port.byte_capacity as usize {
            return Err("live Todo Fore value exceeds its sealed byte bound".into());
        }
        if let Some(contract) = &self.port.value_contract {
            contract
                .validate(canonical)
                .map_err(|error| format!("live Todo Fore value contract: {error:?}"))?;
        } else {
            conduit_core::validate_primitive_info(self.port.value_kind.as_str(), canonical)
                .map_err(|error| format!("live Todo Fore value kind: {error:?}"))?;
        }
        if self.initial.is_some() {
            conduit_todo_plot::TodoCommand::decode_info(canonical)
                .map_err(|error| format!("waiting checkpoint command: {error:?}"))?;
        }
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if self.initial.is_some() && !state.play_started {
            return Err("waiting checkpoint command requires a started Body Play".into());
        }
        if state.close_requested || state.kernel_closed || state.play_terminal {
            return Err("live Todo Fore input is closed".into());
        }
        if state.submitted == self.maximum_items {
            return Err("live Todo Fore selected item bound is exhausted".into());
        }
        let Some(index) = state.free.pop() else {
            return Ok(BodyLiveForeAdmission::Full);
        };
        let sequence = u64::from(state.submitted);
        state.slots[index].clear();
        state.slots[index].extend_from_slice(canonical);
        state.queued.push_back(index);
        state.submitted += 1;
        drop(state);
        self.control.signal_activity();
        Ok(BodyLiveForeAdmission::Accepted { sequence })
    }

    pub fn close(&self) -> Result<(), String> {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if state.close_requested || state.play_terminal {
            return Err("live Todo Fore was already closed".into());
        }
        state.close_requested = true;
        drop(state);
        self.control.signal_activity();
        Ok(())
    }

    /// Wait for the runner's successful start callback before a checkpoint
    /// command is eligible. The caller may request cancellation separately.
    pub fn wait_until_play_started(&self) -> Result<(), String> {
        if self.initial.is_none() {
            return Err("start wait is only available for a waiting checkpoint".into());
        }
        loop {
            let observed = self.control.activity_generation();
            let state = self.state.lock().expect("live Fore queue lock poisoned");
            if state.play_started {
                return Ok(());
            }
            if state.play_terminal || self.control.stop_requested() {
                return Err("waiting checkpoint Play ended before start".into());
            }
            drop(state);
            self.control.wait_for_activity_or_stop(observed);
        }
    }

    /// Distinguishes Host staging from kernel ingress at cancellation or
    /// failure. Kernel admission is still not a committed Todo state output.
    pub fn status(&self) -> BodyLiveForeStatus {
        let state = self.state.lock().expect("live Fore queue lock poisoned");
        BodyLiveForeStatus {
            queue_accepted: state.submitted,
            kernel_admitted: state.kernel_admitted,
            queued: state.queued.len() as u16,
            close_requested: state.close_requested,
            kernel_closed: state.kernel_closed,
            play_terminal: state.play_terminal,
        }
    }

    pub(crate) fn mark_play_terminal(&self) {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        state.play_terminal = true;
        drop(state);
        self.control.signal_activity();
    }

    pub(crate) fn mark_play_started(&self) {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        state.play_started = true;
        state.started_at = Some(Instant::now());
        drop(state);
        self.control.signal_activity();
    }

    pub(crate) fn matches_plan(&self, plan: &Plan, control: &RunControl) -> bool {
        self.plan_id == plan.plan_id
            && self.control.same_source(control)
            && plan.fragments.len() == 1
            && plan.fragments[0]
                .fore_ports
                .iter()
                .any(|port| port == &self.port)
    }

    pub(crate) fn maximum_items(&self) -> u16 {
        self.maximum_items
    }

    pub(crate) fn port(&self) -> &PlannedForePort {
        &self.port
    }

    pub(crate) fn initial(&self) -> Option<(&PlannedForePort, &[u8])> {
        self.initial
            .as_ref()
            .map(|(port, bytes)| (port, bytes.as_slice()))
    }

    pub(crate) fn observed_generation(&self) -> u64 {
        self.control.activity_generation()
    }

    pub(crate) fn wait_if_empty(&self, observed: u64) -> Result<(), String> {
        let state = self.state.lock().expect("live Fore queue lock poisoned");
        if !state.queued.is_empty() || state.close_requested {
            return Ok(());
        }
        let remaining = self.wait_timeout.map(|timeout| {
            timeout.saturating_sub(state.started_at.expect("waiting Play started").elapsed())
        });
        drop(state);
        if let Some(remaining) = remaining {
            let timed_out = self
                .control
                .wait_for_activity_or_stop_for(observed, remaining);
            if timed_out {
                let state = self.state.lock().expect("live Fore queue lock poisoned");
                if state.queued.is_empty()
                    && !state.close_requested
                    && !self.control.stop_requested()
                {
                    return Err("waiting checkpoint command timed out without an effect".into());
                }
            }
        } else {
            self.control.wait_for_activity_or_stop(observed);
        }
        Ok(())
    }

    pub(crate) fn with_front<R>(&self, f: impl FnOnce(Option<(u64, &[u8])>, bool) -> R) -> R {
        let state = self.state.lock().expect("live Fore queue lock poisoned");
        let front = state.queued.front().map(|index| {
            (
                u64::from(state.kernel_admitted),
                state.slots[*index].as_slice(),
            )
        });
        f(
            front,
            state.close_requested && state.queued.is_empty() && !state.kernel_closed,
        )
    }

    pub(crate) fn acknowledge(&self, sequence: u64) -> Result<(), String> {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if sequence != u64::from(state.kernel_admitted) {
            return Err("live Todo Fore acceptance sequence changed".into());
        }
        let index = state
            .queued
            .pop_front()
            .ok_or("live Todo Fore accepted an absent value")?;
        state.free.push(index);
        state.kernel_admitted += 1;
        Ok(())
    }

    pub(crate) fn mark_kernel_closed(&self) -> Result<(), String> {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if !state.close_requested || !state.queued.is_empty() || state.kernel_closed {
            return Err("live Todo Fore close changed during delivery".into());
        }
        state.kernel_closed = true;
        Ok(())
    }

    pub(crate) fn mark_checkpoint_kernel_closed(&self) -> Result<(), String> {
        let mut state = self.state.lock().expect("live Fore queue lock poisoned");
        if self.initial.is_none() || state.kernel_admitted != 1 || state.kernel_closed {
            return Err("waiting checkpoint input close changed during delivery".into());
        }
        state.close_requested = true;
        state.kernel_closed = true;
        Ok(())
    }
}
