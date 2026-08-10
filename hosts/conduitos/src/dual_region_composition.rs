//! One scheduler driving two admitted regions with an observable lifetime overlap.

use conduit_kernel::scheduler::SchedulerStatus;

use crate::{
    composition::{MachineProof, MachineRunError},
    dual_region_kernel::DualRegionKernel,
    machine::{IdleBase, InterruptBase, MonotonicClockBase, SerialBase, TimerBase},
};

const MAXIMUM_KERNEL_STEPS: u32 = 256;

pub fn run<C, T, S, I, D>(
    kernel: &mut DualRegionKernel,
    clock: &mut C,
    timer: &mut T,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<MachineProof, MachineRunError>
where
    C: MonotonicClockBase,
    T: TimerBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    let started = clock.now();
    let disabled_state = interrupts.disable();
    if interrupts.is_enabled() {
        return Err(MachineRunError::InterruptBaseFailure);
    }
    interrupts.restore(disabled_state);
    if disabled_state.enabled != interrupts.is_enabled() {
        return Err(MachineRunError::InterruptBaseFailure);
    }
    let _ = interrupts.disable();

    let mut timer_armed = false;
    let mut timer_completed = false;
    let mut overlap_witness = false;
    for _ in 0..MAXIMUM_KERNEL_STEPS {
        if overlap_witness
            && !timer_completed
            && let Some(interest) = timer
                .take_wake()
                .map_err(|_| MachineRunError::TimerBaseFailure)?
        {
            kernel
                .complete_timer(interest)
                .map_err(|_| MachineRunError::KernelFailure)?;
            timer_completed = true;
        }

        while let Some(request) = kernel.next_host_request() {
            if kernel.is_timer_request(&request) {
                let interest = kernel
                    .timer_interest(request)
                    .map_err(|_| MachineRunError::UnexpectedHostOperation)?;
                timer
                    .arm(interest)
                    .map_err(|_| MachineRunError::TimerBaseFailure)?;
                timer_armed = true;
                continue;
            }
            if kernel.is_upper_request(&request) {
                let output = {
                    let value = kernel
                        .host_value(request.input.value)
                        .map_err(|_| MachineRunError::KernelFailure)?;
                    crate::text_upper::uppercase(value).map_err(|error| match error {
                        crate::text_upper::UppercaseError::MalformedUtf8 => {
                            MachineRunError::TextMalformedUtf8
                        }
                        crate::text_upper::UppercaseError::OutputOverflow => {
                            MachineRunError::TextOutputOverflow
                        }
                    })?
                };
                kernel
                    .complete_upper(request, output.as_bytes())
                    .map_err(|_| MachineRunError::KernelFailure)?;
                continue;
            }
            if kernel.is_text_presentation_request(&request) {
                let value = kernel
                    .host_value(request.input.value)
                    .map_err(|_| MachineRunError::KernelFailure)?;
                core::str::from_utf8(value).map_err(|_| MachineRunError::SerialBaseFailure)?;
                serial
                    .present(value)
                    .map_err(|_| MachineRunError::SerialBaseFailure)?;
                kernel
                    .complete_presentation(request)
                    .map_err(|_| MachineRunError::KernelFailure)?;
                overlap_witness = timer_armed && !timer_completed;
                continue;
            }
            if kernel.is_tick_presentation_request(&request) {
                let value = kernel
                    .host_value(request.input.value)
                    .map_err(|_| MachineRunError::KernelFailure)?;
                if value.len() != conduit_std_catalog::TICK_ENCODED_LEN as usize {
                    return Err(MachineRunError::SerialBaseFailure);
                }
                serial
                    .present(value)
                    .map_err(|_| MachineRunError::SerialBaseFailure)?;
                kernel
                    .complete_presentation(request)
                    .map_err(|_| MachineRunError::KernelFailure)?;
                continue;
            }
            return Err(MachineRunError::UnexpectedHostOperation);
        }

        match kernel.step().map_err(|_| MachineRunError::KernelFailure)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => {
                if kernel.pending_host_operations() == 0 {
                    return Err(MachineRunError::FalseIdle);
                }
                idle.wait_for_interrupt()
                    .map_err(|_| MachineRunError::InterruptBaseFailure)?;
            }
            SchedulerStatus::Complete => {
                if !overlap_witness || !timer_completed {
                    return Err(MachineRunError::KernelFailure);
                }
                return Ok(MachineProof {
                    logical_operations: 5,
                    decisions: kernel.decisions(),
                    kernel_signs: kernel.sign_count(),
                    timer_irq_wakes: timer.wake_count(),
                    idle_entries: idle.idle_count(),
                    serial_presentations: serial.presentation_count(),
                    clock_monotonic: clock.now() >= started,
                    pending_host_operations: kernel.pending_host_operations() as u8,
                    overlap_witness,
                    timer_pending_during_text_progress: overlap_witness,
                    physical_parallelism: false,
                });
            }
            SchedulerStatus::Cancelled => return Err(MachineRunError::KernelFailure),
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::BootIdentities,
        machine::{BaseError, InterruptState, KernelInterest, TimerToken},
        offer::{CpuFeatures, HostOffer},
    };

    #[derive(Default)]
    struct Clock(u64);
    impl MonotonicClockBase for Clock {
        fn now(&mut self) -> u64 {
            self.0 += 1;
            self.0
        }
    }

    #[derive(Default)]
    struct Timer {
        interest: Option<KernelInterest>,
        wakes: u32,
        failed: bool,
    }
    impl TimerBase for Timer {
        fn arm(&mut self, interest: KernelInterest) -> Result<TimerToken, BaseError> {
            if self.failed {
                return Err(BaseError::Unavailable);
            }
            if self.interest.replace(interest).is_some() {
                return Err(BaseError::SlotFull);
            }
            Ok(TimerToken {
                slot: 0,
                generation: 1,
            })
        }
        fn cancel(&mut self, _token: TimerToken) -> Result<KernelInterest, BaseError> {
            self.interest.take().ok_or(BaseError::StaleWake)
        }
        fn take_wake(&mut self) -> Result<Option<KernelInterest>, BaseError> {
            let interest = self.interest.take();
            if interest.is_some() {
                self.wakes += 1;
            }
            Ok(interest)
        }
        fn wake_count(&self) -> u32 {
            self.wakes
        }
    }

    #[derive(Default)]
    struct Serial {
        values: alloc::vec::Vec<alloc::vec::Vec<u8>>,
        failed: bool,
    }
    impl SerialBase for Serial {
        fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
            if self.failed {
                return Err(BaseError::Unavailable);
            }
            self.values.push(bytes.to_vec());
            Ok(())
        }
        fn presentation_count(&self) -> u32 {
            self.values.len() as u32
        }
    }

    #[derive(Default)]
    struct Interrupts(bool);
    impl InterruptBase for Interrupts {
        fn enable(&mut self) {
            self.0 = true;
        }
        fn disable(&mut self) -> InterruptState {
            let state = InterruptState { enabled: self.0 };
            self.0 = false;
            state
        }
        fn restore(&mut self, state: InterruptState) {
            self.0 = state.enabled;
        }
        fn is_enabled(&self) -> bool {
            self.0
        }
    }

    #[derive(Default)]
    struct Idle(u32);
    impl IdleBase for Idle {
        fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
            self.0 += 1;
            Ok(())
        }
        fn idle_count(&self) -> u32 {
            self.0
        }
    }

    #[test]
    fn text_reaches_presentation_while_timer_region_is_outstanding() {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            512 * 1024,
        );
        let mut prepared = crate::dual_region_plan::prepare(&identities, &offer, "build").unwrap();
        let mut clock = Clock::default();
        let mut timer = Timer::default();
        let mut serial = Serial::default();
        let mut interrupts = Interrupts::default();
        let mut idle = Idle::default();
        let proof = run(
            &mut prepared.kernel,
            &mut clock,
            &mut timer,
            &mut serial,
            &mut interrupts,
            &mut idle,
        )
        .unwrap();

        assert!(proof.overlap_witness);
        assert!(proof.timer_pending_during_text_progress);
        assert!(!proof.physical_parallelism);
        assert_eq!(proof.logical_operations, 5);
        assert_eq!(proof.timer_irq_wakes, 1);
        assert_eq!(proof.serial_presentations, 2);
        assert_eq!(
            serial.values[0],
            crate::dual_region_plan::TEXT_RESULT.as_bytes()
        );
        assert_eq!(
            serial.values[1].len(),
            conduit_std_catalog::TICK_ENCODED_LEN as usize
        );
    }

    #[test]
    fn timer_and_presentation_base_loss_remain_distinct() {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            512 * 1024,
        );
        let run_with = |timer: &mut Timer, serial: &mut Serial| {
            let mut prepared =
                crate::dual_region_plan::prepare(&identities, &offer, "build").unwrap();
            run(
                &mut prepared.kernel,
                &mut Clock::default(),
                timer,
                serial,
                &mut Interrupts::default(),
                &mut Idle::default(),
            )
        };
        assert_eq!(
            run_with(
                &mut Timer {
                    failed: true,
                    ..Timer::default()
                },
                &mut Serial::default()
            ),
            Err(MachineRunError::TimerBaseFailure)
        );
        assert_eq!(
            run_with(
                &mut Timer::default(),
                &mut Serial {
                    failed: true,
                    ..Serial::default()
                }
            ),
            Err(MachineRunError::SerialBaseFailure)
        );
    }
}
