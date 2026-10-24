//! Differential execution against the unchanged ordinary kernel/retention log.
use super::execution::run_output_mode_with_inspection;
use conduit_environment_runtime::display_output::{
    DisplaySignBudget, PreparedDisplayKernel, SignBudgetRefusal,
};
use conduit_kernel::{self as k, SignQuery, SignSink};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry(k::KernelEvent, Option<k::RemoteLifecycleIdentity>);
struct Audit {
    inner: k::HostedSignLog,
    trace: Vec<Entry>,
    largest_admission: Cell<u16>,
}
impl SignSink for Audit {
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }
    fn len(&self) -> u16 {
        self.inner.len()
    }
    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }
    fn ensure_capacity(&self, count: u16) -> Result<(), k::SignError> {
        self.largest_admission
            .set(self.largest_admission.get().max(count));
        self.inner.ensure_capacity(count)
    }
    fn ensure_remote_capacity(&self, count: u16) -> Result<(), k::SignError> {
        self.inner.ensure_remote_capacity(count)
    }
    fn record(
        &mut self,
        node: k::NodeId,
        port: Option<k::PortId>,
        request: Option<k::RequestId>,
        kind: k::KernelEventKind,
    ) -> Result<k::KernelEvent, k::SignError> {
        let event = self.inner.record(node, port, request, kind)?;
        assert!(self.trace.len() < self.trace.capacity());
        self.trace.push(Entry(event, None));
        Ok(event)
    }
    fn record_remote(
        &mut self,
        node: k::NodeId,
        port: k::PortId,
        kind: k::KernelEventKind,
        remote: k::RemoteLifecycleIdentity,
    ) -> Result<k::KernelEvent, k::SignError> {
        let event = self.inner.record_remote(node, port, kind, remote)?;
        assert!(self.trace.len() < self.trace.capacity());
        self.trace.push(Entry(event, Some(remote)));
        Ok(event)
    }
}
struct Evidence {
    trace: Vec<Entry>,
    retained: Vec<Entry>,
    gap: Option<k::SignRetentionGap>,
    largest_admission: u16,
}
fn run(mode: u8, reduced: bool) -> Evidence {
    run_output_mode_with_inspection(
        mode,
        false,
        |prepared, software| {
            let budget = DisplaySignBudget::derive(
                2,
                prepared.lowered().sign_items,
                prepared.lowered().sign_bytes,
            )
            .unwrap();
            let inner = if reduced {
                k::HostedSignLog::new_with_remote_storage(
                    budget.ordinary_items(),
                    budget.ordinary_bytes(),
                    budget.remote_items(),
                    budget.remote_bytes(),
                )
                .unwrap()
            } else {
                k::HostedSignLog::new_with_remote_storage(
                    256,
                    256 * core::mem::size_of::<k::KernelEvent>() as u32,
                    256,
                    k::remote_sign_storage_bytes(256).unwrap(),
                )
                .unwrap()
            };
            PreparedDisplayKernel::prepare_with_storage(
                prepared.plan(),
                prepared.lowered(),
                software,
                2,
                k::HostedValueStore::new(12, 1024, 12 * 1024).unwrap(),
                Audit {
                    inner,
                    trace: Vec::with_capacity(512),
                    largest_admission: Cell::new(0),
                },
            )
            .unwrap()
        },
        |audit| Evidence {
            trace: audit.trace.clone(),
            retained: audit
                .inner
                .events()
                .map(|event| Entry(event, audit.inner.remote_identity(event.sequence)))
                .collect(),
            gap: audit.inner.retention_gap(),
            largest_admission: audit.largest_admission.get(),
        },
    )
}

#[test]
fn derived_logs_preserve_effects_complete_event_trace_and_mandatory_evidence() {
    for mode in 0..5 {
        let baseline = run(mode, false);
        let reduced = run(mode, true);
        assert_eq!(reduced.trace, baseline.trace);
        // Actual preflight work stays within the conservative profile burst.
        assert!(reduced.largest_admission <= 18);
        for entry in &baseline.trace {
            // Remote identities and the profile's terminal/fault evidence must
            // survive; optional telemetry may instead have an explicit gap.
            if entry.1.is_some()
                || matches!(
                    entry.0.kind,
                    k::KernelEventKind::BackCompleted
                        | k::KernelEventKind::BackFailed
                        | k::KernelEventKind::StepFuelExceeded
                        | k::KernelEventKind::RunCancelled
                        | k::KernelEventKind::SemanticAbnormal
                        | k::KernelEventKind::SemanticAbnormalRecovered
                )
            {
                assert!(
                    reduced.retained.contains(entry),
                    "lost required evidence {entry:?}"
                );
            }
        }
        if mode == 0 || mode == 3 {
            assert!(reduced.gap.is_some());
        }
    }
}

#[test]
fn budget_derivation_is_finite_and_includes_the_declared_plan_minimum() {
    assert_eq!(
        DisplaySignBudget::derive(0, 13, 717),
        Err(SignBudgetRefusal::ZeroWork)
    );
    assert_eq!(
        DisplaySignBudget::derive(2, 0, 717),
        Err(SignBudgetRefusal::InvalidMinimum)
    );
    assert_eq!(
        DisplaySignBudget::derive(u32::MAX, 13, 717),
        Err(SignBudgetRefusal::Overflow)
    );
    assert_eq!(
        DisplaySignBudget::derive(2, 13, u32::MAX),
        Err(SignBudgetRefusal::Overflow)
    );
    let budget = DisplaySignBudget::derive(2, 13, 717).unwrap();
    assert_eq!(budget.remote_items(), 10);
    assert!(budget.ordinary_items() >= 13 && budget.ordinary_bytes() >= 717);
    assert!(
        budget.storage_bytes().unwrap()
            < k::HostedSignLog::canonical_storage_bytes(256, 256).unwrap()
    );
    eprintln!(
        "display_signs ordinary={} remote={} bytes={} previous_bytes={}",
        budget.ordinary_items(),
        budget.remote_items(),
        budget.storage_bytes().unwrap(),
        k::HostedSignLog::canonical_storage_bytes(256, 256).unwrap()
    );
}
