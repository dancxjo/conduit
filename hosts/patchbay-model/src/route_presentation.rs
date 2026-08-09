//! Toolkit-independent views of one evidence-backed distributed route demonstration.

use conduit_core::{
    ConnectionId, ConnectionProvider, ConnectionProviderInstanceId, EvidenceId, LinkBindingId,
    PlanId, SourceDocumentId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteCandidatePresentation {
    pub order: usize,
    pub binding_id: LinkBindingId,
    pub provider: ConnectionProvider,
    pub provider_instance_id: ConnectionProviderInstanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlanPresentation {
    pub plan_id: PlanId,
    pub connection_id: ConnectionId,
    pub candidates: Vec<RouteCandidatePresentation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlanRecoveryPresentation {
    pub prior: RoutePlanPresentation,
    pub replacement_plan_id: PlanId,
    pub unavailable_binding_id: LinkBindingId,
    pub unavailable_evidence_id: EvidenceId,
    pub unsatisfied_evidence_id: EvidenceId,
    pub planning_request_evidence_id: EvidenceId,
    pub planning_success_evidence_id: EvidenceId,
    pub installed_evidence_id: EvidenceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamePlanFallbackPresentation {
    pub plan: RoutePlanPresentation,
    pub unavailable_binding_id: LinkBindingId,
    pub unavailable_evidence_id: EvidenceId,
    pub selected_binding_id: LinkBindingId,
    pub selection_evidence_id: EvidenceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusedRoutePresentation {
    pub binding_id: LinkBindingId,
    pub observation_evidence_id: EvidenceId,
}

/// One immutable semantic snapshot consumed by every route-demo presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedRoutePresentation {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub new_plan: NewPlanRecoveryPresentation,
    pub same_plan: SamePlanFallbackPresentation,
    pub refused: RefusedRoutePresentation,
}

impl DistributedRoutePresentation {
    /// Compact hierarchy for the native visual surface. Exact identifiers stay visible,
    /// while the recovery distinction occupies the primary hierarchy.
    pub fn visual_lines(&self) -> Vec<String> {
        let mut lines = vec![
            "ROUTE RECOVERY — same facts, two outcomes".into(),
            format!(
                "FORM source={} checked={}",
                self.source_document_id.as_str(),
                self.checked_form_id.as_str()
            ),
            "NEW-PLAN RECOVERY".into(),
            format!(
                "  Plan A  id={} connection={}",
                self.new_plan.prior.plan_id.as_str(),
                self.new_plan.prior.connection_id.as_str()
            ),
        ];
        lines.extend(candidate_lines(
            &self.new_plan.prior,
            self.new_plan
                .prior
                .candidates
                .first()
                .map(|item| &item.binding_id),
            None,
        ));
        lines.extend([
            format!(
                "    ↓ {} unavailable",
                display_binding(&self.new_plan.unavailable_binding_id)
            ),
            "  UNSATISFIED — no admitted route ready".into(),
            format!(
                "    ↓ planning requested evidence={}",
                self.new_plan.planning_request_evidence_id.as_str()
            ),
            format!(
                "  Plan B  id={} prior={}",
                self.new_plan.replacement_plan_id.as_str(),
                self.new_plan.prior.plan_id.as_str()
            ),
            format!(
                "  evidence unavailable={} unsatisfied={} planned={} installed={}",
                self.new_plan.unavailable_evidence_id.as_str(),
                self.new_plan.unsatisfied_evidence_id.as_str(),
                self.new_plan.planning_success_evidence_id.as_str(),
                self.new_plan.installed_evidence_id.as_str()
            ),
            "SAME-PLAN FALLBACK".into(),
            format!(
                "  Plan C  id={} connection={}",
                self.same_plan.plan.plan_id.as_str(),
                self.same_plan.plan.connection_id.as_str()
            ),
        ]);
        lines.extend(candidate_lines(
            &self.same_plan.plan,
            self.same_plan
                .plan
                .candidates
                .first()
                .map(|item| &item.binding_id),
            None,
        ));
        lines.extend([
            format!(
                "    ↓ {} unavailable",
                display_binding(&self.same_plan.unavailable_binding_id)
            ),
            format!(
                "  selected={} — Plan C unchanged id={}",
                display_binding(&self.same_plan.selected_binding_id),
                self.same_plan.plan.plan_id.as_str()
            ),
            format!(
                "  evidence unavailable={} selection={}",
                self.same_plan.unavailable_evidence_id.as_str(),
                self.same_plan.selection_evidence_id.as_str()
            ),
            format!(
                "UNPLANNED ROUTE refused={} reason=not sealed into Plan evidence={}",
                display_binding(&self.refused.binding_id),
                self.refused.observation_evidence_id.as_str()
            ),
        ]);
        let insertion = lines.len().saturating_sub(3);
        let changed_candidates = candidate_lines(
            &self.same_plan.plan,
            Some(&self.same_plan.selected_binding_id),
            Some(&self.same_plan.unavailable_binding_id),
        );
        lines.splice(insertion..insertion, changed_candidates);
        lines
    }

    /// Deterministic non-spatial narration for terminals, screen readers, logs,
    /// and bounded conversational clients.
    pub fn linear_lines(&self) -> Vec<String> {
        vec![
            format!(
                "Form source {} checked {}. Plan {} connection {} has one admitted route, {}. {} became unavailable with evidence {}. The Play became unsatisfied with evidence {}. Planning was requested with evidence {} and succeeded with evidence {}. Replacement Plan {} superseded prior Plan {} with realization evidence {}.",
                self.source_document_id.as_str(),
                self.checked_form_id.as_str(),
                self.new_plan.prior.plan_id.as_str(),
                self.new_plan.prior.connection_id.as_str(),
                candidate_names(&self.new_plan.prior),
                display_binding(&self.new_plan.unavailable_binding_id),
                self.new_plan.unavailable_evidence_id.as_str(),
                self.new_plan.unsatisfied_evidence_id.as_str(),
                self.new_plan.planning_request_evidence_id.as_str(),
                self.new_plan.planning_success_evidence_id.as_str(),
                self.new_plan.replacement_plan_id.as_str(),
                self.new_plan.prior.plan_id.as_str(),
                self.new_plan.installed_evidence_id.as_str(),
            ),
            format!(
                "Plan {} connection {} has two admitted routes in deterministic order: {}. {} became unavailable with evidence {}. {} was selected with evidence {}. Plan identity did not change: {}.",
                self.same_plan.plan.plan_id.as_str(),
                self.same_plan.plan.connection_id.as_str(),
                candidate_names(&self.same_plan.plan),
                display_binding(&self.same_plan.unavailable_binding_id),
                self.same_plan.unavailable_evidence_id.as_str(),
                display_binding(&self.same_plan.selected_binding_id),
                self.same_plan.selection_evidence_id.as_str(),
                self.same_plan.plan.plan_id.as_str(),
            ),
            format!(
                "An observed ambient route, {}, was refused because it was not sealed into the active Plan. Observation evidence was {}.",
                display_binding(&self.refused.binding_id),
                self.refused.observation_evidence_id.as_str(),
            ),
        ]
    }
}

fn candidate_lines(
    plan: &RoutePlanPresentation,
    selected: Option<&LinkBindingId>,
    unavailable: Option<&LinkBindingId>,
) -> Vec<String> {
    plan.candidates
        .iter()
        .map(|candidate| {
            let marker = if unavailable == Some(&candidate.binding_id) {
                "✕"
            } else if selected == Some(&candidate.binding_id) {
                "●"
            } else {
                "○"
            };
            format!(
                "    {marker} order={} {} binding={} provider-instance={}",
                candidate.order,
                display_provider(candidate.provider),
                candidate.binding_id.as_str(),
                candidate.provider_instance_id.as_str()
            )
        })
        .collect()
}

fn candidate_names(plan: &RoutePlanPresentation) -> String {
    plan.candidates
        .iter()
        .map(|candidate| {
            format!(
                "{} (binding {}, provider instance {})",
                display_provider(candidate.provider),
                candidate.binding_id.as_str(),
                candidate.provider_instance_id.as_str()
            )
        })
        .collect::<Vec<_>>()
        .join(", then ")
}

fn display_provider(provider: ConnectionProvider) -> &'static str {
    match provider {
        ConnectionProvider::Local => "local connection",
        ConnectionProvider::InMemory => "in-memory connection",
        ConnectionProvider::FixtureFrame => "fixture frame",
        ConnectionProvider::FixtureDatagram => "fixture datagram",
        ConnectionProvider::WebSocket => "WebSocket",
        ConnectionProvider::UsbCdc => "USB CDC",
    }
}

fn display_binding(binding_id: &LinkBindingId) -> &str {
    match binding_id.as_str() {
        "s4/distributed-signal-usb-link" => "USB CDC",
        conduit_signal::DISTRIBUTED_LINK_BINDING_ID => "WebSocket",
        "ambient/unplanned-wifi" => "ambient Wi-Fi",
        other => other,
    }
}
