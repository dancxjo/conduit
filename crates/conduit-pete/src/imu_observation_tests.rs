use super::*;
use conduit_mpu6050::{I2cBaseAvailability, I2cProviderFailure, FRAME_BYTES};

struct Provider {
    available: bool,
    identity: u8,
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
            [0x75] => read.copy_from_slice(&[self.identity]),
            [0x3b] => read.copy_from_slice(&self.frame),
            _ => return Err(I2cProviderFailure::Read),
        }
        Ok(())
    }
}

fn evidence() -> Mpu6050Evidence {
    Mpu6050Evidence {
        host_id: HostId::from("host/pico-pete"),
        boot_id: BootId::from("boot/pico-pete"),
        offer_generation: OfferGeneration(7),
        i2c_base_id: "base/pico-i2c1-gp2-gp3".into(),
        attachment_id: "attachment/pete-mpu6050".into(),
        session_resource_id: "session/pete-mpu6050".into(),
        body_frame_id: "frame/pete-body-forward-left-up".into(),
        mounting_id: "mounting/pete-r23-mpu6050-v1".into(),
        address: 0x68,
        calibration: GravityCalibration::capture(3, RawImuSample::stationary(10)).unwrap(),
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
        identity: 0x68,
        frame,
    }
}

#[test]
fn unchanged_form_plans_exact_attachment_without_mechanism_vocabulary() {
    for forbidden in ["mpu", "i2c", "pico", "gpio", "uart"] {
        assert!(!MPU6050_FORM.to_ascii_lowercase().contains(forbidden));
    }
    let evidence = evidence();
    let plan = mpu6050_plan(&evidence).unwrap();
    validate_mpu6050_plan(&plan, &evidence).unwrap();
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.implementation_id.as_str() == MPU6050_IMPLEMENTATION)
        .unwrap();
    assert_eq!(placement.resources.len(), 3);
}

#[test]
fn raw_and_portable_truth_remain_distinct_and_safety_is_derived() {
    let evidence = evidence();
    let mut realization = Mpu6050Realization::new(&evidence).unwrap();
    let snapshot = realization
        .observe(&evidence, &mut provider(), 20, 20)
        .unwrap();
    assert_eq!(snapshot.raw, RawImuSample::stationary(20));
    assert_eq!(snapshot.orientation.components(), (0, 0, 0));
    assert_eq!(snapshot.derived.calibration_generation, 3);
    assert!(!snapshot.derived.tilt_active);
    assert!(!snapshot.derived.impact_active);
    assert_eq!(
        snapshot.local_safety_inputs(),
        (
            conduit_create_oi::SafetyInputObservation::Clear,
            conduit_create_oi::SafetyInputObservation::Clear
        )
    );
}

#[test]
fn attachment_absence_staleness_and_identity_mismatch_are_distinct() {
    let mut stale = evidence();
    assert_eq!(
        live_mpu6050_advertisement(&stale, 111),
        Err(Mpu6050OfferRefusal::StaleEvidence)
    );
    stale.i2c_base_id.clear();
    assert_eq!(
        live_mpu6050_advertisement(&stale, 10),
        Err(Mpu6050OfferRefusal::MissingIdentity)
    );

    let evidence = evidence();
    let mut realization = Mpu6050Realization::new(&evidence).unwrap();
    let mut absent = provider();
    absent.available = false;
    assert_eq!(
        realization.observe(&evidence, &mut absent, 20, 20),
        Err(Mpu6050ExecutionFailure::Device(
            Mpu6050Failure::I2cBaseUnavailable
        ))
    );

    let mut realization = Mpu6050Realization::new(&evidence).unwrap();
    let mut wrong = provider();
    wrong.identity = 0x70;
    assert_eq!(
        realization.observe(&evidence, &mut wrong, 20, 20),
        Err(Mpu6050ExecutionFailure::Device(
            Mpu6050Failure::IdentityMismatch { observed: 0x70 }
        ))
    );
}
