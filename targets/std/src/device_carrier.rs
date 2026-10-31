//! Owned Linux read-only CPBR device information session. This acquired local
//! peripheral is not a Body member or Line, and acquisition grants no Host Call.
use conduit_core::{BootId, HostAdvertisement, HostId, OfferGeneration};
use conduit_device_carrier_protocol::{header, InformationDecoder, InformationSession, HEADER};
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::{ErrorKind, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, OpenOptionsExt},
    },
    path::Path,
    time::{Duration, Instant},
};
const RESPONSE: usize = 1536;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOnlyQuery {
    Status,
    SensorStatus,
    Diagnostics,
}
impl ReadOnlyQuery {
    fn command(self) -> u8 {
        match self {
            Self::Status => 3,
            Self::SensorStatus => 6,
            Self::Diagnostics => 7,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Configuration,
    Lost,
    Timeout,
    Protocol,
    Refused,
    Identity,
    Scope,
    Clock,
}
/// Independent checked construction inputs, never learned from the device.
pub struct ExpectedDeviceIdentity {
    pub role: String,
    pub firmware: [u8; 32],
    pub configuration: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceCarrierBinding {
    pub host: HostId,
    pub boot: BootId,
    pub offer_generation: OfferGeneration,
    pub acquisition: [u8; 16],
}
pub struct Information<'a> {
    pub payload: &'a [u8],
    pub observed_at_host_boot_ms: u64,
}
struct Port(File);
impl Drop for Port {
    fn drop(&mut self) {
        release_dtr(&self.0);
    }
}
/// Retain the same exclusive fd/session with its initialized Base owner across
/// child Plays. Failure permanently poisons this owner; reacquisition requires
/// ordinary retirement/re-admission. No automatic ambiguous-request replay.
pub struct DeviceCarrier {
    port: Port,
    binding: DeviceCarrierBinding,
    expected: ExpectedDeviceIdentity,
    firmware_text: [u8; 64],
    configuration_text: [u8; 71],
    session: InformationSession,
    response: [u8; RESPONSE],
    timeout: Duration,
    failure: Option<Failure>,
}
impl DeviceCarrier {
    /// Advance only this retained exclusive physical owner within the same
    /// native Host/Boot while Root prepares a newer inventory before publication.
    pub fn prepare_for_host(&mut self, host: &HostAdvertisement) -> Result<(), Failure> {
        if self.failure.is_some()
            || host.host_id != self.binding.host
            || host.boot_id != self.binding.boot
            || host.offer_generation.0 < self.binding.offer_generation.0
        {
            return Err(Failure::Scope);
        }
        self.binding.offer_generation = host.offer_generation;
        Ok(())
    }
    pub fn acquire(
        path: &Path,
        host: &HostAdvertisement,
        expected: ExpectedDeviceIdentity,
        timeout: Duration,
    ) -> Result<Self, Failure> {
        if timeout.is_zero()
            || timeout > Duration::from_secs(5)
            || expected.role.is_empty()
            || expected.role.len() > 96
            || expected.firmware == [0; 32]
            || expected.configuration == [0; 32]
            || host.host_id.as_str().is_empty()
            || host.boot_id.as_str().is_empty()
            || host.offer_generation.0 == 0
        {
            return Err(Failure::Configuration);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| Failure::Lost)?;
        if !file
            .metadata()
            .map_err(|_| Failure::Lost)?
            .file_type()
            .is_char_device()
        {
            return Err(Failure::Configuration);
        }
        let fd = file.as_raw_fd();
        if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0
            || unsafe { libc::ioctl(fd, libc::TIOCEXCL) } != 0
        {
            return Err(Failure::Lost);
        }
        // Only an exclusively claimed descriptor may own DTR cleanup. A
        // rejected cooperative lock must not disturb its existing owner.
        let port = Port(file);
        crate::usb_cdc::configure_cdc_port(&port.0, 1, 0).map_err(|_| Failure::Lost)?;
        let bits = libc::TIOCM_DTR;
        if unsafe { libc::ioctl(fd, libc::TIOCMBIC, &bits) } != 0 {
            return Err(Failure::Lost);
        }
        // The existing CDC protocol resets its request sequence while DTR is low.
        // This is its session handshake, not a BOOTSEL or MCU reset command.
        std::thread::sleep(Duration::from_millis(650));
        if unsafe { libc::tcflush(fd, libc::TCIFLUSH) } != 0
            || unsafe { libc::ioctl(fd, libc::TIOCMBIS, &bits) } != 0
        {
            return Err(Failure::Lost);
        }
        let mut acquisition = [0; 16];
        File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut acquisition))
            .map_err(|_| Failure::Lost)?;
        if acquisition == [0; 16] {
            return Err(Failure::Lost);
        }
        let mut owner = Self::from_port(
            port,
            DeviceCarrierBinding {
                host: host.host_id.clone(),
                boot: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                acquisition,
            },
            expected,
            timeout,
        );
        owner.query(host, ReadOnlyQuery::Status)?;
        Ok(owner)
    }
    fn from_port(
        port: Port,
        binding: DeviceCarrierBinding,
        expected: ExpectedDeviceIdentity,
        timeout: Duration,
    ) -> Self {
        let firmware_text = hex(expected.firmware);
        let mut configuration_text = [0; 71];
        configuration_text[..7].copy_from_slice(b"sha256:");
        configuration_text[7..].copy_from_slice(&hex(expected.configuration));
        Self {
            port,
            binding,
            expected,
            firmware_text,
            configuration_text,
            session: InformationSession::default(),
            response: [0; RESPONSE],
            timeout,
            failure: None,
        }
    }
    pub fn binding(&self) -> &DeviceCarrierBinding {
        &self.binding
    }
    pub fn query(
        &mut self,
        host: &HostAdvertisement,
        query: ReadOnlyQuery,
    ) -> Result<Information<'_>, Failure> {
        self.query_with_budget(host, query, self.timeout)
    }
    /// Tighten one query's budget without enlarging the initialized owner's
    /// admitted maximum. Useful beneath a bounded multi-observation acquisition.
    pub fn query_with_budget(
        &mut self,
        host: &HostAdvertisement,
        query: ReadOnlyQuery,
        budget: Duration,
    ) -> Result<Information<'_>, Failure> {
        if budget.is_zero() || budget > self.timeout {
            return Err(Failure::Configuration);
        }
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        if host.host_id != self.binding.host
            || host.boot_id != self.binding.boot
            || host.offer_generation != self.binding.offer_generation
        {
            self.failure = Some(Failure::Scope);
            return Err(Failure::Scope);
        }
        let result = exchange(
            &mut self.port.0,
            &mut self.session,
            query,
            budget,
            &mut self.response,
        )
        .and_then(|length| {
            verify_identity(
                &self.response[..length],
                &self.expected,
                &self.firmware_text,
                &self.configuration_text,
            )?;
            Ok((length, boot_ms()?))
        });
        match result {
            Ok((length, observed_at_host_boot_ms)) => Ok(Information {
                payload: &self.response[..length],
                observed_at_host_boot_ms,
            }),
            Err(failure) => {
                self.failure = Some(failure);
                Err(failure)
            }
        }
    }
}
fn hex(input: [u8; 32]) -> [u8; 64] {
    let mut out = [0; 64];
    for (at, byte) in input.into_iter().enumerate() {
        out[at * 2] = b"0123456789abcdef"[(byte >> 4) as usize];
        out[at * 2 + 1] = b"0123456789abcdef"[(byte & 15) as usize];
    }
    out
}
#[derive(Deserialize)]
struct Identity<'a> {
    #[serde(borrow)]
    role: &'a str,
    #[serde(borrow)]
    build: &'a str,
    #[serde(borrow)]
    configuration: &'a str,
}
fn verify_identity(
    bytes: &[u8],
    expected: &ExpectedDeviceIdentity,
    firmware: &[u8; 64],
    configuration: &[u8; 71],
) -> Result<(), Failure> {
    let identity: Identity<'_> = serde_json::from_slice(bytes).map_err(|_| Failure::Protocol)?;
    if identity.role != expected.role
        || identity.build.as_bytes() != firmware
        || identity.configuration.as_bytes() != configuration
    {
        return Err(Failure::Identity);
    }
    Ok(())
}
fn boot_ms() -> Result<u64, Failure> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut value) } != 0
        || value.tv_sec < 0
        || value.tv_nsec < 0
    {
        return Err(Failure::Clock);
    }
    (value.tv_sec as u64)
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add(value.tv_nsec as u64 / 1_000_000))
        .ok_or(Failure::Clock)
}
fn exchange(
    stream: &mut (impl Read + Write),
    session: &mut InformationSession,
    query: ReadOnlyQuery,
    timeout: Duration,
    output: &mut [u8; RESPONSE],
) -> Result<usize, Failure> {
    let request = session
        .begin(query.command())
        .map_err(|_| Failure::Protocol)?;
    let command = header(query.command(), request.id(), 0).map_err(|_| Failure::Protocol)?;
    let mut decoder =
        InformationDecoder::<{ HEADER + RESPONSE }>::new(request).map_err(|_| Failure::Protocol)?;
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(Failure::Configuration)?;
    let mut written = 0;
    while written < command.len() {
        check_deadline(deadline)?;
        match stream.write(&command[written..]) {
            Ok(0) => return Err(Failure::Lost),
            Ok(n) => written += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => wait(),
            Err(_) => return Err(Failure::Lost),
        }
    }
    let mut chunk = [0; 64];
    while !decoder.complete() {
        check_deadline(deadline)?;
        match stream.read(&mut chunk) {
            Ok(0) => return Err(Failure::Lost),
            Ok(n) => decoder
                .accept(session, &chunk[..n])
                .map_err(|_| Failure::Protocol)?,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => wait(),
            Err(_) => return Err(Failure::Lost),
        }
    }
    check_deadline(deadline)?;
    let response = decoder.finish(session).map_err(|_| Failure::Protocol)?;
    if response.kind != 131 {
        return Err(Failure::Refused);
    }
    output[..response.payload.len()].copy_from_slice(response.payload);
    Ok(response.payload.len())
}
fn check_deadline(deadline: Instant) -> Result<(), Failure> {
    if Instant::now() >= deadline {
        Err(Failure::Timeout)
    } else {
        Ok(())
    }
}
fn wait() {
    std::thread::sleep(Duration::from_millis(1));
}
#[cfg(test)]
#[path = "device_carrier/tests.rs"]
mod tests;

fn release_dtr(file: &File) {
    #[cfg(test)]
    DTR_RELEASE_ATTEMPTS.with(|count| count.set(count.get() + 1));
    let bits = libc::TIOCM_DTR;
    unsafe {
        libc::ioctl(file.as_raw_fd(), libc::TIOCMBIC, &bits);
    }
}
#[cfg(test)]
std::thread_local! {static DTR_RELEASE_ATTEMPTS:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
