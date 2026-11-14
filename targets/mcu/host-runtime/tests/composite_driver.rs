#![cfg(feature = "execution")]
//! Actual shared driver transitions under pressure/commit. CPU/no-heap proof,
//! not a physical sensor, prepared hardware ticket or whole Body admission.
#[allow(dead_code)]
#[path = "../../../std/src/allocation_probe.rs"]
mod allocation_probe;
use conduit_core::{
    kind_id, CheckedValueContract, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoValue, BOOL_INFO_ID,
};
use conduit_kernel::{scheduler::*, PortId, ValueRef};
use conduit_mcu_host_runtime::{execution_factory::Driver, execution_software::SoftwareBack};

#[test]
fn wrapped_terminal_state_waits_for_capacity_and_commits_without_play_allocations() {
    let boolean = StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap();
    let schema = StructuredInfoType::record(
        kind_id("fixture/terminal-state@1"),
        vec![StructuredFieldType::new("terminal", boolean.clone()).unwrap()],
    )
    .unwrap();
    let bytes = StructuredInfoValue::record(
        schema.clone(),
        vec![StructuredFieldValue::new(
            "terminal",
            StructuredInfoValue::leaf(boolean, vec![1]).unwrap(),
        )
        .unwrap()],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let maximum = u32::try_from(bytes.len()).unwrap();
    let contract = CheckedValueContract::new(
        schema.profile().unwrap().value_kind().clone(),
        maximum,
        vec![],
    )
    .unwrap();
    let mut driver: Driver<'_, HostCallBack> =
        Driver::Software(SoftwareBack::seeded_until(&contract, &schema).unwrap());
    let value = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: maximum,
    };
    let probe = allocation_probe::begin();
    for _ in 0..16 {
        let mut io = StepIo::test_frame([Some(value), None], [false; 2], [None; 2], None, 16);
        assert_eq!(
            driver.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&bytes), None], None)
            ),
            StepOutcome::Await
        );
        assert!(!io.test_consumed(PortId(0)));
    }
    // A discarded kernel step has no committed terminal effect.
    for _ in 0..2 {
        let mut io = StepIo::test_frame(
            [Some(value), None],
            [false; 2],
            [Some(maximum), None],
            None,
            16,
        );
        assert_eq!(
            driver.step(
                &mut io,
                &StepInputBytes::test_frame([Some(&bytes), None], None)
            ),
            StepOutcome::Progress
        );
        assert_eq!(io.test_output(PortId(0)), Some(value));
    }
    <Driver<HostCallBack> as StepBack<2>>::step_committed(&mut driver);
    let mut io = StepIo::test_frame([None; 2], [false; 2], [None; 2], None, 16);
    assert_eq!(
        driver.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
        StepOutcome::Complete
    );
    assert_eq!(io.test_output(PortId(0)), None);
    assert!(io.test_discards().contains(&Some(value)));
    assert_eq!(probe.finish(), 0);
}
