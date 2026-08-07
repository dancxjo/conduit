use std::thread;
use std::time::Duration;

use conduit_core::{
    bind_active_play, bind_evidence, bind_presentation, BootId, ConnectionProvider, EvidenceId,
    PlacementId, PlanFragment, PresentationId,
};
use conduit_kernel::scheduler::{
    FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    CordId, EvidenceQuery, FixedHostOperationBindings, FixedRoutes, HostOperationDisposition,
    HostOperationOutcome, HostedEvidenceLog, HostedValueStore, KernelEventKind, NodeId,
    RemoteEndpointId, ValueStorage,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, KernelExecutionIdentityMap, LoweredPlanFragment, RemoteCordDirection,
    MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_signal::{
    decode_signal_bytes, encode_signal, parse_pulse_configuration, triple, Signal, PULSE_KIND,
    SHOW_KIND, SIGNAL_ENCODED_LEN,
};
use conduit_wire::{SessionBinding, SessionFrame, SessionMachine, SessionRole};

use super::operation::TripleOperation;

#[path = "prepare.rs"]
mod prepare;

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const VALUES: usize = 16;
const WAITS: usize = VALUES - 1;
const STORED_ITEMS: u16 = (VALUES + WAITS) as u16;
const STORED_BYTES: u32 = VALUES as u32 * SIGNAL_ENCODED_LEN + WAITS as u32 * 8;
const EVIDENCE_ITEMS: u16 = 512;

type TripleScheduler = FixedScheduler<
    OperationDriver<TripleOperation, PORTS>,
    HostedValueStore,
    HostedEvidenceLog,
    2,
    3,
    PORTS,
    3,
    { 2 * PORTS },
    3,
    2,
    2,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteKind {
    Browser,
    Pico,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TripleOffer {
    pub sequence: u64,
    pub payload: [u8; SIGNAL_ENCODED_LEN as usize],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdoutReceipt {
    pub plan_id: String,
    pub fragment_id: String,
    pub active_play_id: String,
    pub placement_id: PlacementId,
    pub presentation_id: PresentationId,
    pub evidence_id: EvidenceId,
    pub sequence: u64,
    pub level: bool,
}

struct RemoteBranch {
    endpoint: RemoteEndpointId,
    cord: CordId,
    binding: SessionBinding,
    session: SessionMachine,
    pressure_retries: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CapacitySeal {
    values: (usize, usize),
    evidence: usize,
    drivers: usize,
    identity: (usize, usize, usize),
    receipts: usize,
}

pub struct TripleSource {
    scheduler: TripleScheduler,
    fragment: PlanFragment,
    lowered: LoweredPlanFragment,
    identity: KernelExecutionIdentityMap,
    pulse_node: NodeId,
    show_node: NodeId,
    show_placement: PlacementId,
    active_play_id: conduit_core::ActivePlayId,
    browser: RemoteBranch,
    pico: RemoteBranch,
    receipts: Vec<StdoutReceipt>,
    seal: CapacitySeal,
}

impl TripleSource {
    pub fn fragment(&self) -> &PlanFragment {
        &self.fragment
    }

    pub fn binding(&self, kind: RemoteKind) -> &SessionBinding {
        &self.branch(kind).binding
    }

    pub fn observe_pico_boot(&mut self, boot: BootId) -> Result<(), String> {
        let binding = self
            .pico
            .binding
            .clone()
            .with_observed_boots(self.pico.binding.source.boot_id.clone(), boot)
            .map_err(|error| format!("{error:?}"))?;
        self.pico.session = SessionMachine::new(binding.clone(), SessionRole::Source)
            .map_err(|error| format!("{error:?}"))?;
        self.pico.binding = binding;
        Ok(())
    }

    pub fn admit_outbound(
        &mut self,
        kind: RemoteKind,
        frame: SessionFrame<'_>,
    ) -> Result<(), String> {
        self.branch_mut(kind)
            .session
            .admit_outbound(frame)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn admit_inbound(
        &mut self,
        kind: RemoteKind,
        frame: SessionFrame<'_>,
    ) -> Result<(), String> {
        self.branch_mut(kind)
            .session
            .admit_inbound(frame)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn is_active(&self, kind: RemoteKind) -> bool {
        self.branch(kind).session.is_active()
    }

    pub fn is_terminal(&self, kind: RemoteKind) -> bool {
        self.branch(kind).session.is_terminal()
    }

    pub fn next_sequence(&self, kind: RemoteKind) -> u64 {
        self.branch(kind).session.next_sequence()
    }

    pub fn next_offer(&mut self) -> Result<Option<TripleOffer>, String> {
        loop {
            let browser = self.remote_offer(RemoteKind::Browser)?;
            let pico = self.remote_offer(RemoteKind::Pico)?;
            match (browser, pico) {
                (
                    Some((browser_sequence, browser_payload)),
                    Some((pico_sequence, pico_payload)),
                ) => {
                    if browser_sequence != pico_sequence || browser_payload != pico_payload {
                        return Err("atomic fan-out branches disagree".to_owned());
                    }
                    return Ok(Some(TripleOffer {
                        sequence: browser_sequence,
                        payload: browser_payload,
                    }));
                }
                (Some(_), None) | (None, Some(_)) => {
                    return Err("only one remote branch received an atomic emission".to_owned())
                }
                (None, None) => {}
            }
            if let Some(request) = self.scheduler.next_host_request() {
                self.complete_host_operation(request)?;
                continue;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => return Ok(None),
                SchedulerStatus::Idle => return Err("triple source became idle early".to_owned()),
                SchedulerStatus::Cancelled => return Err("triple source cancelled".to_owned()),
            }
        }
    }

    pub fn pressure(&mut self, kind: RemoteKind, sequence: u64) -> Result<(), String> {
        let branch = self.branch_mut(kind);
        if sequence != branch.session.next_sequence() {
            return Err("pressure sequence disagrees with session".to_owned());
        }
        branch.pressure_retries = branch.pressure_retries.saturating_add(1);
        Ok(())
    }

    pub fn accepted(&mut self, kind: RemoteKind, sequence: u64) -> Result<(), String> {
        let branch = self.branch(kind);
        self.scheduler
            .remote_egress_accept(branch.endpoint, branch.cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn delivered(&mut self, kind: RemoteKind, sequence: u64) -> Result<(), String> {
        let branch = self.branch(kind);
        self.scheduler
            .remote_egress_delivered(branch.endpoint, branch.cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn receipts(&self) -> &[StdoutReceipt] {
        &self.receipts
    }

    pub fn manifest_stdout(&mut self, sequence: u64) -> Result<StdoutReceipt, String> {
        let index = usize::try_from(sequence).map_err(|_| "stdout sequence width".to_owned())?;
        loop {
            if let Some(receipt) = self.receipts.get(index) {
                return Ok(receipt.clone());
            }
            if let Some(request) = self.scheduler.next_host_request() {
                self.complete_host_operation(request)?;
                continue;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Complete => {
                    return Err("kernel completed before stdout manifested".to_owned())
                }
                SchedulerStatus::Idle => return Err("stdout branch became idle".to_owned()),
                SchedulerStatus::Cancelled => return Err("stdout branch cancelled".to_owned()),
            }
        }
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        self.scheduler
            .cancel()
            .map_err(|error| format!("{error:?}"))
    }

    pub fn finish_kernel(&mut self) -> Result<u64, String> {
        while let Some(request) = self.scheduler.next_host_request() {
            self.complete_host_operation(request)?;
        }
        loop {
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {
                    while let Some(request) = self.scheduler.next_host_request() {
                        self.complete_host_operation(request)?;
                    }
                }
                SchedulerStatus::Complete => break,
                SchedulerStatus::Idle => {
                    return Err("triple kernel idle before terminal".to_owned())
                }
                SchedulerStatus::Cancelled => return Err("triple kernel cancelled".to_owned()),
            }
        }
        for kind in [RemoteKind::Browser, RemoteKind::Pico] {
            let branch = self.branch(kind);
            if !self
                .scheduler
                .remote_egress_terminal(branch.endpoint, branch.cord)
                .map_err(|error| format!("{error:?}"))?
                || self
                    .scheduler
                    .cord_usage(branch.cord)
                    .map_err(|error| format!("{error:?}"))?
                    != (0, 0)
            {
                return Err("triple remote branch not terminal".to_owned());
            }
        }
        if self.receipts.len() != VALUES
            || self.scheduler.values().used_items() != 0
            || !self
                .scheduler
                .evidence()
                .contains_kind(KernelEventKind::RemoteValueDelivered)
            || !self
                .scheduler
                .evidence()
                .contains_kind(KernelEventKind::OperationCompleted)
            || self.capacity_seal() != self.seal
        {
            return Err("triple terminal/capacity invariants failed".to_owned());
        }
        Ok(VALUES as u64)
    }

    fn remote_offer(
        &mut self,
        kind: RemoteKind,
    ) -> Result<Option<(u64, [u8; SIGNAL_ENCODED_LEN as usize])>, String> {
        let (endpoint, cord) = {
            let branch = self.branch(kind);
            (branch.endpoint, branch.cord)
        };
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(endpoint, cord)
            .map_err(|error| format!("{error:?}"))?
        else {
            return Ok(None);
        };
        let payload = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("{error:?}"))?
            .try_into()
            .map_err(|_| "triple remote payload width".to_owned())?;
        Ok(Some((offer.sequence, payload)))
    }

    fn complete_host_operation(&mut self, request: HostOperationRequest) -> Result<(), String> {
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("{error:?}"))?;
        self.identity
            .bind_request(
                &self.lowered.identity,
                request.node,
                request.request,
                request.operation,
            )
            .map_err(|error| format!("{error:?}"))?;
        if request.node == self.pulse_node {
            let duration = u64::from_le_bytes(
                input
                    .try_into()
                    .map_err(|_| "triple wait input width".to_owned())?,
            );
            thread::sleep(Duration::from_millis(duration));
        } else if request.node == self.show_node {
            let signal = decode_signal_bytes(input).map_err(|error| error.to_string())?;
            let presentation =
                bind_presentation(&self.active_play_id, &self.show_placement, signal.sequence);
            let evidence = bind_evidence(
                &self.fragment.host_id,
                &self.fragment.boot_id,
                Some(&self.active_play_id),
                signal.sequence,
            );
            self.identity
                .bind_presentation(
                    &self.lowered.identity,
                    request.node,
                    request.request,
                    &presentation,
                )
                .map_err(|error| format!("{error:?}"))?;
            self.identity
                .bind_evidence(
                    &evidence,
                    Some(request.node),
                    Some(request.request),
                    Some(&presentation.presentation_id),
                )
                .map_err(|error| format!("{error:?}"))?;
            self.receipts.push(StdoutReceipt {
                plan_id: self.fragment.plan_id.as_str().to_owned(),
                fragment_id: self.fragment.fragment_id.as_str().to_owned(),
                active_play_id: self.active_play_id.as_str().to_owned(),
                placement_id: self.show_placement.clone(),
                presentation_id: presentation.presentation_id,
                evidence_id: evidence.evidence_id,
                sequence: signal.sequence,
                level: signal.level,
            });
        } else {
            return Err("host request came from an uninstalled triple node".to_owned());
        }
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|error| format!("{error:?}"))
    }

    fn branch(&self, kind: RemoteKind) -> &RemoteBranch {
        match kind {
            RemoteKind::Browser => &self.browser,
            RemoteKind::Pico => &self.pico,
        }
    }

    fn branch_mut(&mut self, kind: RemoteKind) -> &mut RemoteBranch {
        match kind {
            RemoteKind::Browser => &mut self.browser,
            RemoteKind::Pico => &mut self.pico,
        }
    }

    fn capacity_seal(&self) -> CapacitySeal {
        CapacitySeal {
            values: self.scheduler.values().allocation_capacities(),
            evidence: self.scheduler.evidence().allocation_capacity(),
            drivers: self
                .scheduler
                .drivers()
                .iter()
                .map(|driver| driver.operation().allocation_capacity())
                .sum(),
            identity: self.identity.allocation_capacities(),
            receipts: self.receipts.capacity(),
        }
    }
}

fn node_for_kind(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    kind: &str,
) -> Result<NodeId, String> {
    lowered
        .nodes
        .iter()
        .find(|node| {
            fragment.placements[usize::from(node.node.0)]
                .kind_id
                .as_str()
                == kind
        })
        .map(|node| node.node)
        .ok_or_else(|| format!("triple source has no {kind} node"))
}

fn remote_branch(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    provider: ConnectionProvider,
) -> Result<RemoteBranch, String> {
    let remote = lowered
        .remote_endpoints
        .iter()
        .find(|remote| remote.binding.provider == provider)
        .ok_or_else(|| format!("missing {provider:?} triple endpoint"))?;
    if remote.direction != RemoteCordDirection::Egress {
        return Err("triple source endpoint is not egress".to_owned());
    }
    let connection = fragment
        .connections
        .iter()
        .find(|connection| connection.connection_id == remote.connection_id)
        .ok_or_else(|| "triple planned connection missing".to_owned())?;
    let binding = SessionBinding::from_planned_connection(
        fragment.plan_id.clone(),
        remote.source_fragment_id.clone(),
        remote.sink_fragment_id.clone(),
        connection,
    )
    .map_err(|error| format!("{error:?}"))?;
    let session = SessionMachine::new(binding.clone(), SessionRole::Source)
        .map_err(|error| format!("{error:?}"))?;
    Ok(RemoteBranch {
        endpoint: remote.endpoint,
        cord: remote.cord,
        binding,
        session,
        pressure_retries: 0,
    })
}
