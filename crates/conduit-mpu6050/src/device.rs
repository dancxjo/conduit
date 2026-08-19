pub const DEFAULT_ADDRESS: u8 = 0x68;
pub const ALTERNATE_ADDRESS: u8 = 0x69;
pub const WHO_AM_I_VALUE: u8 = 0x68;
pub const FRAME_BYTES: usize = 14;

const ACCEL_XOUT_H: u8 = 0x3b;
const GYRO_CONFIG: u8 = 0x1b;
const ACCEL_CONFIG: u8 = 0x1c;
const PWR_MGMT_1: u8 = 0x6b;
const WHO_AM_I: u8 = 0x75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cBaseAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cProviderFailure {
    Write,
    Read,
}

pub trait Mpu6050I2cProvider {
    fn availability(&self) -> I2cBaseAvailability;
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), I2cProviderFailure>;
    fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), I2cProviderFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mpu6050Failure {
    InvalidAddress,
    I2cBaseUnavailable,
    DeviceNoResponse,
    IdentityMismatch { observed: u8 },
    WakeWriteFailed,
    GyroConfigWriteFailed,
    AccelConfigWriteFailed,
    FrameReadFailed,
    ClockRegressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawImuSample {
    pub observed_at_tick: u64,
    pub accel_x_mm_s2: i16,
    pub accel_y_mm_s2: i16,
    pub accel_z_mm_s2: i16,
    pub gyro_x_milliradians_s: i16,
    pub gyro_y_milliradians_s: i16,
    pub gyro_z_milliradians_s: i16,
}

impl RawImuSample {
    pub const fn stationary(observed_at_tick: u64) -> Self {
        Self {
            observed_at_tick,
            accel_x_mm_s2: 0,
            accel_y_mm_s2: 0,
            accel_z_mm_s2: 9_807,
            gyro_x_milliradians_s: 0,
            gyro_y_milliradians_s: 0,
            gyro_z_milliradians_s: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mpu6050Session {
    address: u8,
    initialized: bool,
    last_observed_at_tick: Option<u64>,
}

impl Mpu6050Session {
    pub const fn new(address: u8) -> Result<Self, Mpu6050Failure> {
        if address != DEFAULT_ADDRESS && address != ALTERNATE_ADDRESS {
            return Err(Mpu6050Failure::InvalidAddress);
        }
        Ok(Self {
            address,
            initialized: false,
            last_observed_at_tick: None,
        })
    }

    pub fn observe<P: Mpu6050I2cProvider>(
        &mut self,
        provider: &mut P,
        now_tick: u64,
    ) -> Result<RawImuSample, Mpu6050Failure> {
        require_base(provider)?;
        if self
            .last_observed_at_tick
            .is_some_and(|previous| now_tick < previous)
        {
            return Err(Mpu6050Failure::ClockRegressed);
        }
        if !self.initialized {
            self.initialize(provider)?;
        }
        let mut bytes = [0_u8; FRAME_BYTES];
        provider
            .write_read(self.address, &[ACCEL_XOUT_H], &mut bytes)
            .map_err(|_| Mpu6050Failure::FrameReadFailed)?;
        let sample = decode_sample(now_tick, &bytes);
        self.last_observed_at_tick = Some(now_tick);
        Ok(sample)
    }

    fn initialize<P: Mpu6050I2cProvider>(
        &mut self,
        provider: &mut P,
    ) -> Result<(), Mpu6050Failure> {
        let mut identity = [0_u8; 1];
        provider
            .write_read(self.address, &[WHO_AM_I], &mut identity)
            .map_err(|_| Mpu6050Failure::DeviceNoResponse)?;
        if identity[0] != WHO_AM_I_VALUE {
            return Err(Mpu6050Failure::IdentityMismatch {
                observed: identity[0],
            });
        }
        write_register(provider, self.address, PWR_MGMT_1, 0)
            .map_err(|_| Mpu6050Failure::WakeWriteFailed)?;
        write_register(provider, self.address, GYRO_CONFIG, 0)
            .map_err(|_| Mpu6050Failure::GyroConfigWriteFailed)?;
        write_register(provider, self.address, ACCEL_CONFIG, 0)
            .map_err(|_| Mpu6050Failure::AccelConfigWriteFailed)?;
        self.initialized = true;
        Ok(())
    }
}

fn require_base<P: Mpu6050I2cProvider>(provider: &P) -> Result<(), Mpu6050Failure> {
    match provider.availability() {
        I2cBaseAvailability::Available => Ok(()),
        I2cBaseAvailability::Unavailable => Err(Mpu6050Failure::I2cBaseUnavailable),
    }
}

fn write_register<P: Mpu6050I2cProvider>(
    provider: &mut P,
    address: u8,
    register: u8,
    value: u8,
) -> Result<(), I2cProviderFailure> {
    provider.write(address, &[register, value])
}

pub fn decode_sample(observed_at_tick: u64, bytes: &[u8; FRAME_BYTES]) -> RawImuSample {
    RawImuSample {
        observed_at_tick,
        accel_x_mm_s2: accel_raw_to_mm_s2(read_i16(bytes, 0)),
        accel_y_mm_s2: accel_raw_to_mm_s2(read_i16(bytes, 2)),
        accel_z_mm_s2: accel_raw_to_mm_s2(read_i16(bytes, 4)),
        gyro_x_milliradians_s: gyro_raw_to_milliradians_s(read_i16(bytes, 8)),
        gyro_y_milliradians_s: gyro_raw_to_milliradians_s(read_i16(bytes, 10)),
        gyro_z_milliradians_s: gyro_raw_to_milliradians_s(read_i16(bytes, 12)),
    }
}

fn read_i16(bytes: &[u8; FRAME_BYTES], offset: usize) -> i16 {
    i16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn accel_raw_to_mm_s2(raw: i16) -> i16 {
    clamp_i16(i32::from(raw).saturating_mul(9_807) / 16_384)
}

fn gyro_raw_to_milliradians_s(raw: i16) -> i16 {
    clamp_i16(i32::from(raw).saturating_mul(133) / 1_000)
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    struct Provider {
        available: bool,
        identity: u8,
        frame: [u8; FRAME_BYTES],
        fail_write: Option<usize>,
        writes: Vec<[u8; 2]>,
    }

    impl Mpu6050I2cProvider for Provider {
        fn availability(&self) -> I2cBaseAvailability {
            if self.available {
                I2cBaseAvailability::Available
            } else {
                I2cBaseAvailability::Unavailable
            }
        }

        fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), I2cProviderFailure> {
            assert_eq!(address, DEFAULT_ADDRESS);
            let index = self.writes.len();
            if self.fail_write == Some(index) {
                return Err(I2cProviderFailure::Write);
            }
            self.writes.push(bytes.try_into().unwrap());
            Ok(())
        }

        fn write_read(
            &mut self,
            address: u8,
            write: &[u8],
            read: &mut [u8],
        ) -> Result<(), I2cProviderFailure> {
            assert_eq!(address, DEFAULT_ADDRESS);
            match write {
                [WHO_AM_I] => read.copy_from_slice(&[self.identity]),
                [ACCEL_XOUT_H] => read.copy_from_slice(&self.frame),
                _ => return Err(I2cProviderFailure::Read),
            }
            Ok(())
        }
    }

    fn provider() -> Provider {
        let mut frame = [0_u8; FRAME_BYTES];
        frame[4..6].copy_from_slice(&16_384_i16.to_be_bytes());
        Provider {
            available: true,
            identity: WHO_AM_I_VALUE,
            frame,
            fail_write: None,
            writes: Vec::new(),
        }
    }

    #[test]
    fn exact_initialization_and_frame_are_finite() {
        let mut provider = provider();
        let mut session = Mpu6050Session::new(DEFAULT_ADDRESS).unwrap();
        let sample = session.observe(&mut provider, 10).unwrap();
        assert_eq!(sample, RawImuSample::stationary(10));
        assert_eq!(
            provider.writes,
            [[PWR_MGMT_1, 0], [GYRO_CONFIG, 0], [ACCEL_CONFIG, 0]]
        );
        session.observe(&mut provider, 20).unwrap();
        assert_eq!(provider.writes.len(), 3);
    }

    #[test]
    fn base_identity_configuration_and_clock_fail_distinctly() {
        let mut missing = provider();
        missing.available = false;
        assert_eq!(
            Mpu6050Session::new(DEFAULT_ADDRESS)
                .unwrap()
                .observe(&mut missing, 1),
            Err(Mpu6050Failure::I2cBaseUnavailable)
        );

        let mut wrong = provider();
        wrong.identity = 0x70;
        assert_eq!(
            Mpu6050Session::new(DEFAULT_ADDRESS)
                .unwrap()
                .observe(&mut wrong, 1),
            Err(Mpu6050Failure::IdentityMismatch { observed: 0x70 })
        );

        let mut failed = provider();
        failed.fail_write = Some(1);
        assert_eq!(
            Mpu6050Session::new(DEFAULT_ADDRESS)
                .unwrap()
                .observe(&mut failed, 1),
            Err(Mpu6050Failure::GyroConfigWriteFailed)
        );

        let mut good = provider();
        let mut session = Mpu6050Session::new(DEFAULT_ADDRESS).unwrap();
        session.observe(&mut good, 2).unwrap();
        assert_eq!(
            session.observe(&mut good, 1),
            Err(Mpu6050Failure::ClockRegressed)
        );
    }
}
