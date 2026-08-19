use super::*;
use crate::{UartParity, UartProfile};
use std::vec;
use std::vec::Vec;
#[derive(Default)]
struct Provider {
    writes: Vec<Vec<u8>>,
    fail_at: Option<usize>,
}
impl CreateUartProvider for Provider {
    type Error = ();

    fn is_available(&self) -> bool {
        true
    }

    fn profile(&self) -> UartProfile {
        UartProfile {
            baud: 57_600,
            data_bits: 8,
            stop_bits: 1,
            parity: UartParity::None,
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        if self.fail_at == Some(self.writes.len()) {
            self.writes.push(bytes.to_vec());
            return Err(());
        }
        self.writes.push(bytes.to_vec());
        Ok(())
    }

    fn read_byte(&mut self, _deadline_tick: u64) -> Result<Option<u8>, Self::Error> {
        Ok(None)
    }
}
fn frame(generation: u32, tick: u64, left: bool, right: bool) -> ContactFrame {
    ContactFrame {
        generation,
        observed_at_tick: tick,
        maximum_age_ticks: 10,
        left,
        right,
    }
}
fn forward() -> Option<ActiveWheelOutput> {
    Some(ActiveWheelOutput {
        command_generation: 7,
        left_mm_s: 100,
        right_mm_s: 100,
    })
}
#[test]
fn fresh_forward_edge_reverses_once_then_stops_by_time() {
    let mut provider = Provider::default();
    let mut reflex = LocalContactWithdrawal::new();
    assert_eq!(
        reflex.step(
            &mut provider,
            1,
            frame(1, 1, false, false),
            forward(),
            Some(0),
            WithdrawalInhibitors::default()
        ),
        None
    );
    assert!(matches!(
        reflex.step(
            &mut provider,
            2,
            frame(2, 2, true, false),
            forward(),
            Some(0),
            WithdrawalInhibitors::default()
        ),
        Some(ContactWithdrawalSign::Started {
            contact_side: ContactSide::Left,
            preempted_command_generation: 7,
            deadline_tick: 252,
            ..
        })
    ));
    assert_eq!(provider.writes, [vec![145, 0xff, 0xb0, 0xff, 0xb0]]);
    assert_eq!(
        reflex.step(
            &mut provider,
            251,
            frame(3, 251, true, false),
            None,
            Some(-19),
            WithdrawalInhibitors::default()
        ),
        None
    );
    assert!(matches!(
        reflex.step(
            &mut provider,
            252,
            frame(4, 252, true, false),
            None,
            Some(-19),
            WithdrawalInhibitors::default()
        ),
        Some(ContactWithdrawalSign::Completed {
            withdrawn_distance_mm: Some(19),
            ..
        })
    ));
    assert_eq!(provider.writes.last().unwrap(), &[145, 0, 0, 0, 0]);
    assert!(!reflex.is_active());
}

#[test]
fn distance_bound_stops_before_time_bound() {
    let mut provider = Provider::default();
    let mut reflex = LocalContactWithdrawal::new();
    reflex.step(
        &mut provider,
        1,
        frame(1, 1, false, false),
        forward(),
        Some(100),
        WithdrawalInhibitors::default(),
    );
    reflex.step(
        &mut provider,
        2,
        frame(2, 2, false, true),
        forward(),
        Some(100),
        WithdrawalInhibitors::default(),
    );
    assert!(matches!(
        reflex.step(
            &mut provider,
            10,
            frame(3, 10, false, true),
            None,
            Some(80),
            WithdrawalInhibitors::default()
        ),
        Some(ContactWithdrawalSign::Completed {
            withdrawn_distance_mm: Some(20),
            ..
        })
    ));
}

#[test]
fn baseline_stationary_reverse_stale_and_level_contacts_never_move() {
    for (baseline, next, output, now) in [
        (
            frame(1, 1, true, false),
            frame(2, 2, true, false),
            forward(),
            2,
        ),
        (frame(1, 1, false, false), frame(2, 2, true, false), None, 2),
        (
            frame(1, 1, false, false),
            frame(2, 2, true, false),
            Some(ActiveWheelOutput {
                command_generation: 8,
                left_mm_s: -20,
                right_mm_s: -20,
            }),
            2,
        ),
        (
            frame(1, 1, false, false),
            frame(2, 2, true, false),
            forward(),
            20,
        ),
    ] {
        let mut provider = Provider::default();
        let mut reflex = LocalContactWithdrawal::new();
        reflex.step(
            &mut provider,
            1,
            baseline,
            output,
            None,
            WithdrawalInhibitors::default(),
        );
        assert_eq!(
            reflex.step(
                &mut provider,
                now,
                next,
                output,
                None,
                WithdrawalInhibitors::default()
            ),
            None
        );
        assert!(provider.writes.is_empty());
    }
}

#[test]
fn host_loss_does_not_cancel_but_every_stronger_invariant_stops() {
    let cases = [
        (
            WithdrawalInhibitors {
                emergency_stop: true,
                ..Default::default()
            },
            WithdrawalPreemption::EmergencyStop,
        ),
        (
            WithdrawalInhibitors {
                wheel_drop: true,
                ..Default::default()
            },
            WithdrawalPreemption::WheelDrop,
        ),
        (
            WithdrawalInhibitors {
                cliff: true,
                ..Default::default()
            },
            WithdrawalPreemption::Cliff,
        ),
        (
            WithdrawalInhibitors {
                charging: true,
                ..Default::default()
            },
            WithdrawalPreemption::Charging,
        ),
        (
            WithdrawalInhibitors {
                explicitly_disarmed: true,
                ..Default::default()
            },
            WithdrawalPreemption::ExplicitDisarm,
        ),
        (
            WithdrawalInhibitors {
                tilt: true,
                ..Default::default()
            },
            WithdrawalPreemption::Tilt,
        ),
        (
            WithdrawalInhibitors {
                impact: true,
                ..Default::default()
            },
            WithdrawalPreemption::Impact,
        ),
        (
            WithdrawalInhibitors {
                motor_feedback_invalid: true,
                ..Default::default()
            },
            WithdrawalPreemption::MotorFeedbackInvalid,
        ),
        (
            WithdrawalInhibitors {
                create_feedback_lost: true,
                ..Default::default()
            },
            WithdrawalPreemption::CreateFeedbackLost,
        ),
        (
            WithdrawalInhibitors {
                watchdog_failed: true,
                ..Default::default()
            },
            WithdrawalPreemption::WatchdogFailed,
        ),
    ];
    for (inhibitors, cause) in cases {
        let mut provider = Provider::default();
        let mut reflex = LocalContactWithdrawal::new();
        reflex.step(
            &mut provider,
            1,
            frame(1, 1, false, false),
            forward(),
            None,
            WithdrawalInhibitors::default(),
        );
        reflex.step(
            &mut provider,
            2,
            frame(2, 2, true, false),
            forward(),
            None,
            WithdrawalInhibitors::default(),
        );
        // No active host output is supplied after trigger: ordinary host/LINE loss is inert.
        assert!(matches!(
            reflex.step(&mut provider, 3, frame(3, 3, true, false), None, None, inhibitors),
            Some(ContactWithdrawalSign::Preempted { cause: observed, stop_confirmed: true, .. }) if observed == cause
        ));
        assert_eq!(provider.writes.last().unwrap(), &[145, 0, 0, 0, 0]);
    }
}

#[test]
fn provider_failure_never_becomes_success_or_retry() {
    let mut provider = Provider {
        fail_at: Some(0),
        ..Default::default()
    };
    let mut reflex = LocalContactWithdrawal::new();
    reflex.step(
        &mut provider,
        1,
        frame(1, 1, false, false),
        forward(),
        None,
        WithdrawalInhibitors::default(),
    );
    assert!(matches!(
        reflex.step(
            &mut provider,
            2,
            frame(2, 2, true, false),
            forward(),
            None,
            WithdrawalInhibitors::default()
        ),
        Some(ContactWithdrawalSign::Preempted {
            cause: WithdrawalPreemption::ProviderFailure(_),
            stop_confirmed: true,
            ..
        })
    ));
    assert_eq!(provider.writes.len(), 2);
    assert!(!reflex.is_active());
}

#[test]
fn bilateral_contact_is_straight_and_repeated_level_never_retriggers() {
    let mut provider = Provider::default();
    let mut reflex = LocalContactWithdrawal::new();
    reflex.step(
        &mut provider,
        1,
        frame(1, 1, false, false),
        forward(),
        Some(0),
        WithdrawalInhibitors::default(),
    );
    assert!(matches!(
        reflex.step(
            &mut provider,
            2,
            frame(2, 2, true, true),
            forward(),
            Some(0),
            WithdrawalInhibitors::default()
        ),
        Some(ContactWithdrawalSign::Started {
            contact_side: ContactSide::Bilateral,
            ..
        })
    ));
    reflex.step(
        &mut provider,
        22,
        frame(3, 22, true, true),
        None,
        Some(-20),
        WithdrawalInhibitors::default(),
    );
    assert_eq!(provider.writes.len(), 2);
    assert_eq!(
        reflex.step(
            &mut provider,
            23,
            frame(4, 23, true, true),
            forward(),
            Some(-20),
            WithdrawalInhibitors::default()
        ),
        None
    );
    assert_eq!(provider.writes.len(), 2);
}

#[test]
fn stronger_truth_prevents_initial_withdrawal() {
    let cases = [
        WithdrawalInhibitors {
            emergency_stop: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            wheel_drop: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            cliff: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            charging: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            explicitly_disarmed: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            tilt: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            impact: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            motor_feedback_invalid: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            create_feedback_lost: true,
            ..Default::default()
        },
        WithdrawalInhibitors {
            watchdog_failed: true,
            ..Default::default()
        },
    ];
    for inhibitors in cases {
        let mut provider = Provider::default();
        let mut reflex = LocalContactWithdrawal::new();
        reflex.step(
            &mut provider,
            1,
            frame(1, 1, false, false),
            forward(),
            None,
            WithdrawalInhibitors::default(),
        );
        assert_eq!(
            reflex.step(
                &mut provider,
                2,
                frame(2, 2, true, false),
                forward(),
                None,
                inhibitors
            ),
            None
        );
        assert!(provider.writes.is_empty());
    }
}

#[test]
fn invalid_freshness_during_withdrawal_stops_with_typed_cause() {
    for (next, now, cause) in [
        (
            frame(2, 3, true, false),
            3,
            WithdrawalPreemption::ObservationGenerationRegressed,
        ),
        (
            frame(3, 4, true, false),
            3,
            WithdrawalPreemption::ObservationClockInvalid,
        ),
        (
            frame(3, 3, true, false),
            20,
            WithdrawalPreemption::ObservationStale,
        ),
    ] {
        let mut provider = Provider::default();
        let mut reflex = LocalContactWithdrawal::new();
        reflex.step(
            &mut provider,
            1,
            frame(1, 1, false, false),
            forward(),
            None,
            WithdrawalInhibitors::default(),
        );
        reflex.step(
            &mut provider,
            2,
            frame(2, 2, true, false),
            forward(),
            None,
            WithdrawalInhibitors::default(),
        );
        assert!(matches!(
            reflex.step(
                &mut provider,
                now,
                next,
                None,
                None,
                WithdrawalInhibitors::default()
            ),
            Some(ContactWithdrawalSign::Preempted { cause: observed, .. }) if observed == cause
        ));
    }
}
