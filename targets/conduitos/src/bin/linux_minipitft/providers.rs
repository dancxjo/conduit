//! Linux-owned fixed B+ attachment. Protocol commands remain checked Source.
use conduitos::{
    cryptographic_entropy::{CryptographicEntropySource, EntropyProvider, EntropyRefusal},
    digital_base::{DigitalDisposition, DigitalProvider, DigitalRequest},
    monotonic_clock::{codec::ClockDisposition, owner::MonotonicDeadlineProvider},
    spi_base::transaction::{SpiDisposition, SpiProvider, SpiTransaction},
};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::fd::{AsRawFd, FromRawFd},
    sync::OnceLock,
    time::Instant,
};
unsafe extern "C" {
    fn ioctl(fd: i32, request: usize, ...) -> i32;
    fn flock(fd: i32, operation: i32) -> i32;
}
fn control<T>(file: &File, request: usize, value: &mut T) -> Result<(), &'static str> {
    if unsafe { ioctl(file.as_raw_fd(), request, value as *mut T) } < 0 {
        Err("linux-device-ioctl-failed")
    } else {
        Ok(())
    }
}
static START: OnceLock<Instant> = OnceLock::new();
pub fn counter() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_micros() as u64
}
pub struct Clock {
    revoked: bool,
}
impl Clock {
    pub fn new() -> Self {
        let _ = counter();
        Self { revoked: false }
    }
}
impl MonotonicDeadlineProvider for Clock {
    fn sample_now(&mut self) -> Result<u64, ClockDisposition> {
        if self.revoked {
            Err(ClockDisposition::Unavailable)
        } else {
            Ok(counter() / 1000)
        }
    }
    fn poll_until(&mut self, deadline: u64) -> Result<Option<u64>, ClockDisposition> {
        let now = self.sample_now()?;
        Ok((now >= deadline).then_some(now))
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}
pub struct Entropy(File);
impl Entropy {
    pub fn new() -> Result<Self, &'static str> {
        File::open("/dev/urandom")
            .map(Self)
            .map_err(|_| "linux-entropy-unavailable")
    }
}
impl CryptographicEntropySource for Entropy {
    fn provider(&self) -> EntropyProvider {
        EntropyProvider {
            base_id: "linux/getrandom",
            provider_instance_id: "linux/urandom",
            provider_generation: 1,
        }
    }
    fn fill_exact(&mut self, out: &mut [u8]) -> Result<(), EntropyRefusal> {
        self.0
            .read_exact(out)
            .map_err(|_| EntropyRefusal::SourceFailure)
    }
}
#[repr(C)]
struct LineRequest {
    offsets: [u32; 64],
    flags: u32,
    defaults: [u8; 64],
    consumer: [u8; 32],
    lines: u32,
    fd: i32,
}
pub struct Contact {
    handle: File,
    output: bool,
    revoked: bool,
}
impl Contact {
    fn new(chip: &File, offset: u32, output: bool) -> Result<Self, &'static str> {
        let mut req = LineRequest {
            offsets: [0; 64],
            flags: if output { 2 } else { 1 },
            defaults: [0; 64],
            consumer: [0; 32],
            lines: 1,
            fd: -1,
        };
        req.offsets[0] = offset;
        let label = b"conduit-minipitft";
        req.consumer[..label.len()].copy_from_slice(label);
        control(chip, 0xc16cb403, &mut req)?;
        Ok(Self {
            handle: unsafe { File::from_raw_fd(req.fd) },
            output,
            revoked: false,
        })
    }
    fn drive(&mut self, high: bool) -> Result<(), &'static str> {
        let mut data = [0u8; 64];
        data[0] = u8::from(high);
        control(&self.handle, 0xc040b409, &mut data)
    }
}
impl DigitalProvider for Contact {
    fn access(&mut self, req: DigitalRequest) -> Result<bool, DigitalDisposition> {
        if self.revoked {
            return Err(DigitalDisposition::ProviderLost);
        }
        if let DigitalRequest::Drive(high) = req {
            if !self.output {
                return Err(DigitalDisposition::Unsupported);
            }
            self.drive(high)
                .map_err(|_| DigitalDisposition::ProviderLost)?;
        }
        let mut data = [0u8; 64];
        control(&self.handle, 0xc040b408, &mut data)
            .map_err(|_| DigitalDisposition::ProviderLost)?;
        Ok(data[0] != 0)
    }
    fn revoke(&mut self) {
        if self.output {
            let _ = self.drive(false);
        }
        self.revoked = true;
    }
}
impl Drop for Contact {
    fn drop(&mut self) {
        self.revoke();
    }
}
#[repr(C)]
struct Transfer {
    tx: u64,
    rx: u64,
    len: u32,
    speed: u32,
    delay: u16,
    bits: u8,
    cs_change: u8,
    tx_nbits: u8,
    rx_nbits: u8,
    word_delay: u8,
    pad: u8,
}
pub struct Spi {
    file: File,
    _lock: File,
    revoked: bool,
}
impl SpiProvider for Spi {
    fn exchange(
        &mut self,
        req: &SpiTransaction<'_>,
        input: &mut [u8],
    ) -> Result<(), SpiDisposition> {
        if self.revoked || input.len() != req.output().len() {
            return Err(SpiDisposition::Refused);
        }
        let mut transfer = Transfer {
            tx: req.output().as_ptr() as u64,
            rx: input.as_mut_ptr() as u64,
            len: input.len() as u32,
            speed: 4_000_000,
            delay: 0,
            bits: 8,
            cs_change: 0,
            tx_nbits: 0,
            rx_nbits: 0,
            word_delay: 0,
            pad: 0,
        };
        let transferred = unsafe {
            ioctl(
                self.file.as_raw_fd(),
                0x40206b00,
                &mut transfer as *mut Transfer,
            )
        };
        if transferred == transfer.len as i32 {
            Ok(())
        } else {
            self.revoked = true;
            Err(SpiDisposition::ProviderLost)
        }
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
}
pub struct Attachment {
    pub spi: Spi,
    pub backlight: Contact,
    pub command_data: Contact,
    pub button_a: Contact,
    pub button_b: Contact,
}
pub fn admit() -> Result<Attachment, &'static str> {
    let model = std::fs::read_to_string("/proc/device-tree/model")
        .map_err(|_| "board-model-unavailable")?;
    if model.trim_end_matches('\0') != "Raspberry Pi Model B Plus Rev 1.2" {
        return Err("board-model-unsupported");
    }
    let lockpath = std::env::var("XDG_RUNTIME_DIR").map_err(|_| "runtime-dir-unavailable")?
        + "/conduit-minipitft.lock";
    let lock = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(lockpath)
        .map_err(|_| "attachment-lock-unavailable")?;
    if unsafe { flock(lock.as_raw_fd(), 6) } != 0 {
        return Err("attachment-already-owned");
    }
    let chip = File::open("/dev/gpiochip0").map_err(|_| "gpio-unavailable")?;
    let backlight = Contact::new(&chip, 22, true)?;
    let command_data = Contact::new(&chip, 25, true)?;
    let button_a = Contact::new(&chip, 23, false)?;
    let button_b = Contact::new(&chip, 24, false)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/spidev0.0")
        .map_err(|_| "spi-unavailable")?;
    control(&file, 0x40016b01, &mut 0u8)?;
    control(&file, 0x40016b03, &mut 8u8)?;
    control(&file, 0x40046b04, &mut 4_000_000u32)?;
    Ok(Attachment {
        spi: Spi {
            file,
            _lock: lock,
            revoked: false,
        },
        backlight,
        command_data,
        button_a,
        button_b,
    })
}

// Linux UAPI geometry is identical on the ARMv6 and x86 build machines.
const _: () = assert!(std::mem::size_of::<LineRequest>() == 364);
const _: () = assert!(std::mem::size_of::<Transfer>() == 32);
