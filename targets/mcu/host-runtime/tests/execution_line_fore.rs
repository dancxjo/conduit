#![cfg(feature = "execution")]
//! Original PreparedFragment storage path, not live carrier or execution proof.
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::{scheduler::*, *};
use conduit_mcu_host_runtime::execution::{PreparedBacks, PreparedFragment, Refusal};
use conduit_mcu_host_runtime::execution_workset::PreparedLocalKernels;
use std::cell::Cell;
#[allow(dead_code)]
#[path = "../../../../architecture/body/tests/delegated_line_fore/support.rs"]
mod documents;
#[allow(dead_code)]
#[path = "execution_line_fore/support.rs"]
mod numeric;
use numeric::TablesFixture;
struct Back;
impl StepBack<1> for Back {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Await
    }
}
struct Backs<'a> {
    bound: &'a Cell<usize>,
    report: bool,
}
// SAFETY: inert CPU Back reports immutable empty terminal laws; bind only moves
// that exact Back and counts transfer. This fixture owns no native effects.
unsafe impl PreparedBacks<1, 1> for Backs<'_> {
    type Driver = Back;
    type Calls = ();
    fn terminal_transductions(&self) -> Option<[[Option<AssignedTerminalTransduction>; 1]; 1]> {
        self.report.then_some([[None]])
    }
    fn requires_terminal_gate(&self) -> bool {
        true
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    fn bind(self) -> ([Back; 1], ()) {
        self.bound.set(self.bound.get() + 1);
        ([Back], ())
    }
}
type Prepared<'a> = PreparedFragment<Backs<'a>, 1, 2, 1, 1, 1, 1, 16>;
fn values() -> HostedValueStore {
    HostedValueStore::new(4, 4, 16).unwrap()
}
fn signs() -> FixedSignLog<16> {
    FixedSignLog::new_with_remote_storage(
        (16 * size_of::<KernelEvent>()) as u32,
        8,
        remote_sign_storage_bytes(8).unwrap(),
    )
    .unwrap()
}
#[test]
fn explicit_constructor_reuses_original_funded_storage_and_legacy_door_refuses() {
    let native = documents::Fixture::new();
    let numeric = TablesFixture::new([true, true]);
    let document = numeric.document();
    let (approved, fore) = b::receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&documents::policy()),
        native.context(),
        documents::command(&native, &document),
        numeric.preparation(),
    )
    .unwrap();
    let bound = Cell::new(0);
    let factory = Cell::new(0);
    let prepared = Prepared::prepare_with_line_fore_terminal_transductions_ref(
        &approved,
        numeric.preparation(),
        &numeric.layout(),
        values(),
        signs(),
        65536,
        || {
            factory.set(factory.get() + 1);
            Ok::<_, ()>(Backs {
                bound: &bound,
                report: true,
            })
        },
        &[[None]],
    )
    .unwrap();
    assert!(prepared.retained_backing_bytes().unwrap() > 0);
    assert_eq!(factory.get(), 1);
    assert_eq!(bound.get(), 0);
    drop(prepared);
    assert_eq!(bound.get(), 0);
    let result = Prepared::prepare_with_terminal_transductions_ref(
        &approved,
        numeric.preparation().preparation,
        fore.as_slice(),
        &numeric.layout(),
        values(),
        signs(),
        65536,
        || {
            factory.set(factory.get() + 1);
            Ok::<_, ()>(Backs {
                bound: &bound,
                report: true,
            })
        },
        &[[None]],
    );
    assert!(matches!(result, Err(Refusal::Tables(_))));
    assert_eq!(factory.get(), 1);
}
#[test]
fn malformed_layout_or_line_pins_refuse_before_arbitrary_factory() {
    let native = documents::Fixture::new();
    let numeric = TablesFixture::new([true, true]);
    let document = numeric.document();
    let (approved, _) = b::receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&documents::policy()),
        native.context(),
        documents::command(&native, &document),
        numeric.preparation(),
    )
    .unwrap();
    let bound = Cell::new(0);
    let factory = Cell::new(0);
    let invalid = conduit_kernel::assigned::ResidentLayout {
        records: &[],
        counts: numeric.counts,
    };
    let result = Prepared::prepare_with_line_fore_terminal_transductions_ref(
        &approved,
        numeric.preparation(),
        &invalid,
        values(),
        signs(),
        65536,
        || {
            factory.set(factory.get() + 1);
            Ok::<_, ()>(Backs {
                bound: &bound,
                report: true,
            })
        },
        &[[None]],
    );
    assert!(matches!(result, Err(Refusal::Tables(_))));
    let mut changed = numeric.lines.clone();
    let first = changed[0];
    changed[0] = c::AssignedLineForeRequirement::new(
        *first.fore(),
        numeric.bindings[0],
        c::AssignedIdentity::from_text("foreign-session"),
        c::AssignedIdentity::from_text("session-profile"),
        [3; 32],
    );
    let mut profile = numeric.preparation();
    profile.line_fore.lines = &changed;
    let result = Prepared::prepare_with_line_fore_terminal_transductions_ref(
        &approved,
        profile,
        &numeric.layout(),
        values(),
        signs(),
        65536,
        || {
            factory.set(factory.get() + 1);
            Ok::<_, ()>(Backs {
                bound: &bound,
                report: true,
            })
        },
        &[[None]],
    );
    assert!(matches!(result, Err(Refusal::Tables(_))));
    assert_eq!(factory.get(), 0);
    assert_eq!(bound.get(), 0);
}
#[test]
fn actual_terminal_report_and_original_funding_still_gate_schema8_storage() {
    let native = documents::Fixture::new();
    let numeric = TablesFixture::new([true, true]);
    let document = numeric.document();
    let (approved, _) = b::receive_delegated_body_fragment_with_line_fore::<2>(
        Some(&documents::policy()),
        native.context(),
        documents::command(&native, &document),
        numeric.preparation(),
    )
    .unwrap();
    let bound = Cell::new(0);
    let result = Prepared::prepare_with_line_fore_terminal_transductions_ref(
        &approved,
        numeric.preparation(),
        &numeric.layout(),
        values(),
        signs(),
        65536,
        || {
            Ok::<_, ()>(Backs {
                bound: &bound,
                report: false,
            })
        },
        &[[None]],
    );
    assert!(matches!(result, Err(Refusal::TerminalContracts)));
    let expected = [[Some(AssignedTerminalTransduction {
        input: PortId(0),
        output: PortId(0),
        normal_close: AssignedNormalCloseTransduction::Consume,
        abnormal: AssignedAbnormalTransduction::NotAccepted,
        cancellation: AssignedCancellationTransduction::NotCancellable,
    })]];
    let result = Prepared::prepare_with_line_fore_terminal_transductions_ref(
        &approved,
        numeric.preparation(),
        &numeric.layout(),
        values(),
        signs(),
        65536,
        || {
            Ok::<_, ()>(Backs {
                bound: &bound,
                report: true,
            })
        },
        &expected,
    );
    assert!(matches!(
        result,
        Err(Refusal::Storage(KernelPreparationError::Topology(
            SchedulerError::InvalidPlan
        )))
    ));
    let result = Prepared::prepare_with_line_fore_terminal_transductions_ref(
        &approved,
        numeric.preparation(),
        &numeric.layout(),
        values(),
        signs(),
        0,
        || {
            Ok::<_, ()>(Backs {
                bound: &bound,
                report: true,
            })
        },
        &[[None]],
    );
    assert!(matches!(result, Err(Refusal::Bounds)));
    assert_eq!(bound.get(), 0);
}
