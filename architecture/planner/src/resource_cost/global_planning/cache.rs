//! Bounded reuse of ordinary sealed Plans; volatile scores are never retained.
use super::cache_fingerprint as hash;
use super::*;

#[derive(Debug, Clone, Copy)]
pub struct GlobalPlanCachePolicy {
    pub maximum_entries: usize,
    /// Extent of the internal encoded Plan stream, not a heap-byte measurement.
    pub maximum_encoded_plan_bytes: usize,
    pub maximum_fingerprint_bytes_per_call: usize,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GlobalPlanCacheMetrics {
    pub constructed_plans: u32,
    pub reused_plans: u32,
    pub invalidated_entries: u32,
    pub evicted_entries: u32,
    pub bypassed_assignments: u32,
    pub fingerprint_bytes: usize,
    pub retained_entries: usize,
    pub retained_encoded_plan_bytes: usize,
}
struct Entry {
    key: [u8; 32],
    choices: PlacementChoices,
    plan: Plan,
    bytes: usize,
}
/// Preparation-only cache. A fingerprint neither authenticates a producer nor
/// grants capacity. Every call still admits fresh candidates and observations.
pub struct GlobalPlanCache {
    policy: GlobalPlanCachePolicy,
    entries: Vec<Entry>,
    metrics: GlobalPlanCacheMetrics,
    call_common: Option<[u8; 32]>,
    call_hosts: Vec<(usize, [u8; 32])>,
    call_advertisements: Vec<(usize, [u8; 32])>,
}
impl GlobalPlanCache {
    pub fn admit(policy: GlobalPlanCachePolicy) -> Result<Self, CostEvidenceRefusal> {
        if policy.maximum_entries == 0
            || policy.maximum_entries > 32
            || policy.maximum_encoded_plan_bytes == 0
            || policy.maximum_encoded_plan_bytes > 1_048_576
            || policy.maximum_fingerprint_bytes_per_call == 0
            || policy.maximum_fingerprint_bytes_per_call > 16_777_216
        {
            return Err(CostEvidenceRefusal::InputLimit);
        }
        Ok(Self {
            policy,
            entries: Vec::with_capacity(policy.maximum_entries),
            metrics: GlobalPlanCacheMetrics::default(),
            call_common: None,
            call_hosts: Vec::new(),
            call_advertisements: Vec::new(),
        })
    }
    pub fn discard(&mut self) {
        self.entries.clear();
        self.metrics.retained_entries = 0;
        self.metrics.retained_encoded_plan_bytes = 0;
    }
    pub fn metrics(&self) -> GlobalPlanCacheMetrics {
        self.metrics
    }
    pub fn plan<'a>(
        &mut self,
        plot: &CheckedPlot,
        inputs: CostPlanningInputs<'a, '_>,
        budget: GlobalPlacementBudget,
        objective: GlobalPlacementObjective,
        transfer: Option<GlobalTransferInputs<'a>>,
        work: &mut ExactNumericWork,
    ) -> Result<GlobalPlacementResult, GlobalPlacementRefusal> {
        // These memoized digests live for this call only, never across updates.
        self.call_common = None;
        self.call_hosts.clear();
        self.call_advertisements.clear();
        self.metrics = GlobalPlanCacheMetrics {
            retained_entries: self.entries.len(),
            retained_encoded_plan_bytes: self.entries.iter().map(|e| e.bytes).sum(),
            ..Default::default()
        };
        plot.validate_identities()
            .map_err(|_| GlobalPlacementRefusal::Input(CostEvidenceRefusal::InvalidIdentity))?;
        super::plan_global(plot, inputs, budget, objective, transfer, Some(self), work)
    }
    pub(super) fn key(
        &mut self,
        plot: &CheckedPlot,
        inputs: CostPlanningInputs<'_, '_>,
        requirements: &BTreeMap<conduit_core::GearId, crate::HardRealizationRequirements>,
        choices: &PlacementChoices,
    ) -> Option<[u8; 32]> {
        if self.metrics.fingerprint_bytes == self.policy.maximum_fingerprint_bytes_per_call {
            self.metrics.bypassed_assignments += 1;
            return None;
        }
        if requirements.values().any(|r| !r.predicates.is_empty()) {
            self.metrics.bypassed_assignments += 1;
            return None;
        }
        let needs: Vec<_> = requirements
            .iter()
            .map(|(gear, r)| {
                (
                    gear,
                    (
                        r.minimum_queue_items,
                        r.minimum_queue_bytes,
                        &r.maximum_resource_units,
                        &r.permitted_host_calls,
                        &r.permitted_authority_contracts,
                        &r.minimum_characteristic_counts,
                        &r.maximum_characteristic_counts,
                        &r.required_characteristic_flags,
                        &r.required_characteristic_labels,
                    ),
                )
            })
            .collect();
        let options = inputs.options;
        let common = if let Some(common) = self.call_common {
            common
        } else {
            let hard = (
                plot.identity(),
                inputs.bases,
                needs,
                (
                    options.connection_bases,
                    options.line_candidates,
                    options.connection_item_capacity,
                    options.connection_byte_capacity,
                    options.authority_grants,
                    options.protected_resource_grants,
                    options.line_offers,
                ),
            );
            let Some(common) = self.digest(&hard) else {
                self.metrics.bypassed_assignments += 1;
                return None;
            };
            self.call_common = Some(common);
            common
        };
        let mut hosts = Vec::new();
        for (position, host) in inputs
            .hosts
            .iter()
            .enumerate()
            .filter(|(_, host)| choices.by_gear.values().any(|c| c.host_id == host.host_id))
        {
            let found = self
                .call_hosts
                .iter()
                .find(|(index, _)| *index == position)
                .map(|(_, digest)| *digest);
            let digest = if let Some(digest) = found {
                digest
            } else {
                let Some(digest) = self.digest(host) else {
                    self.metrics.bypassed_assignments += 1;
                    return None;
                };
                self.call_hosts.push((position, digest));
                digest
            };
            hosts.push(digest);
        }
        let mut advertisements = Vec::new();
        for (position, advertisement) in
            inputs.advertisements.iter().enumerate().filter(|(_, a)| {
                choices
                    .by_gear
                    .values()
                    .any(|c| c.host_id == a.host_id && c.capability_id == a.capability_id)
            })
        {
            let found = self
                .call_advertisements
                .iter()
                .find(|(index, _)| *index == position)
                .map(|(_, digest)| *digest);
            let digest = if let Some(digest) = found {
                digest
            } else {
                let Some(digest) = self.digest(advertisement) else {
                    self.metrics.bypassed_assignments += 1;
                    return None;
                };
                self.call_advertisements.push((position, digest));
                digest
            };
            advertisements.push(digest);
        }
        let identities: Vec<_> = choices
            .by_gear
            .iter()
            .map(|(gear, choice)| (gear, (&choice.host_id, &choice.capability_id)))
            .collect();
        let key = self.digest(&(common, identities, hosts, advertisements));
        if key.is_none() {
            self.metrics.bypassed_assignments += 1;
        }
        key
    }
    fn digest<T: serde::Serialize + ?Sized>(&mut self, value: &T) -> Option<[u8; 32]> {
        let left = self.policy.maximum_fingerprint_bytes_per_call - self.metrics.fingerprint_bytes;
        let (result, bytes) = hash::attempt(value, left);
        self.metrics.fingerprint_bytes += bytes;
        result.ok()
    }
    pub(super) fn get(&mut self, key: [u8; 32], choices: &PlacementChoices) -> Option<Plan> {
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.key == key && entry.choices == *choices)
        {
            self.metrics.reused_plans += 1;
            return Some(entry.plan.clone());
        }
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.choices == *choices)
        {
            let old = self.entries.remove(index);
            self.metrics.retained_encoded_plan_bytes -= old.bytes;
            self.metrics.invalidated_entries += 1;
            self.metrics.retained_entries = self.entries.len();
        }
        None
    }
    pub(super) fn constructed(&mut self) {
        self.metrics.constructed_plans += 1;
    }
    pub(super) fn retain(&mut self, key: [u8; 32], choices: PlacementChoices, plan: &Plan) {
        let left = self.policy.maximum_fingerprint_bytes_per_call - self.metrics.fingerprint_bytes;
        let (result, bytes) = hash::attempt(plan, left.min(self.policy.maximum_encoded_plan_bytes));
        self.metrics.fingerprint_bytes += bytes;
        if result.is_err() {
            return;
        }
        while self.entries.len() == self.policy.maximum_entries
            || self.metrics.retained_encoded_plan_bytes + bytes
                > self.policy.maximum_encoded_plan_bytes
        {
            if self.entries.is_empty() {
                return;
            }
            let old = self.entries.remove(0);
            self.metrics.retained_encoded_plan_bytes -= old.bytes;
            self.metrics.evicted_entries += 1;
        }
        self.entries.push(Entry {
            key,
            choices,
            plan: plan.clone(),
            bytes,
        });
        self.metrics.retained_entries = self.entries.len();
        self.metrics.retained_encoded_plan_bytes += bytes;
    }
}
