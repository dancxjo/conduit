//! Bounded Host completion observations, distinct from the kernel event log.
//! Exact accepted outcomes are retained; overwritten observations are counted.
use conduit_kernel::{HostOperationDisposition, HostOperationOutcome, NodeId, RequestId};
use serde::Serialize;

const CAPACITY: usize = 64;
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct HostOutcomeRecord {
    sequence: u64,
    node: u16,
    request: u32,
    disposition: &'static str,
    failure_code: Option<&'static str>,
    failure_detail: Option<u16>,
}
#[derive(Debug, Serialize)]
pub(super) struct HostOutcomeEvidence {
    schema: &'static str,
    capacity: usize,
    omitted: u64,
    records: Vec<HostOutcomeRecord>,
}
pub(super) struct HostOutcomes {
    records: Box<[Option<HostOutcomeRecord>; CAPACITY]>,
    next: u64,
}
impl HostOutcomes {
    pub(super) fn new() -> Self {
        Self {
            records: Box::new([None; CAPACITY]),
            next: 0,
        }
    }
    pub(super) fn check_capacity(&self) -> Result<(), String> {
        self.next
            .checked_add(1)
            .map(|_| ())
            .ok_or_else(|| "Host outcome identity exhausted".into())
    }
    /// Call only after the kernel accepts this exact correlated completion.
    pub(super) fn record(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostOperationOutcome,
    ) {
        let record = HostOutcomeRecord {
            sequence: self.next,
            node: node.0,
            request: request.0,
            disposition: match outcome.disposition {
                HostOperationDisposition::Completed => "completed",
                HostOperationDisposition::Denied => "denied",
                HostOperationDisposition::Failed => "failed",
                HostOperationDisposition::Cancelled => "cancelled",
            },
            failure_code: outcome.failure.map(|failure| failure.code.as_str()),
            failure_detail: outcome.failure.map(|failure| failure.detail),
        };
        self.records[(self.next % CAPACITY as u64) as usize] = Some(record);
        self.next += 1;
    }
    pub(super) fn evidence(&self) -> HostOutcomeEvidence {
        let first = self.next.saturating_sub(CAPACITY as u64);
        HostOutcomeEvidence {
            schema: "conduit.browser/host-completion-evidence@1",
            capacity: CAPACITY,
            omitted: first,
            records: (first..self.next)
                .map(|sequence| self.records[(sequence % CAPACITY as u64) as usize].unwrap())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_outcomes_keep_denial_distinct_and_report_exact_overwrite() {
        let mut log = HostOutcomes::new();
        let capacity = log.records.len();
        for request in 0..70 {
            log.check_capacity().unwrap();
            log.record(
                NodeId(2),
                RequestId(request),
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Denied,
                    output: None,
                    failure: Some(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::HostOperationDenied,
                        detail: 7,
                    }),
                },
            );
        }
        let evidence = log.evidence();
        assert_eq!(log.records.len(), capacity);
        assert_eq!(evidence.omitted, 6);
        assert_eq!(evidence.records.len(), 64);
        assert_eq!(evidence.records[0].request, 6);
        assert_eq!(evidence.records[63].request, 69);
        assert!(evidence
            .records
            .iter()
            .all(|record| record.disposition == "denied"
                && record.failure_code == Some("host_operation_denied")
                && record.failure_detail == Some(7)));
        log.next = u64::MAX;
        assert!(log.check_capacity().is_err());
    }
}
