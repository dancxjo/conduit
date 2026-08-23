//! Continuous, bounded MPU-6050 ownership for the Pete carrier.

use conduit_mpu6050::{
    GravityCalibration, I2cBaseAvailability, I2cProviderFailure, ImuDeriver, ImuThresholds,
    Mpu6050Failure, Mpu6050I2cProvider, Mpu6050Session, RawImuSample, ALTERNATE_ADDRESS,
    DEFAULT_ADDRESS,
};
use embassy_rp::i2c::{Blocking, Config as I2cConfig, I2c};
use embassy_rp::peripherals::{I2C1, PIN_2, PIN_3};
use embassy_rp::Peri;
use embassy_time::{Duration, Instant, Timer};
use portable_atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU8, Ordering};

const POLL_MS: u64 = 20;
const RETRY_MS: u64 = 250;
const CALIBRATION_SAMPLES: u32 = 50;
const MINIMUM_GRAVITY_MM_S2: u16 = 7_000;
const MAXIMUM_GRAVITY_MM_S2: u16 = 13_000;
const MAXIMUM_STATIONARY_GYRO_MRAD_S: i16 = 250;
const THRESHOLDS: ImuThresholds = ImuThresholds {
    tilt_stop_microradians: 650_000,
    impact_stop_mm_s2: 18_000,
    maximum_sample_age_ticks: 100,
};

type BoardI2c = I2c<'static, I2C1, Blocking>;

struct Provider(BoardI2c);

impl Mpu6050I2cProvider for Provider {
    fn availability(&self) -> I2cBaseAvailability {
        I2cBaseAvailability::Available
    }

    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), I2cProviderFailure> {
        self.0
            .blocking_write(address, bytes)
            .map_err(|_| I2cProviderFailure::Write)
    }

    fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), I2cProviderFailure> {
        self.0
            .blocking_write_read(address, write, read)
            .map_err(|_| I2cProviderFailure::Read)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum State {
    Probing = 0,
    Calibrating = 1,
    Healthy = 2,
    Fault = 3,
}

impl State {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Probing => "probing",
            Self::Calibrating => "calibrating",
            Self::Healthy => "healthy",
            Self::Fault => "fault",
        }
    }

    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Calibrating,
            2 => Self::Healthy,
            3 => Self::Fault,
            _ => Self::Probing,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Snapshot {
    pub state: State,
    pub address: u8,
    pub samples: u32,
    pub observed_at_ms: u32,
    pub accel_x_mm_s2: i16,
    pub accel_y_mm_s2: i16,
    pub accel_z_mm_s2: i16,
    pub gyro_x_milliradians_s: i16,
    pub gyro_y_milliradians_s: i16,
    pub gyro_z_milliradians_s: i16,
    pub tilt_active: bool,
    pub impact_active: bool,
    pub calibration_generation: u32,
    pub failure: u8,
}

static STATE: AtomicU8 = AtomicU8::new(State::Probing as u8);
static ADDRESS: AtomicU8 = AtomicU8::new(0);
static SAMPLES: AtomicU32 = AtomicU32::new(0);
static OBSERVED_AT_MS: AtomicU32 = AtomicU32::new(0);
static ACCEL_X: AtomicI32 = AtomicI32::new(0);
static ACCEL_Y: AtomicI32 = AtomicI32::new(0);
static ACCEL_Z: AtomicI32 = AtomicI32::new(0);
static GYRO_X: AtomicI32 = AtomicI32::new(0);
static GYRO_Y: AtomicI32 = AtomicI32::new(0);
static GYRO_Z: AtomicI32 = AtomicI32::new(0);
static TILT_ACTIVE: AtomicBool = AtomicBool::new(false);
static IMPACT_ACTIVE: AtomicBool = AtomicBool::new(false);
static USB_CONNECTION_READY: AtomicBool = AtomicBool::new(false);
static CALIBRATION_GENERATION: AtomicU32 = AtomicU32::new(0);
static FAILURE: AtomicU8 = AtomicU8::new(0);

/// Allow the potentially blocking physical I2C probe only after USB has
/// enumerated and the exact firmware identity has crossed CDC.
pub fn permit_probe_after_usb_identity() {
    USB_CONNECTION_READY.store(true, Ordering::Release);
}

pub fn snapshot() -> Snapshot {
    Snapshot {
        state: State::from_raw(STATE.load(Ordering::Acquire)),
        address: ADDRESS.load(Ordering::Acquire),
        samples: SAMPLES.load(Ordering::Acquire),
        observed_at_ms: OBSERVED_AT_MS.load(Ordering::Acquire),
        accel_x_mm_s2: ACCEL_X.load(Ordering::Acquire) as i16,
        accel_y_mm_s2: ACCEL_Y.load(Ordering::Acquire) as i16,
        accel_z_mm_s2: ACCEL_Z.load(Ordering::Acquire) as i16,
        gyro_x_milliradians_s: GYRO_X.load(Ordering::Acquire) as i16,
        gyro_y_milliradians_s: GYRO_Y.load(Ordering::Acquire) as i16,
        gyro_z_milliradians_s: GYRO_Z.load(Ordering::Acquire) as i16,
        tilt_active: TILT_ACTIVE.load(Ordering::Acquire),
        impact_active: IMPACT_ACTIVE.load(Ordering::Acquire),
        calibration_generation: CALIBRATION_GENERATION.load(Ordering::Acquire),
        failure: FAILURE.load(Ordering::Acquire),
    }
}

pub fn is_fresh(snapshot: &Snapshot, now_ms: u32) -> bool {
    snapshot.state == State::Healthy
        && snapshot.samples > 0
        && now_ms.wrapping_sub(snapshot.observed_at_ms) <= THRESHOLDS.maximum_sample_age_ticks as u32
}

pub const fn failure_name(code: u8) -> &'static str {
    match code {
        1 => "invalid_address",
        2 => "i2c_base_unavailable",
        3 => "device_no_response",
        4 => "identity_mismatch",
        5 => "wake_write_failed",
        6 => "gyro_config_write_failed",
        7 => "accel_config_write_failed",
        8 => "frame_read_failed",
        9 => "clock_regressed",
        _ => "none",
    }
}

fn failure_code(failure: Mpu6050Failure) -> u8 {
    match failure {
        Mpu6050Failure::InvalidAddress => 1,
        Mpu6050Failure::I2cBaseUnavailable => 2,
        Mpu6050Failure::DeviceNoResponse => 3,
        Mpu6050Failure::IdentityMismatch { .. } => 4,
        Mpu6050Failure::WakeWriteFailed => 5,
        Mpu6050Failure::GyroConfigWriteFailed => 6,
        Mpu6050Failure::AccelConfigWriteFailed => 7,
        Mpu6050Failure::FrameReadFailed => 8,
        Mpu6050Failure::ClockRegressed => 9,
    }
}

fn magnitude(sample: RawImuSample) -> u16 {
    let squared = i64::from(sample.accel_x_mm_s2).pow(2)
        + i64::from(sample.accel_y_mm_s2).pow(2)
        + i64::from(sample.accel_z_mm_s2).pow(2);
    integer_sqrt(squared as u64).min(u64::from(u16::MAX)) as u16
}

fn integer_sqrt(value: u64) -> u64 {
    let mut result = 0_u64;
    let mut bit = 1_u64 << 62;
    while bit > value {
        bit >>= 2;
    }
    let mut remainder = value;
    while bit != 0 {
        if remainder >= result + bit {
            remainder -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    result
}

fn stationary(sample: RawImuSample) -> bool {
    let gravity = magnitude(sample);
    (MINIMUM_GRAVITY_MM_S2..=MAXIMUM_GRAVITY_MM_S2).contains(&gravity)
        && sample.gyro_x_milliradians_s.abs() <= MAXIMUM_STATIONARY_GYRO_MRAD_S
        && sample.gyro_y_milliradians_s.abs() <= MAXIMUM_STATIONARY_GYRO_MRAD_S
        && sample.gyro_z_milliradians_s.abs() <= MAXIMUM_STATIONARY_GYRO_MRAD_S
}

fn publish_sample(sample: RawImuSample) {
    ACCEL_X.store(i32::from(sample.accel_x_mm_s2), Ordering::Release);
    ACCEL_Y.store(i32::from(sample.accel_y_mm_s2), Ordering::Release);
    ACCEL_Z.store(i32::from(sample.accel_z_mm_s2), Ordering::Release);
    GYRO_X.store(
        i32::from(sample.gyro_x_milliradians_s),
        Ordering::Release,
    );
    GYRO_Y.store(
        i32::from(sample.gyro_y_milliradians_s),
        Ordering::Release,
    );
    GYRO_Z.store(
        i32::from(sample.gyro_z_milliradians_s),
        Ordering::Release,
    );
    OBSERVED_AT_MS.store(sample.observed_at_tick as u32, Ordering::Release);
    SAMPLES.fetch_add(1, Ordering::Relaxed);
}

#[embassy_executor::task]
pub async fn task(
    i2c1: Peri<'static, I2C1>,
    sda: Peri<'static, PIN_2>,
    scl: Peri<'static, PIN_3>,
) {
    // Embassy's blocking RP2040 I2C implementation waits without a software
    // deadline when a physical bus is held. Keep that work off the shared
    // executor until USB has enumerated and emitted the build-bound boot
    // record. A bad auxiliary attachment can still fail its own qualification,
    // but it can no longer make the running image anonymous to the host.
    while !USB_CONNECTION_READY.load(Ordering::Acquire) {
        Timer::after(Duration::from_millis(POLL_MS)).await;
    }

    let mut config = I2cConfig::default();
    config.frequency = 100_000;
    let mut provider = Provider(I2c::new_blocking(i2c1, scl, sda, config));

    loop {
        STATE.store(State::Probing as u8, Ordering::Release);
        let mut selected = None;
        for address in [DEFAULT_ADDRESS, ALTERNATE_ADDRESS] {
            let mut session = Mpu6050Session::new(address).expect("fixed address is valid");
            match session.observe(&mut provider, Instant::now().as_millis()) {
                Ok(sample) => {
                    ADDRESS.store(address, Ordering::Release);
                    FAILURE.store(0, Ordering::Release);
                    publish_sample(sample);
                    selected = Some((session, sample));
                    break;
                }
                Err(failure) => FAILURE.store(failure_code(failure), Ordering::Release),
            }
        }

        let Some((mut session, first)) = selected else {
            STATE.store(State::Fault as u8, Ordering::Release);
            Timer::after(Duration::from_millis(RETRY_MS)).await;
            continue;
        };

        STATE.store(State::Calibrating as u8, Ordering::Release);
        let mut calibration_count = 0_u32;
        let mut sum_x = 0_i32;
        let mut sum_y = 0_i32;
        let mut sum_z = 0_i32;
        let mut sample = first;
        let calibration = loop {
            if stationary(sample) {
                calibration_count += 1;
                sum_x += i32::from(sample.accel_x_mm_s2);
                sum_y += i32::from(sample.accel_y_mm_s2);
                sum_z += i32::from(sample.accel_z_mm_s2);
            } else {
                calibration_count = 0;
                sum_x = 0;
                sum_y = 0;
                sum_z = 0;
            }
            if calibration_count == CALIBRATION_SAMPLES {
                let reference = RawImuSample {
                    observed_at_tick: sample.observed_at_tick,
                    accel_x_mm_s2: (sum_x / CALIBRATION_SAMPLES as i32) as i16,
                    accel_y_mm_s2: (sum_y / CALIBRATION_SAMPLES as i32) as i16,
                    accel_z_mm_s2: (sum_z / CALIBRATION_SAMPLES as i32) as i16,
                    gyro_x_milliradians_s: 0,
                    gyro_y_milliradians_s: 0,
                    gyro_z_milliradians_s: 0,
                };
                match GravityCalibration::capture(1, reference) {
                    Ok(calibration) => break Some(calibration),
                    Err(_) => break None,
                }
            }
            Timer::after(Duration::from_millis(POLL_MS)).await;
            match session.observe(&mut provider, Instant::now().as_millis()) {
                Ok(next) => {
                    sample = next;
                    publish_sample(sample);
                }
                Err(failure) => {
                    FAILURE.store(failure_code(failure), Ordering::Release);
                    break None;
                }
            }
        };

        let Some(calibration) = calibration else {
            STATE.store(State::Fault as u8, Ordering::Release);
            Timer::after(Duration::from_millis(RETRY_MS)).await;
            continue;
        };
        CALIBRATION_GENERATION.store(1, Ordering::Release);
        let mut deriver = ImuDeriver::new(calibration);
        STATE.store(State::Healthy as u8, Ordering::Release);

        loop {
            Timer::after(Duration::from_millis(POLL_MS)).await;
            let now = Instant::now().as_millis();
            match session.observe(&mut provider, now) {
                Ok(sample) => {
                    publish_sample(sample);
                    match deriver.derive(sample, now, THRESHOLDS) {
                        Ok(derived) => {
                            TILT_ACTIVE.store(derived.tilt_active, Ordering::Release);
                            IMPACT_ACTIVE.store(derived.impact_active, Ordering::Release);
                            FAILURE.store(0, Ordering::Release);
                        }
                        Err(_) => {
                            STATE.store(State::Fault as u8, Ordering::Release);
                            break;
                        }
                    }
                }
                Err(failure) => {
                    FAILURE.store(failure_code(failure), Ordering::Release);
                    STATE.store(State::Fault as u8, Ordering::Release);
                    break;
                }
            }
        }
        TILT_ACTIVE.store(false, Ordering::Release);
        IMPACT_ACTIVE.store(false, Ordering::Release);
        CALIBRATION_GENERATION.store(0, Ordering::Release);
        Timer::after(Duration::from_millis(RETRY_MS)).await;
    }
}
