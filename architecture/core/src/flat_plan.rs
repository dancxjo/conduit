//! Immutable descriptive flat Plans and their Core structural verification.
//!
//! This proof is not Source selection, current-offer admission, a Body seal,
//! native ownership, or permission to execute. Those boundaries remain separate.
use crate::*;

/// Stable owned records or immutable borrowed records; no caller reader callbacks.
#[derive(Clone, Copy)]
pub enum PlannedGearRefs<'a> {
    Owned(&'a [PlannedGear]),
    Borrowed(&'a [PlannedGearRef<'a>]),
}
impl<'a> PlannedGearRefs<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Owned(v) => v.len(),
            Self::Borrowed(v) => v.len(),
        }
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = PlannedGearRef<'a>> {
        (0..self.len()).map(move |i| match self {
            Self::Owned(v) => (&v[i]).into(),
            Self::Borrowed(v) => v[i],
        })
    }
}

/// Complete one-fragment description. Independent root/fragment IDs remain
/// independent inputs so contradictory claims cannot disappear in adaptation.
#[derive(Clone, Copy)]
pub struct FlatPlanRef<'a> {
    pub plan_id: &'a str,
    pub root: PlanFingerprintInputRef<'a>,
    pub completion_policy: PlanCompletionPolicy,
    pub fragment_plan_id: &'a str,
    pub fragment_id: &'a str,
    pub header: FragmentFingerprintHeaderRef<'a>,
    pub placements: PlannedGearRefs<'a>,
    pub tail: FragmentFingerprintTailRef<'a>,
    pub fragment_commitments: FragmentCommitmentRefs<'a>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatPlanRefusal {
    FragmentCount,
    UnsupportedProfile,
    InvalidPlan,
}

/// Opaque proof over stable, immutable full descriptions. No Deserialize or
/// constructor accepting alleged IDs/digests. Configuration remains full input.
pub struct VerifiedFlatPlanRef<'a> {
    input: FlatPlanRef<'a>,
}
impl<'a> VerifiedFlatPlanRef<'a> {
    pub fn view(&self) -> FlatPlanRef<'a> {
        self.input
    }
}
impl<'a> FlatPlanRef<'a> {
    /// Borrow a rich Plan without cloning any record. Unsupported structural
    /// profiles are refused by verification, never silently projected away.
    pub fn from_plan(plan: &'a Plan) -> Result<Self, FlatPlanRefusal> {
        let [fragment] = plan.fragments.as_slice() else {
            return Err(FlatPlanRefusal::FragmentCount);
        };
        Ok(Self {
            plan_id: plan.plan_id.as_str(),
            root: PlanFingerprintInputRef {
                source_document_id: plan.source_document_id.as_str(),
                checked_plot_id: plan.checked_plot_id.as_str(),
                expanded_plot_id: plan.expanded_plot_id.as_str(),
                realization_backs: &plan.realization_backs,
                activations: &plan.activations,
                commitments: FragmentCommitmentRefs::Single(FragmentCommitmentRef {
                    host_id: fragment.host_id.as_str(),
                    fragment_id: fragment.fragment_id.as_str(),
                }),
                activation_preparations: &plan.activation_preparations,
            },
            completion_policy: plan.completion_policy,
            fragment_plan_id: fragment.plan_id.as_str(),
            fragment_id: fragment.fragment_id.as_str(),
            header: fragment.into(),
            placements: PlannedGearRefs::Owned(&fragment.placements),
            tail: fragment.into(),
            fragment_commitments: FragmentCommitmentRefs::Owned(&fragment.plan_fragments),
        })
    }
    pub fn verify(self) -> Result<VerifiedFlatPlanRef<'a>, FlatPlanRefusal> {
        self.profile()?;
        if !self.root_fingerprint_matches() {
            return Err(FlatPlanRefusal::InvalidPlan);
        }
        let mut builder = FragmentFingerprintBuilder::new_ref(self.header, self.placements.len());
        for placement in self.placements.iter() {
            builder
                .push_placement_ref(placement)
                .map_err(|_| FlatPlanRefusal::InvalidPlan)?;
        }
        let digest = builder
            .finish_ref_digest(self.tail)
            .map_err(|_| FlatPlanRefusal::InvalidPlan)?;
        if !digest_matches(self.fragment_id, digest) || !self.structure() {
            return Err(FlatPlanRefusal::InvalidPlan);
        }
        Ok(VerifiedFlatPlanRef { input: self })
    }
    pub(crate) fn profile(self) -> Result<(), FlatPlanRefusal> {
        if !self.root.realization_backs.is_empty()
            || !self.header.realization_backs.is_empty()
            || !self.root.activations.is_empty()
            || !self.root.activation_preparations.is_empty()
            || !self.header.execution_regions.is_empty()
            || !self.header.execution_fusions.is_empty()
            || !self.tail.fore_ports.is_empty()
            || !self.tail.shared_pools.is_empty()
            || !self.tail.states.is_empty()
            || self
                .tail
                .connections
                .iter()
                .any(|v| v.selected_line.is_some() || !v.admitted_lines.is_empty())
        {
            return Err(FlatPlanRefusal::UnsupportedProfile);
        }
        Ok(())
    }
    pub(crate) fn root_fingerprint_matches(self) -> bool {
        digest_matches(
            self.plan_id,
            crate::plan_fingerprint::root::digest(self.root),
        )
    }
    pub(crate) fn structure(self) -> bool {
        self.fragment_plan_id == self.plan_id
            && self.header.source_document_id == self.root.source_document_id
            && self.header.checked_plot_id == self.root.checked_plot_id
            && self.header.expanded_plot_id == self.root.expanded_plot_id
            && self.header.completion_policy == self.completion_policy
            && self.root.commitments.len() == 1
            && self.root.commitments.iter().all(|item| {
                item.host_id == self.header.host_id && item.fragment_id == self.fragment_id
            })
            && verify_commitments(
                self.header.host_id,
                self.fragment_id,
                self.fragment_commitments,
            )
            && self.fragment_commitments.len() == 1
            && verify_terminal_ports(self.placements)
            // With no pools, every reference must still resolve (none can).
            && self.placements.iter().all(|item| item.pool_references.is_empty())
            && verify_local_connections(self.placements, self.tail.connections)
    }
}

pub(crate) fn verify_commitments(
    host: &str,
    id: &str,
    commitments: FragmentCommitmentRefs<'_>,
) -> bool {
    let mut prior = None;
    let mut own = 0usize;
    for item in commitments.iter() {
        if prior
            .is_some_and(|(h, f)| (h, f) > (item.host_id, item.fragment_id) || h == item.host_id)
        {
            return false;
        }
        prior = Some((item.host_id, item.fragment_id));
        if item.host_id == host && item.fragment_id == id {
            own += 1;
        }
    }
    own == 1
}
pub(crate) fn verify_terminal_ports(placements: PlannedGearRefs<'_>) -> bool {
    placements.iter().all(|v| {
        v.terminal_transductions.iter().all(|profile| {
            crate::capability_offer::validate_terminal_transduction_ports_ref(
                v.inputs, v.outputs, profile,
            )
            .is_ok()
        })
    })
}
fn verify_local_connections(
    placements: PlannedGearRefs<'_>,
    connections: PlannedConnectionRefs<'_>,
) -> bool {
    for connection in connections.iter() {
        if connections
            .iter()
            .filter(|v| v.connection_id == connection.connection_id)
            .count()
            != 1
        {
            return false;
        }
        let mut sources = placements
            .iter()
            .filter(|v| v.placement_id == connection.source_placement_id);
        let mut sinks = placements
            .iter()
            .filter(|v| v.placement_id == connection.sink_placement_id);
        let (Some(source), Some(sink)) = (sources.next(), sinks.next()) else {
            return false;
        };
        if sources.next().is_some()
            || sinks.next().is_some()
            || source.host_id != sink.host_id
            || connection.selected_line.is_some()
            || !connection.admitted_lines.is_empty()
        {
            return false;
        }
    }
    true
}
fn digest_matches(id: &str, digest: [u8; 32]) -> bool {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    id.len() == 64
        && digest.iter().enumerate().all(|(i, b)| {
            id.as_bytes()[i * 2] == HEX[usize::from(b >> 4)]
                && id.as_bytes()[i * 2 + 1] == HEX[usize::from(b & 15)]
        })
}
