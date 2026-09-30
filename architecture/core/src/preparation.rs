//! Finite all-Host preparation before an exact plan may start.
//!
//! This is an admission protocol, not a scheduler or consensus system. Every
//! selected host remains authoritative for validating, reserving, starting,
//! and releasing its own exact fragment.

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    verify_plan, ActivePlayId, BootId, FailureReason, FragmentId, HostId, OfferGeneration, Plan,
    PlanFragment, PlanId,
};

/// Maximum selected hosts admitted by one coordinated preparation attempt.
pub const MAX_PREPARATION_HOSTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparationHostIdentity {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedFragmentReceipt {
    plan_id: PlanId,
    fragment_id: FragmentId,
    host: PreparationHostIdentity,
}

impl PreparedFragmentReceipt {
    pub fn new(fragment: &PlanFragment) -> Self {
        Self {
            plan_id: fragment.plan_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
            host: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
        }
    }

    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn fragment_id(&self) -> &FragmentId {
        &self.fragment_id
    }

    pub fn host(&self) -> &PreparationHostIdentity {
        &self.host
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostPreparationRefusal {
    InvalidFragment,
    WrongHost,
    StaleBoot,
    StaleOffer,
    CapabilityUnavailable,
    ImplementationUnavailable,
    ResourceUnavailable,
    AuthorityUnavailable,
    LineUnavailable,
    FiniteLimitExceeded,
    AlreadyPrepared,
    NotPrepared,
    PreparedBindingMismatch,
    LocalFailure(FailureReason),
}

/// One selected host's independent preparation boundary.
///
/// `prepare_fragment` must either retain the exact fragment and its finite
/// reservations or leave local state unchanged. It must not start semantic
/// production or external effects. `validate_start` is likewise non-mutating.
pub trait PlanPreparationHost {
    fn preparation_identity(&self) -> PreparationHostIdentity;

    fn prepare_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<PreparedFragmentReceipt, HostPreparationRefusal>;

    fn release_fragment(
        &mut self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal>;

    fn validate_start(
        &self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal>;

    fn start_fragment(&mut self, receipt: &PreparedFragmentReceipt) -> ActivePlayId;
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct PreparedPlan {
    plan_id: PlanId,
    receipts: Vec<PreparedFragmentReceipt>,
    subordinate_receipts: Vec<(alloc::string::String, PreparedFragmentReceipt)>,
}

impl PreparedPlan {
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn receipts(&self) -> &[PreparedFragmentReceipt] {
        &self.receipts
    }
    pub fn subordinate_receipts(&self) -> &[(alloc::string::String, PreparedFragmentReceipt)] {
        &self.subordinate_receipts
    }
}

pub fn verify_prepared_plan(prepared: &PreparedPlan, plan: &Plan) -> bool {
    if !verify_plan(plan)
        || prepared.plan_id != plan.plan_id
        || prepared.receipts.len() != plan.fragments.len()
    {
        return false;
    }
    if !prepared
        .receipts
        .iter()
        .zip(&plan.fragments)
        .all(|(receipt, fragment)| receipt_matches_fragment(receipt, fragment))
    {
        return false;
    }
    let expected = plan
        .activation_preparations
        .iter()
        .map(|binding| binding.child_fragments.len())
        .sum::<usize>();
    prepared.subordinate_receipts.len() == expected
        && plan.activation_preparations.iter().all(|binding| {
            let selected = plan.activations.iter().find_map(|entry| match entry {
                crate::PlannedActivationEntry::Unary(v)
                    if v.activation_id == binding.activation_id =>
                {
                    Some(v.selected_plan.as_ref())
                }
                crate::PlannedActivationEntry::Fold(v)
                    if v.activation_id == binding.activation_id =>
                {
                    Some(v.selected_plan.as_ref())
                }
                crate::PlannedActivationEntry::Scan(v)
                    if v.activation_id == binding.activation_id =>
                {
                    Some(v.selected_plan.as_ref())
                }
                _ => None,
            });
            selected.is_some_and(|selected| {
                binding.child_fragments.iter().all(|child| {
                    let fragment = selected
                        .fragments
                        .iter()
                        .find(|fragment| fragment.fragment_id == child.fragment_id);
                    fragment.is_some_and(|fragment| {
                        prepared
                            .subordinate_receipts
                            .iter()
                            .filter(|(activation, receipt)| {
                                activation == &binding.activation_id
                                    && receipt_matches_fragment(receipt, fragment)
                            })
                            .count()
                            == 1
                    })
                })
            })
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartedPlan {
    plan_id: PlanId,
    active_plays: Vec<(HostId, ActivePlayId)>,
    subordinate_receipts: Vec<(alloc::string::String, PreparedFragmentReceipt)>,
}

impl StartedPlan {
    pub fn plan_id(&self) -> &PlanId {
        &self.plan_id
    }

    pub fn active_plays(&self) -> &[(HostId, ActivePlayId)] {
        &self.active_plays
    }
    pub fn subordinate_receipts(&self) -> &[(alloc::string::String, PreparedFragmentReceipt)] {
        &self.subordinate_receipts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparationRollbackFailure {
    pub fragment_id: FragmentId,
    pub reason: HostPreparationRefusal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanPreparationError {
    InvalidPlan,
    EmptyPlan,
    HostCapacityExceeded,
    HostSelectionFailed {
        fragment_id: FragmentId,
        reason: HostSelectionFailure,
        rollback_failures: Vec<PreparationRollbackFailure>,
    },
    HostRefused {
        fragment_id: FragmentId,
        reason: HostPreparationRefusal,
        rollback_failures: Vec<PreparationRollbackFailure>,
    },
    InvalidReceipt {
        fragment_id: FragmentId,
        rollback_failures: Vec<PreparationRollbackFailure>,
    },
    StartRefused {
        fragment_id: FragmentId,
        reason: HostPreparationRefusal,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostSelectionFailure {
    Missing,
    Ambiguous,
}

/// Independently prepares every exact fragment or releases all earlier
/// reservations in reverse preparation order.
pub fn prepare_plan_on_hosts(
    plan: &Plan,
    hosts: &mut [&mut dyn PlanPreparationHost],
) -> Result<PreparedPlan, PlanPreparationError> {
    if !verify_plan(plan) {
        return Err(PlanPreparationError::InvalidPlan);
    }
    if plan.fragments.is_empty() {
        return Err(PlanPreparationError::EmptyPlan);
    }
    let subordinate_count = plan
        .activation_preparations
        .iter()
        .try_fold(0usize, |n, b| n.checked_add(b.child_fragments.len()))
        .ok_or(PlanPreparationError::HostCapacityExceeded)?;
    if plan
        .fragments
        .len()
        .checked_add(subordinate_count)
        .is_none_or(|n| n > MAX_PREPARATION_HOSTS)
        || hosts.len() > MAX_PREPARATION_HOSTS
    {
        return Err(PlanPreparationError::HostCapacityExceeded);
    }

    let mut receipts = Vec::with_capacity(plan.fragments.len());
    for fragment in &plan.fragments {
        let matching = hosts
            .iter()
            .enumerate()
            .filter(|(_, host)| host.preparation_identity().host_id == fragment.host_id)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let host_index = match matching.as_slice() {
            [] => {
                let rollback_failures = rollback_receipts(&mut receipts, hosts);
                return Err(PlanPreparationError::HostSelectionFailed {
                    fragment_id: fragment.fragment_id.clone(),
                    reason: HostSelectionFailure::Missing,
                    rollback_failures,
                });
            }
            [index] => *index,
            _ => {
                let rollback_failures = rollback_receipts(&mut receipts, hosts);
                return Err(PlanPreparationError::HostSelectionFailed {
                    fragment_id: fragment.fragment_id.clone(),
                    reason: HostSelectionFailure::Ambiguous,
                    rollback_failures,
                });
            }
        };
        let identity = hosts[host_index].preparation_identity();
        let stale_reason = if identity.boot_id != fragment.boot_id {
            Some(HostPreparationRefusal::StaleBoot)
        } else if identity.offer_generation != fragment.offer_generation {
            Some(HostPreparationRefusal::StaleOffer)
        } else {
            None
        };
        if let Some(reason) = stale_reason {
            let rollback_failures = rollback_receipts(&mut receipts, hosts);
            return Err(PlanPreparationError::HostRefused {
                fragment_id: fragment.fragment_id.clone(),
                reason,
                rollback_failures,
            });
        }
        match hosts[host_index].prepare_fragment(fragment) {
            Ok(receipt) if receipt_matches_fragment(&receipt, fragment) => receipts.push(receipt),
            Ok(receipt) => {
                let mut rollback_failures = Vec::new();
                if let Err(reason) = hosts[host_index].release_fragment(&receipt) {
                    rollback_failures.push(PreparationRollbackFailure {
                        fragment_id: fragment.fragment_id.clone(),
                        reason,
                    });
                }
                rollback_failures.extend(rollback_receipts(&mut receipts, hosts));
                return Err(PlanPreparationError::InvalidReceipt {
                    fragment_id: fragment.fragment_id.clone(),
                    rollback_failures,
                });
            }
            Err(reason) => {
                let rollback_failures = rollback_receipts(&mut receipts, hosts);
                return Err(PlanPreparationError::HostRefused {
                    fragment_id: fragment.fragment_id.clone(),
                    reason,
                    rollback_failures,
                });
            }
        }
    }
    let mut subordinate_receipts: Vec<(alloc::string::String, PreparedFragmentReceipt)> =
        Vec::with_capacity(subordinate_count);
    for binding in &plan.activation_preparations {
        let selected = plan
            .activations
            .iter()
            .find_map(|entry| match entry {
                crate::PlannedActivationEntry::Unary(v)
                    if v.activation_id == binding.activation_id =>
                {
                    Some(v.selected_plan.as_ref())
                }
                crate::PlannedActivationEntry::Fold(v)
                    if v.activation_id == binding.activation_id =>
                {
                    Some(v.selected_plan.as_ref())
                }
                crate::PlannedActivationEntry::Scan(v)
                    if v.activation_id == binding.activation_id =>
                {
                    Some(v.selected_plan.as_ref())
                }
                _ => None,
            })
            .ok_or(PlanPreparationError::InvalidPlan)?;
        for child_binding in &binding.child_fragments {
            let child = selected
                .fragments
                .iter()
                .find(|f| f.fragment_id == child_binding.fragment_id)
                .ok_or(PlanPreparationError::InvalidPlan)?;
            let matching = hosts
                .iter()
                .enumerate()
                .filter(|(_, h)| h.preparation_identity().host_id == child.host_id)
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            let [host_index] = matching.as_slice() else {
                let mut all = receipts.clone();
                all.extend(subordinate_receipts.iter().map(|(_, r)| r.clone()));
                let rollback_failures = rollback_receipts(&mut all, hosts);
                return Err(PlanPreparationError::HostSelectionFailed {
                    fragment_id: child.fragment_id.clone(),
                    reason: if matching.is_empty() {
                        HostSelectionFailure::Missing
                    } else {
                        HostSelectionFailure::Ambiguous
                    },
                    rollback_failures,
                });
            };
            match hosts[*host_index].prepare_fragment(child) {
                Ok(receipt) if receipt_matches_fragment(&receipt, child) => {
                    subordinate_receipts.push((binding.activation_id.clone(), receipt))
                }
                Ok(receipt) => {
                    let _ = hosts[*host_index].release_fragment(&receipt);
                    let mut all = receipts.clone();
                    all.extend(subordinate_receipts.iter().map(|(_, r)| r.clone()));
                    let rollback_failures = rollback_receipts(&mut all, hosts);
                    return Err(PlanPreparationError::InvalidReceipt {
                        fragment_id: child.fragment_id.clone(),
                        rollback_failures,
                    });
                }
                Err(reason) => {
                    let mut all = receipts.clone();
                    all.extend(subordinate_receipts.iter().map(|(_, r)| r.clone()));
                    let rollback_failures = rollback_receipts(&mut all, hosts);
                    return Err(PlanPreparationError::HostRefused {
                        fragment_id: child.fragment_id.clone(),
                        reason,
                        rollback_failures,
                    });
                }
            }
        }
    }
    Ok(PreparedPlan {
        plan_id: plan.plan_id.clone(),
        receipts,
        subordinate_receipts,
    })
}

/// Starts only the exact receipts produced by complete all-Host preparation.
pub fn start_prepared_plan(
    prepared: PreparedPlan,
    hosts: &mut [&mut dyn PlanPreparationHost],
) -> Result<StartedPlan, PlanPreparationError> {
    if prepared.receipts.is_empty() || prepared.receipts.len() > MAX_PREPARATION_HOSTS {
        return Err(PlanPreparationError::EmptyPlan);
    }
    let mut indexes = Vec::with_capacity(prepared.receipts.len());
    for (_, receipt) in &prepared.subordinate_receipts {
        let matching = hosts
            .iter()
            .filter(|host| host.preparation_identity().host_id == receipt.host().host_id)
            .collect::<Vec<_>>();
        let [host] = matching.as_slice() else {
            return Err(PlanPreparationError::StartRefused {
                fragment_id: receipt.fragment_id().clone(),
                reason: HostPreparationRefusal::PreparedBindingMismatch,
            });
        };
        host.validate_start(receipt)
            .map_err(|reason| PlanPreparationError::StartRefused {
                fragment_id: receipt.fragment_id().clone(),
                reason,
            })?;
    }
    for receipt in &prepared.receipts {
        let matching = hosts
            .iter()
            .enumerate()
            .filter(|(_, host)| host.preparation_identity().host_id == receipt.host().host_id)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let index = match matching.as_slice() {
            [] => {
                return Err(PlanPreparationError::HostSelectionFailed {
                    fragment_id: receipt.fragment_id().clone(),
                    reason: HostSelectionFailure::Missing,
                    rollback_failures: Vec::new(),
                })
            }
            [index] => *index,
            _ => {
                return Err(PlanPreparationError::HostSelectionFailed {
                    fragment_id: receipt.fragment_id().clone(),
                    reason: HostSelectionFailure::Ambiguous,
                    rollback_failures: Vec::new(),
                })
            }
        };
        let identity = hosts[index].preparation_identity();
        if identity.boot_id != receipt.host().boot_id {
            return Err(PlanPreparationError::StartRefused {
                fragment_id: receipt.fragment_id().clone(),
                reason: HostPreparationRefusal::StaleBoot,
            });
        }
        if identity.offer_generation != receipt.host().offer_generation {
            return Err(PlanPreparationError::StartRefused {
                fragment_id: receipt.fragment_id().clone(),
                reason: HostPreparationRefusal::StaleOffer,
            });
        }
        hosts[index].validate_start(receipt).map_err(|reason| {
            PlanPreparationError::StartRefused {
                fragment_id: receipt.fragment_id().clone(),
                reason,
            }
        })?;
        indexes.push(index);
    }

    let mut active_plays = Vec::with_capacity(prepared.receipts.len());
    for (receipt, index) in prepared.receipts.iter().zip(indexes) {
        let active_play = hosts[index].start_fragment(receipt);
        active_plays.push((receipt.host.host_id.clone(), active_play));
    }
    Ok(StartedPlan {
        plan_id: prepared.plan_id,
        active_plays,
        subordinate_receipts: prepared.subordinate_receipts,
    })
}

fn rollback_receipts(
    receipts: &mut Vec<PreparedFragmentReceipt>,
    hosts: &mut [&mut dyn PlanPreparationHost],
) -> Vec<PreparationRollbackFailure> {
    let mut failures = Vec::new();
    while let Some(receipt) = receipts.pop() {
        let matching = hosts
            .iter()
            .enumerate()
            .filter(|(_, host)| host.preparation_identity().host_id == receipt.host().host_id)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        match matching.as_slice() {
            [index] if hosts[*index].preparation_identity().boot_id != receipt.host().boot_id => {
                failures.push(PreparationRollbackFailure {
                    fragment_id: receipt.fragment_id().clone(),
                    reason: HostPreparationRefusal::StaleBoot,
                });
            }
            [index]
                if hosts[*index].preparation_identity().offer_generation
                    != receipt.host().offer_generation =>
            {
                failures.push(PreparationRollbackFailure {
                    fragment_id: receipt.fragment_id().clone(),
                    reason: HostPreparationRefusal::StaleOffer,
                });
            }
            [index] => {
                if let Err(reason) = hosts[*index].release_fragment(&receipt) {
                    failures.push(PreparationRollbackFailure {
                        fragment_id: receipt.fragment_id().clone(),
                        reason,
                    });
                }
            }
            _ => failures.push(PreparationRollbackFailure {
                fragment_id: receipt.fragment_id().clone(),
                reason: HostPreparationRefusal::PreparedBindingMismatch,
            }),
        }
    }
    failures
}

fn identity_matches_fragment(identity: &PreparationHostIdentity, fragment: &PlanFragment) -> bool {
    identity.host_id == fragment.host_id
        && identity.boot_id == fragment.boot_id
        && identity.offer_generation == fragment.offer_generation
}

fn receipt_matches_fragment(receipt: &PreparedFragmentReceipt, fragment: &PlanFragment) -> bool {
    receipt.plan_id == fragment.plan_id
        && receipt.fragment_id == fragment.fragment_id
        && identity_matches_fragment(&receipt.host, fragment)
}
