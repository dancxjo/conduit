use super::*;
use crate::{mpu6050_plan, Mpu6050Evidence};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_mpu6050::{
    GravityCalibration, I2cBaseAvailability, I2cProviderFailure, ImuThresholds, Mpu6050Failure,
    RawImuSample, FRAME_BYTES,
};

struct Provider {
    available: bool,
    frame: [u8; FRAME_BYTES],
}

impl Mpu6050I2cProvider for Provider {
    fn availability(&self) -> I2cBaseAvailability {
        if self.available {
            I2cBaseAvailability::Available
        } else {
            I2cBaseAvailability::Unavailable
        }
    }
    fn write(&mut self, _address: u8, _bytes: &[u8]) -> Result<(), I2cProviderFailure> {
        Ok(())
    }
    fn write_read(
        &mut self,
        _address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), I2cProviderFailure> {
        match write {
            [0x75] => read.copy_from_slice(&[0x68]),
            [0x3b] => read.copy_from_slice(&self.frame),
            _ => return Err(I2cProviderFailure::Read),
        }
        Ok(())
    }
}

fn evidence() -> Mpu6050Evidence {
    Mpu6050Evidence {
        host_id: HostId::from("host/pico"),
        boot_id: BootId::from("boot/pico"),
        offer_generation: OfferGeneration(1),
        i2c_base_id: "base/i2c".into(),
        attachment_id: "attachment/imu".into(),
        session_resource_id: "session/imu".into(),
        body_frame_id: "frame/body".into(),
        mounting_id: "mounting/r23".into(),
        address: 0x68,
        calibration: GravityCalibration::capture(1, RawImuSample::stationary(10)).unwrap(),
        thresholds: ImuThresholds {
            tilt_stop_microradians: 650_000,
            impact_stop_mm_s2: 18_000,
            maximum_sample_age_ticks: 100,
        },
        observed_at_tick: 10,
    }
}

fn provider() -> Provider {
    let mut frame = [0_u8; FRAME_BYTES];
    frame[4..6].copy_from_slice(&16_384_i16.to_be_bytes());
    Provider {
        available: true,
        frame,
    }
}

#[test]
fn planned_orientation_runs_through_production_kernel() {
    let evidence = evidence();
    let plan = mpu6050_plan(&evidence).unwrap();
    let mut execution = prepare_mpu6050_execution(&plan, &evidence).unwrap();
    let report = run_mpu6050_execution(&mut execution, &mut provider(), 20, 20);
    assert_eq!(report.terminal, Mpu6050Terminal::Completed);
    assert_eq!(report.orientation.unwrap().components(), (0, 0, 0));
    assert_eq!(report.raw, Some(RawImuSample::stationary(20)));
    assert!(report.kernel_decisions > 0);
    assert!(report.kernel_signs > 0);
    assert!(!report.physical_hil_claimed);
}

#[test]
fn cancellation_provider_loss_and_wrong_plan_remain_distinct() {
    let evidence = evidence();
    let plan = mpu6050_plan(&evidence).unwrap();
    let mut cancelled = prepare_mpu6050_execution(&plan, &evidence).unwrap();
    assert_eq!(
        cancel_mpu6050_execution(&mut cancelled).terminal,
        Mpu6050Terminal::CancelledBeforeDispatch
    );
    let mut cancelled_after = prepare_mpu6050_execution(&plan, &evidence).unwrap();
    dispatch_mpu6050_execution(&mut cancelled_after, &mut provider(), 20, 20).unwrap();
    assert_eq!(
        cancel_mpu6050_execution(&mut cancelled_after).terminal,
        Mpu6050Terminal::CancelledAfterDispatch
    );

    let mut lost = prepare_mpu6050_execution(&plan, &evidence).unwrap();
    let mut unavailable = provider();
    unavailable.available = false;
    assert_eq!(
        run_mpu6050_execution(&mut lost, &mut unavailable, 20, 20).terminal,
        Mpu6050Terminal::Failed(Mpu6050PlayFailure::DeviceOrDerivation(
            Mpu6050ExecutionFailure::Device(Mpu6050Failure::I2cBaseUnavailable)
        ))
    );

    let mut wrong = plan.clone();
    wrong.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.implementation_id.as_str() == crate::MPU6050_IMPLEMENTATION)
        .unwrap()
        .boot_id = BootId::from("boot/wrong");
    assert_eq!(
        prepare_mpu6050_execution(&wrong, &evidence).err(),
        Some("Plan does not seal the exact MPU-6050 realization")
    );
    let mut pressure = plan.clone();
    pressure.fragments[0]
        .placements
        .iter_mut()
        .find(|placement| placement.implementation_id.as_str() == crate::MPU6050_IMPLEMENTATION)
        .unwrap()
        .host_operations[0]
        .maximum_output_bytes = 1;
    assert_eq!(
        prepare_mpu6050_execution(&pressure, &evidence).err(),
        Some("Plan does not seal the exact MPU-6050 realization")
    );
}
