//! Optional, exact software construction intent. Readiness remains target-owned.
use crate::{CapabilitySelection, MAX_PROFILE_ITEMS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostSoftwarePurpose {
    pub capabilities: Vec<CapabilitySelection>,
}
impl HostSoftwarePurpose {
    pub(crate) fn canonicalize(&mut self) {
        self.capabilities.sort_by(|a, b| {
            (&a.kind, &a.contract_revision, &a.implementation).cmp(&(
                &b.kind,
                &b.contract_revision,
                &b.implementation,
            ))
        });
    }
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.capabilities.len() > MAX_PROFILE_ITEMS {
            return Err("software purpose exceeds existing profile capability bound".into());
        }
        let mut seen = BTreeSet::new();
        for selected in &self.capabilities {
            if !seen.insert((
                &selected.kind,
                &selected.contract_revision,
                &selected.implementation,
            )) {
                return Err("duplicate software capability selection".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "software_tests.rs"]
mod tests;
