//! Bounded causal evidence projected from actual std-host Body execution.

use crate::body_execution::BodyRunReport;
use conduit_body::{BodyId, BodyPlan, Wake, WakeId, WakeLifecycle, WakeLifecycleEvent};
use conduit_core::{
    AuthorityBinding, BootId, GearId, HostId, ImplementationId, KindId, PlacementId, PlanId,
    ResourceBinding, SourceDocumentId, TerminalDisposition, TerminalInfo,
};
use conduit_kernel::{
    causal_evidence::{
        CausalEdge, CausalEvidence, CausalEvidenceRefusal, CausalRelationship, EvidenceIdentity,
        EvidenceOutcome, TerminalEvidenceCorrelation, TerminalEvidenceIndex,
    },
    KernelEvent, KernelEventKind,
};
use conduit_plan_lowering::lowering::KernelIdentityMap;

mod continuity;
use continuity::validate_recovery_continuity;
mod graph;
use graph::{record_intra_run_edges, record_planned_recovery_edges, record_planned_transfer_edges};
mod identity;
mod metadata;
use identity::{digest_u64, evidence_sign, execution_envelope};
mod time;
pub use time::{BodyEventTimeObservation, EventTimeCapture};

pub const MAXIMUM_BODY_CAUSAL_NODES: usize = 128;
pub const MAXIMUM_BODY_CAUSAL_EDGES: usize = 128;
pub const MAXIMUM_BODY_TERMINAL_CORRELATIONS: usize = 16;
pub const MAXIMUM_BODY_NODE_RESOURCES: usize = 16;
pub const MAXIMUM_BODY_NODE_AUTHORITIES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyCausalEvidenceRefusal {
    CapacityExceeded,
    EmptyEvidence,
    MismatchedBodyPlan,
    MismatchedExecution,
    MismatchedContinuity,
    MismatchedIdentityMap,
    NoUnresolvedSemanticAbnormal,
    NotFailed,
    Causal(CausalEvidenceRefusal),
    UnknownClockEvent,
    ConflictingClockObservation,
    InvalidClockObservation,
}

impl From<CausalEvidenceRefusal> for BodyCausalEvidenceRefusal {
    fn from(value: CausalEvidenceRefusal) -> Self {
        Self::Causal(value)
    }
}

#[derive(Debug, Clone)]
struct BodyCausalNode {
    evidence: EvidenceIdentity,
    outcome: EvidenceOutcome,
    source_document_id: SourceDocumentId,
    source_span: Option<conduit_core::SourceSpan>,
    body_id: BodyId,
    wake_id: WakeId,
    plan_id: PlanId,
    play_id: conduit_core::ActivePlayId,
    placement_id: PlacementId,
    gear_id: GearId,
    kind_id: KindId,
    implementation_id: ImplementationId,
    host_id: HostId,
    boot_id: BootId,
    resources: Vec<ResourceBinding>,
    authority: Vec<AuthorityBinding>,
    kernel_kind: KernelEventKind,
    kernel_port: Option<conduit_kernel::PortId>,
    kernel_sequence: u32,
    semantic_terminal: bool,
    observed_time: Option<BodyEventTimeObservation>,
}

/// Exact bounded evidence for one run or one successful replacement pair.
pub struct BodyRunCausalRecord {
    graph: CausalEvidence<MAXIMUM_BODY_CAUSAL_EDGES>,
    terminals: TerminalEvidenceIndex<MAXIMUM_BODY_TERMINAL_CORRELATIONS>,
    nodes: Vec<BodyCausalNode>,
}

impl BodyRunCausalRecord {
    pub fn from_run(
        plan: &BodyPlan,
        report: &BodyRunReport,
    ) -> Result<Self, BodyCausalEvidenceRefusal> {
        let nodes = collect_nodes(plan, report)?;
        if nodes.is_empty() {
            return Err(BodyCausalEvidenceRefusal::EmptyEvidence);
        }
        let mut graph = CausalEvidence::default();
        record_intra_run_edges(&mut graph, &nodes)?;
        record_planned_transfer_edges(&mut graph, &nodes, plan, &report.partitions)?;
        record_planned_recovery_edges(&mut graph, &nodes, plan, &report.partitions)?;
        Ok(Self {
            graph,
            terminals: TerminalEvidenceIndex::default(),
            nodes,
        })
    }

    /// Retains a failed/unsatisfied realization and its completed replacement
    /// as recovery history. This constructor deliberately has no terminal
    /// correlation input and therefore cannot manufacture semantic failure.
    pub fn successful_recovery(
        continuity: &Wake,
        prior_plan: &BodyPlan,
        prior_report: &BodyRunReport,
        replacement_plan: &BodyPlan,
        replacement_report: &BodyRunReport,
    ) -> Result<Self, BodyCausalEvidenceRefusal> {
        if !matches!(prior_report.terminal, TerminalDisposition::Failed { .. })
            || replacement_report.terminal != TerminalDisposition::Completed
            || prior_plan.body_id != replacement_plan.body_id
            || prior_plan.wake_id != replacement_plan.wake_id
            || prior_plan.plan_id == replacement_plan.plan_id
        {
            return Err(BodyCausalEvidenceRefusal::MismatchedExecution);
        }
        validate_recovery_continuity(
            continuity,
            prior_plan,
            prior_report,
            replacement_plan,
            replacement_report,
        )?;
        let mut prior = collect_nodes(prior_plan, prior_report)?;
        let mut replacement = collect_nodes(replacement_plan, replacement_report)?;
        if prior.len() + replacement.len() > MAXIMUM_BODY_CAUSAL_NODES {
            return Err(BodyCausalEvidenceRefusal::CapacityExceeded);
        }
        let cause = prior
            .iter()
            .rev()
            .find(|node| is_abnormal(node.kernel_kind))
            .map(|node| node.evidence)
            .ok_or(BodyCausalEvidenceRefusal::NoUnresolvedSemanticAbnormal)?;
        let recovered = replacement
            .iter_mut()
            .rev()
            .find(|node| is_completion(node.kernel_kind))
            .ok_or(BodyCausalEvidenceRefusal::EmptyEvidence)?;
        recovered.outcome = EvidenceOutcome::Recovered;
        let effect = recovered.evidence;

        let mut graph = CausalEvidence::default();
        record_intra_run_edges(&mut graph, &prior)?;
        record_planned_transfer_edges(&mut graph, &prior, prior_plan, &prior_report.partitions)?;
        record_planned_recovery_edges(&mut graph, &prior, prior_plan, &prior_report.partitions)?;
        record_intra_run_edges(&mut graph, &replacement)?;
        record_planned_transfer_edges(
            &mut graph,
            &replacement,
            replacement_plan,
            &replacement_report.partitions,
        )?;
        record_planned_recovery_edges(
            &mut graph,
            &replacement,
            replacement_plan,
            &replacement_report.partitions,
        )?;
        graph.record(CausalEdge {
            effect,
            relationship: CausalRelationship::Corrects,
            cause,
        })?;
        prior.append(&mut replacement);
        Ok(Self {
            graph,
            terminals: TerminalEvidenceIndex::default(),
            nodes: prior,
        })
    }

    /// Correlates a real portable abnormal terminal only when this exact run
    /// failed and its last semantic abnormal was not subsequently recovered.
    pub fn correlate_unresolved_semantic_abnormal(
        &mut self,
        wake: &Wake,
        report: &BodyRunReport,
        terminal: TerminalInfo,
    ) -> Result<EvidenceIdentity, BodyCausalEvidenceRefusal> {
        if !matches!(report.terminal, TerminalDisposition::Failed { .. }) {
            return Err(BodyCausalEvidenceRefusal::NotFailed);
        }
        if wake.validate().is_err()
            || wake.lifecycle != WakeLifecycle::Failed
            || wake.body_id != report.play.body_id
            || wake.wake_id != report.play.wake_id
            || !wake.events.iter().any(|event| {
                matches!(
                    event,
                    WakeLifecycleEvent::Failed { sign_id }
                        if sign_id == &report.terminal_sign.sign_id
                )
            })
        {
            return Err(BodyCausalEvidenceRefusal::MismatchedContinuity);
        }
        validate_report_identity_against_nodes(report, &self.nodes)?;
        let node_index = self
            .nodes
            .iter()
            .enumerate()
            .rev()
            .find(|(_, node)| {
                node.play_id == report.play.active_play_id
                    && node.kernel_kind == KernelEventKind::SemanticAbnormal
                    && !self.nodes.iter().any(|later| {
                        later.play_id == node.play_id
                            && later.placement_id == node.placement_id
                            && later.kernel_sequence > node.kernel_sequence
                            && later.kernel_kind == KernelEventKind::SemanticAbnormalRecovered
                    })
            })
            .map(|(index, _)| index)
            .ok_or(BodyCausalEvidenceRefusal::NoUnresolvedSemanticAbnormal)?;
        let evidence = self.nodes[node_index].evidence;
        self.terminals.record(TerminalEvidenceCorrelation {
            cause_digest: terminal.cause_digest(),
            terminal: evidence,
        })?;
        self.nodes[node_index].semantic_terminal = true;
        Ok(evidence)
    }

    pub const fn graph(&self) -> &CausalEvidence<MAXIMUM_BODY_CAUSAL_EDGES> {
        &self.graph
    }

    pub const fn terminals(&self) -> &TerminalEvidenceIndex<MAXIMUM_BODY_TERMINAL_CORRELATIONS> {
        &self.terminals
    }

    /// Enumerates the exact identities retained by this bounded record.
    ///
    /// Successful recovery deliberately has no semantic-terminal correlation,
    /// so inspection enters through this finite history rather than inventing
    /// a terminal root merely to make the evidence discoverable.
    pub fn retained_evidence(&self) -> impl ExactSizeIterator<Item = EvidenceIdentity> + '_ {
        self.nodes.iter().map(|node| node.evidence)
    }
}

fn collect_nodes(
    plan: &BodyPlan,
    report: &BodyRunReport,
) -> Result<Vec<BodyCausalNode>, BodyCausalEvidenceRefusal> {
    validate_plan_report(plan, report)?;
    let relevant = report
        .kernel_events
        .iter()
        .filter(|event| is_relevant(event.kind))
        .collect::<Vec<_>>();
    if relevant.len() > MAXIMUM_BODY_CAUSAL_NODES {
        return Err(BodyCausalEvidenceRefusal::CapacityExceeded);
    }
    let envelope = execution_envelope(plan);
    let mut nodes = Vec::with_capacity(relevant.len());
    for event in relevant {
        let (fragment, placement) = resolve_event(plan, &report.partitions, event)?;
        let observed_time = report
            .clock_observations
            .iter()
            .find(|observation| observation.sequence == event.sequence)
            .map(|observation| {
                observation.time.validate()?;
                if observation.time.local().clock().host_id() != &placement.host_id
                    || observation.time.local().clock().boot_id() != &placement.boot_id
                {
                    return Err(BodyCausalEvidenceRefusal::InvalidClockObservation);
                }
                Ok(observation.time.clone())
            })
            .transpose()?;
        if placement.resources.len() > MAXIMUM_BODY_NODE_RESOURCES
            || placement.authority.len() > MAXIMUM_BODY_NODE_AUTHORITIES
        {
            return Err(BodyCausalEvidenceRefusal::CapacityExceeded);
        }
        nodes.push(BodyCausalNode {
            evidence: EvidenceIdentity {
                sign: evidence_sign(&report.terminal_sign.sign_id, event.sequence),
                execution: envelope,
                host_session: digest_u64(&[placement.host_id.as_str(), placement.boot_id.as_str()]),
            },
            outcome: outcome(event.kind),
            source_document_id: fragment.source_document_id.clone(),
            source_span: placement.source_span,
            body_id: plan.body_id.clone(),
            wake_id: plan.wake_id.clone(),
            plan_id: plan.plan_id.clone(),
            play_id: report.play.active_play_id.clone(),
            placement_id: placement.placement_id.clone(),
            gear_id: placement.gear_id.clone(),
            kind_id: placement.kind_id.clone(),
            implementation_id: placement.implementation_id.clone(),
            host_id: placement.host_id.clone(),
            boot_id: placement.boot_id.clone(),
            resources: placement.resources.clone(),
            authority: placement.authority.clone(),
            kernel_kind: event.kind,
            kernel_port: event.port,
            kernel_sequence: event.sequence,
            semantic_terminal: false,
            observed_time,
        });
    }
    if nodes.iter().enumerate().any(|(index, node)| {
        nodes[index + 1..]
            .iter()
            .any(|candidate| candidate.evidence == node.evidence)
    }) {
        return Err(BodyCausalEvidenceRefusal::MismatchedExecution);
    }
    Ok(nodes)
}

fn validate_plan_report(
    plan: &BodyPlan,
    report: &BodyRunReport,
) -> Result<(), BodyCausalEvidenceRefusal> {
    if !report.play.validate_for(plan)
        || report.play.plan_id != plan.plan_id
        || report.play.body_id != plan.body_id
        || report.play.wake_id != plan.wake_id
        || report.wake_at_start.body_id != plan.body_id
        || report.wake_at_start.wake_id != plan.wake_id
        || report.terminal_sign.active_play_id.as_ref() != Some(&report.play.active_play_id)
    {
        return Err(BodyCausalEvidenceRefusal::MismatchedBodyPlan);
    }
    let fragments = plan
        .plots
        .iter()
        .flat_map(|plot| &plot.plan.fragments)
        .collect::<Vec<_>>();
    if fragments.len() != report.partitions.len() {
        return Err(BodyCausalEvidenceRefusal::MismatchedIdentityMap);
    }
    if fragments.iter().any(|fragment| {
        fragment.host_id != report.terminal_sign.host_id
            || fragment.boot_id != report.terminal_sign.boot_id
            || fragment.placements.iter().any(|placement| {
                placement.host_id != report.terminal_sign.host_id
                    || placement.boot_id != report.terminal_sign.boot_id
            })
    }) {
        return Err(BodyCausalEvidenceRefusal::MismatchedExecution);
    }
    for map in &report.partitions {
        let Some(fragment) = fragments.iter().find(|fragment| {
            fragment.plan_id == map.plan_id && fragment.fragment_id == map.fragment_id
        }) else {
            return Err(BodyCausalEvidenceRefusal::MismatchedIdentityMap);
        };
        if map.placements.len() != fragment.placements.len()
            || map.placements.iter().any(|(_, placement)| {
                !fragment
                    .placements
                    .iter()
                    .any(|candidate| &candidate.placement_id == placement)
            })
        {
            return Err(BodyCausalEvidenceRefusal::MismatchedIdentityMap);
        }
    }
    Ok(())
}

fn validate_report_identity_against_nodes(
    report: &BodyRunReport,
    nodes: &[BodyCausalNode],
) -> Result<(), BodyCausalEvidenceRefusal> {
    if report.terminal_sign.active_play_id.as_ref() != Some(&report.play.active_play_id)
        || !nodes.iter().any(|node| {
            node.play_id == report.play.active_play_id && node.plan_id == report.play.plan_id
        })
    {
        return Err(BodyCausalEvidenceRefusal::MismatchedExecution);
    }
    Ok(())
}

fn resolve_event<'a>(
    plan: &'a BodyPlan,
    maps: &[KernelIdentityMap],
    event: &KernelEvent,
) -> Result<
    (
        &'a conduit_core::PlanFragment,
        &'a conduit_core::PlannedGear,
    ),
    BodyCausalEvidenceRefusal,
> {
    let matches = maps
        .iter()
        .filter_map(|map| {
            map.placement_for_node(event.node)
                .map(|placement| (map, placement))
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(BodyCausalEvidenceRefusal::MismatchedIdentityMap);
    }
    let (map, placement_id) = matches[0];
    let fragment = plan
        .plots
        .iter()
        .flat_map(|plot| &plot.plan.fragments)
        .find(|fragment| fragment.plan_id == map.plan_id && fragment.fragment_id == map.fragment_id)
        .ok_or(BodyCausalEvidenceRefusal::MismatchedIdentityMap)?;
    let placement = fragment
        .placements
        .iter()
        .find(|placement| &placement.placement_id == placement_id)
        .ok_or(BodyCausalEvidenceRefusal::MismatchedIdentityMap)?;
    Ok((fragment, placement))
}

fn is_relevant(kind: KernelEventKind) -> bool {
    matches!(
        kind,
        KernelEventKind::ValueRouted
            | KernelEventKind::ValueConsumed
            | KernelEventKind::CancellationRequested
            | KernelEventKind::BackFailed
            | KernelEventKind::SemanticAbnormal
            | KernelEventKind::SemanticAbnormalRecovered
            | KernelEventKind::BackCompleted
    )
}

fn is_abnormal(kind: KernelEventKind) -> bool {
    matches!(
        kind,
        KernelEventKind::BackFailed | KernelEventKind::SemanticAbnormal
    )
}

fn is_completion(kind: KernelEventKind) -> bool {
    matches!(
        kind,
        KernelEventKind::BackCompleted | KernelEventKind::SemanticAbnormalRecovered
    )
}

fn outcome(kind: KernelEventKind) -> EvidenceOutcome {
    match kind {
        KernelEventKind::ValueRouted => EvidenceOutcome::InfoRouted,
        KernelEventKind::ValueConsumed => EvidenceOutcome::InfoConsumed,
        KernelEventKind::CancellationRequested => EvidenceOutcome::CancellationRequested,
        KernelEventKind::BackFailed | KernelEventKind::SemanticAbnormal => {
            EvidenceOutcome::RealizationUnsatisfied
        }
        KernelEventKind::SemanticAbnormalRecovered => EvidenceOutcome::Recovered,
        KernelEventKind::BackCompleted => EvidenceOutcome::PlayCompleted,
        _ => unreachable!("only relevant kernel events become causal nodes"),
    }
}

#[cfg(test)]
#[path = "body_causal_evidence_tests.rs"]
mod tests;
