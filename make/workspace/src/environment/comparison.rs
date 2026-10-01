use std::collections::BTreeSet;

use conduit_core::HostId;
use conduit_observatory::{build_report, ObservatorySnapshot};

use super::{AuthoredEnvironment, AuthoredEnvironmentError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedPartBinding {
    pub part_id: String,
    pub host_id: HostId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentComparisonRow {
    ModeledOnly {
        part_id: String,
        expected_profile: String,
    },
    Matching {
        part_id: String,
        host_id: HostId,
        boot_id: String,
    },
    Discrepant {
        part_id: String,
        host_id: HostId,
        expected_profile: String,
        observed_profile: String,
    },
    ObservedOnly {
        host_id: HostId,
        boot_id: String,
        observed_profile: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentComparison {
    pub rows: Vec<EnvironmentComparisonRow>,
}

impl AuthoredEnvironment {
    pub fn compare_observed(
        &self,
        snapshot: &ObservatorySnapshot,
        bindings: &[ObservedPartBinding],
    ) -> Result<EnvironmentComparison, AuthoredEnvironmentError> {
        self.validate()?;
        let report =
            build_report(snapshot).map_err(AuthoredEnvironmentError::InvalidObservation)?;
        let mut part_ids = BTreeSet::new();
        let mut host_ids = BTreeSet::new();
        for binding in bindings {
            if !self
                .parts
                .iter()
                .any(|part| part.part_id == binding.part_id)
            {
                return Err(AuthoredEnvironmentError::UnknownBindingPart);
            }
            if !part_ids.insert(binding.part_id.as_str())
                || !host_ids.insert(binding.host_id.as_str())
            {
                return Err(AuthoredEnvironmentError::DuplicateBinding);
            }
        }
        let mut rows = Vec::with_capacity(self.parts.len() + report.hosts.len());
        for part in &self.parts {
            let binding = bindings
                .iter()
                .find(|binding| binding.part_id == part.part_id);
            let observed = binding.and_then(|binding| {
                report
                    .hosts
                    .iter()
                    .find(|host| host.host_id == binding.host_id)
            });
            match observed {
                None => rows.push(EnvironmentComparisonRow::ModeledOnly {
                    part_id: part.part_id.clone(),
                    expected_profile: part.profile.expected_observed_profile().into(),
                }),
                Some(host) if host.profile.as_str() == part.profile.expected_observed_profile() => {
                    rows.push(EnvironmentComparisonRow::Matching {
                        part_id: part.part_id.clone(),
                        host_id: host.host_id.clone(),
                        boot_id: host.boot_id.as_str().into(),
                    });
                }
                Some(host) => rows.push(EnvironmentComparisonRow::Discrepant {
                    part_id: part.part_id.clone(),
                    host_id: host.host_id.clone(),
                    expected_profile: part.profile.expected_observed_profile().into(),
                    observed_profile: host.profile.as_str().into(),
                }),
            }
        }
        for host in &report.hosts {
            if !bindings
                .iter()
                .any(|binding| binding.host_id == host.host_id)
            {
                rows.push(EnvironmentComparisonRow::ObservedOnly {
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.as_str().into(),
                    observed_profile: host.profile.as_str().into(),
                });
            }
        }
        Ok(EnvironmentComparison { rows })
    }
}
