//! Allocator-free SSD1306 128x32 I2C device protocol.
//!
//! Presentation selection, Host identity, planning, and safety live above this
//! exact display mechanism.

#![no_std]

#[cfg(test)]
extern crate std;

pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 32;
pub const FRAMEBUFFER_BYTES: usize = WIDTH * HEIGHT / 8;
pub const DEFAULT_ADDRESS: u8 = 0x3c;
pub const ALTERNATE_ADDRESS: u8 = 0x3d;
pub const DATA_CHUNK_BYTES: usize = 16;
pub const DATA_TRANSACTIONS: usize = FRAMEBUFFER_BYTES / DATA_CHUNK_BYTES;

const COMMAND_CONTROL: u8 = 0x00;
const DATA_CONTROL: u8 = 0x40;
const DISPLAY_OFF: u8 = 0xae;
const DISPLAY_ON: u8 = 0xaf;
const INITIALIZATION: &[u8] = &[
    0xd5, 0x80, // clock
    0xa8, 0x1f, // 32-row multiplex
    0xd3, 0x00, // display offset
    0x40, // start line
    0x8d, 0x14, // charge pump
    0x20, 0x00, // horizontal addressing
    0xa1, // segment remap
    0xc8, // COM scan direction
    0xda, 0x02, // COM pins for 128x32
    0x81, 0x8f, // contrast
    0xd9, 0xf1, // pre-charge
    0xdb, 0x40, // VCOM detect
    0xa4, // display follows RAM
    0xa6, // non-inverted
];
const FRAME_WINDOW: &[u8] = &[0x21, 0x00, 0x7f, 0x22, 0x00, 0x03];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cBaseAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I2cProviderFailure {
    Write,
}

pub trait Ssd1306I2cProvider {
    fn availability(&self) -> I2cBaseAvailability;
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), I2cProviderFailure>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ssd1306Failure {
    InvalidAddress,
    I2cBaseUnavailable,
    DisplayNoResponse,
    InitializationFailed,
    FrameWindowFailed,
    FrameWriteFailed { chunk: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ssd1306Session {
    address: u8,
    initialized: bool,
}

impl Ssd1306Session {
    pub const fn new(address: u8) -> Result<Self, Ssd1306Failure> {
        if address != DEFAULT_ADDRESS && address != ALTERNATE_ADDRESS {
            return Err(Ssd1306Failure::InvalidAddress);
        }
        Ok(Self {
            address,
            initialized: false,
        })
    }

    pub fn display<P: Ssd1306I2cProvider>(
        &mut self,
        provider: &mut P,
        framebuffer: &[u8; FRAMEBUFFER_BYTES],
    ) -> Result<(), Ssd1306Failure> {
        require_base(provider)?;
        if !self.initialized {
            self.initialize(provider)?;
        }
        write_commands(provider, self.address, FRAME_WINDOW)
            .map_err(|_| Ssd1306Failure::FrameWindowFailed)?;
        for (index, chunk) in framebuffer
            .as_chunks::<DATA_CHUNK_BYTES>()
            .0
            .iter()
            .enumerate()
        {
            let mut transaction = [0_u8; DATA_CHUNK_BYTES + 1];
            transaction[0] = DATA_CONTROL;
            transaction[1..].copy_from_slice(chunk);
            provider
                .write(self.address, &transaction)
                .map_err(|_| Ssd1306Failure::FrameWriteFailed { chunk: index as u8 })?;
        }
        Ok(())
    }

    fn initialize<P: Ssd1306I2cProvider>(
        &mut self,
        provider: &mut P,
    ) -> Result<(), Ssd1306Failure> {
        write_commands(provider, self.address, &[DISPLAY_OFF])
            .map_err(|_| Ssd1306Failure::DisplayNoResponse)?;
        write_commands(provider, self.address, INITIALIZATION)
            .map_err(|_| Ssd1306Failure::InitializationFailed)?;
        write_commands(provider, self.address, &[DISPLAY_ON])
            .map_err(|_| Ssd1306Failure::InitializationFailed)?;
        self.initialized = true;
        Ok(())
    }
}

fn require_base<P: Ssd1306I2cProvider>(provider: &P) -> Result<(), Ssd1306Failure> {
    match provider.availability() {
        I2cBaseAvailability::Available => Ok(()),
        I2cBaseAvailability::Unavailable => Err(Ssd1306Failure::I2cBaseUnavailable),
    }
}

fn write_commands<P: Ssd1306I2cProvider>(
    provider: &mut P,
    address: u8,
    commands: &[u8],
) -> Result<(), I2cProviderFailure> {
    let mut transaction = [0_u8; INITIALIZATION.len() + 1];
    transaction[0] = COMMAND_CONTROL;
    transaction[1..commands.len() + 1].copy_from_slice(commands);
    provider.write(address, &transaction[..commands.len() + 1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    struct Provider {
        available: bool,
        fail_at: Option<usize>,
        writes: Vec<Vec<u8>>,
    }

    impl Ssd1306I2cProvider for Provider {
        fn availability(&self) -> I2cBaseAvailability {
            if self.available {
                I2cBaseAvailability::Available
            } else {
                I2cBaseAvailability::Unavailable
            }
        }
        fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), I2cProviderFailure> {
            assert_eq!(address, DEFAULT_ADDRESS);
            if self.fail_at == Some(self.writes.len()) {
                return Err(I2cProviderFailure::Write);
            }
            self.writes.push(bytes.to_vec());
            Ok(())
        }
    }

    fn provider() -> Provider {
        Provider {
            available: true,
            fail_at: None,
            writes: Vec::new(),
        }
    }

    #[test]
    fn exact_initialization_and_frame_are_bounded() {
        let mut provider = provider();
        let mut session = Ssd1306Session::new(DEFAULT_ADDRESS).unwrap();
        let mut frame = [0_u8; FRAMEBUFFER_BYTES];
        frame[0] = 0xaa;
        frame[FRAMEBUFFER_BYTES - 1] = 0x55;
        session.display(&mut provider, &frame).unwrap();
        assert_eq!(provider.writes.len(), 4 + DATA_TRANSACTIONS);
        assert_eq!(provider.writes[0], [COMMAND_CONTROL, DISPLAY_OFF]);
        assert_eq!(
            provider.writes[3],
            [COMMAND_CONTROL, 0x21, 0x00, 0x7f, 0x22, 0x00, 0x03]
        );
        assert_eq!(provider.writes[4][1], 0xaa);
        assert_eq!(provider.writes.last().unwrap()[DATA_CHUNK_BYTES], 0x55);
        session.display(&mut provider, &frame).unwrap();
        assert_eq!(
            provider.writes.len(),
            4 + DATA_TRANSACTIONS + 1 + DATA_TRANSACTIONS
        );
    }

    #[test]
    fn base_device_initialization_window_and_frame_fail_distinctly() {
        let frame = [0_u8; FRAMEBUFFER_BYTES];
        let mut missing = provider();
        missing.available = false;
        assert_eq!(
            Ssd1306Session::new(DEFAULT_ADDRESS)
                .unwrap()
                .display(&mut missing, &frame),
            Err(Ssd1306Failure::I2cBaseUnavailable)
        );
        for (failure_index, expected) in [
            (0, Ssd1306Failure::DisplayNoResponse),
            (1, Ssd1306Failure::InitializationFailed),
            (3, Ssd1306Failure::FrameWindowFailed),
            (4, Ssd1306Failure::FrameWriteFailed { chunk: 0 }),
        ] {
            let mut failed = provider();
            failed.fail_at = Some(failure_index);
            assert_eq!(
                Ssd1306Session::new(DEFAULT_ADDRESS)
                    .unwrap()
                    .display(&mut failed, &frame),
                Err(expected)
            );
        }
    }
}
