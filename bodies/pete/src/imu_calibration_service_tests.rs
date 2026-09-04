use super::*;

fn identities() -> (HostId, BootId) {
    (HostId::from("host/pico"), BootId::from("boot/pico"))
}

fn binding<'a>(host: &'a HostId, boot: &'a BootId) -> ImuCalibrationBinding<'a> {
    ImuCalibrationBinding {
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(2),
        implementation_id: crate::MPU6050_IMPLEMENTATION,
        i2c_base_id: "base/i2c",
        attachment_id: "attachment/imu",
        body_frame_id: "frame/body",
        mounting_id: "mounting/r23",
        current_calibration_generation: 4,
        maximum_sample_age_ticks: 100,
    }
}

fn authority<'a>(host: &'a HostId, boot: &'a BootId) -> ImuCalibrationAuthority<'a> {
    ImuCalibrationAuthority {
        grant_id: "grant/calibrate-imu",
        contract_id: IMU_CALIBRATION_AUTHORITY,
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(2),
        implementation_id: crate::MPU6050_IMPLEMENTATION,
        attachment_id: "attachment/imu",
        valid_until_tick: 200,
    }
}

fn request() -> ImuCalibrationRequest<'static> {
    ImuCalibrationRequest {
        request_id: "request/zero-imu",
        expected_calibration_generation: 4,
        deadline_tick: 150,
    }
}

#[test]
fn exact_authorized_stationary_sample_advances_generation() {
    let (host, boot) = identities();
    let sign = zero_imu_orientation(
        binding(&host, &boot),
        request(),
        Some(authority(&host, &boot)),
        RawImuSample::stationary(100),
        100,
    )
    .unwrap();
    assert_eq!(sign.prior_calibration_generation, 4);
    assert_eq!(sign.calibration.generation, 5);
    assert_eq!(sign.calibration.captured_at_tick, 100);
    assert_eq!(sign.body_frame_id, "frame/body");
}

#[test]
fn stale_moving_wrong_generation_and_authority_refuse_distinctly() {
    let (host, boot) = identities();
    assert_eq!(
        zero_imu_orientation(
            binding(&host, &boot),
            request(),
            Some(authority(&host, &boot)),
            RawImuSample::stationary(1),
            102
        ),
        Err(ImuCalibrationRefusal::StaleSample)
    );
    let moving = RawImuSample {
        gyro_z_milliradians_s: 51,
        ..RawImuSample::stationary(100)
    };
    assert_eq!(
        zero_imu_orientation(
            binding(&host, &boot),
            request(),
            Some(authority(&host, &boot)),
            moving,
            100
        ),
        Err(ImuCalibrationRefusal::BodyNotStationary)
    );
    let mut wrong_generation = request();
    wrong_generation.expected_calibration_generation = 3;
    assert_eq!(
        zero_imu_orientation(
            binding(&host, &boot),
            wrong_generation,
            Some(authority(&host, &boot)),
            RawImuSample::stationary(100),
            100
        ),
        Err(ImuCalibrationRefusal::CalibrationGenerationMismatch)
    );
    assert_eq!(
        zero_imu_orientation(
            binding(&host, &boot),
            request(),
            None,
            RawImuSample::stationary(100),
            100
        ),
        Err(ImuCalibrationRefusal::MissingAuthority)
    );
}

#[test]
fn every_exact_identity_is_bound() {
    let (host, boot) = identities();
    let wrong_host = HostId::from("host/wrong");
    let mut grant = authority(&host, &boot);
    grant.host_id = &wrong_host;
    assert_eq!(
        zero_imu_orientation(
            binding(&host, &boot),
            request(),
            Some(grant),
            RawImuSample::stationary(100),
            100
        ),
        Err(ImuCalibrationRefusal::HostMismatch)
    );
    let mut grant = authority(&host, &boot);
    grant.attachment_id = "attachment/wrong";
    assert_eq!(
        zero_imu_orientation(
            binding(&host, &boot),
            request(),
            Some(grant),
            RawImuSample::stationary(100),
            100
        ),
        Err(ImuCalibrationRefusal::AttachmentMismatch)
    );
}
