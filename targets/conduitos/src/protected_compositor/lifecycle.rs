//! Finite read-only observations, never an admission or a second cost ledger.
//! Costs remain cumulative backend snapshots; absent costs are not zero costs.
use crate::protected_region::DomainCost;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Sample {
    pub before: Option<DomainCost>,
    pub after: Option<DomainCost>,
    /// Transport/activation completed; does not imply a semantic Show acknowledgement.
    pub succeeded: bool,
    /// A well-formed guest command refusal is distinct from transport failure.
    pub refused: bool,
}

/// Last sample plus a checked count. No unbounded history or hot-path allocation.
/// Overflow makes the observation incomplete without changing service behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Phase {
    pub count: u64,
    pub overflowed: bool,
    pub last: Option<Sample>,
}
impl Phase {
    pub fn record(&mut self, sample: Sample) {
        match self.count.checked_add(1) {
            Some(count) => self.count = count,
            None => self.overflowed = true,
        }
        self.last = Some(sample);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ServiceObservation {
    pub instance: u64,
    /// Installation has finished, but Initialize has not entered the guest.
    pub installed: DomainCost,
    pub initialization: Option<Sample>,
    pub requests: Phase,
    /// Recorded once when the live backend is taken, including fault disposal.
    pub disposal: Option<Sample>,
}
impl ServiceObservation {
    pub fn installed(instance: u64, cost: DomainCost) -> Self {
        Self {
            instance,
            installed: cost,
            initialization: None,
            requests: Phase::default(),
            disposal: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ActivationObservation {
    pub attempts: Phase,
    pub instance_before: u64,
    pub instance_after: u64,
    pub namespace_before: u64,
    pub namespace_after: u64,
}

/// Root staging and guest private storage have distinct owners and teardown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DisplayObservation {
    pub service: ServiceObservation,
    pub activations: ActivationObservation,
    pub current_cost: DomainCost,
    pub staging_prepared_bytes: usize,
    pub staging_live_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_is_explicit_and_keeps_last_sample_without_wrapping() {
        let mut phase = Phase {
            count: u64::MAX,
            ..Phase::default()
        };
        let sample = Sample {
            before: None,
            after: Some(DomainCost::default()),
            succeeded: false,
            refused: false,
        };
        phase.record(sample);
        assert_eq!(phase.count, u64::MAX);
        assert!(phase.overflowed);
        assert_eq!(phase.last, Some(sample));
        assert_ne!(sample.before, sample.after);
    }
    #[test]
    fn installation_does_not_invent_requests_or_disposal() {
        let cost = DomainCost {
            setup_ticks: 7,
            reserved_bytes: 4096,
            ..DomainCost::default()
        };
        let observed = ServiceObservation::installed(42, cost);
        assert_eq!(observed.installed, cost);
        assert_eq!(observed.instance, 42);
        assert_eq!(observed.requests, Phase::default());
        assert_eq!(observed.initialization, None);
        assert_eq!(observed.disposal, None);
    }
}
