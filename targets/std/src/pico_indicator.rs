//! Bounded POSIX CDC provider for the Pico indicator-resource firmware.
//! This is an acquired local peripheral, not a Conduit Line or a remote Host.
use std::{
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::{ErrorKind, Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
    time::{Duration, Instant},
};

use crate::hosted_indicator::{
    HostedIndicatorAdapter, IndicatorBinding, IndicatorFailure, IndicatorRequest,
};
use conduit_core::HostAdvertisement;

const BYTES: usize = 96;
type Frame = [u8; BYTES];
mod provenance;

struct Port(File);

impl Drop for Port {
    fn drop(&mut self) {
        // End the acquisition even when handshake or effect validation fails.
        // This asks the peripheral to perform fault cleanup; it is not an ACK.
        let bits = libc::TIOCM_DTR;
        unsafe {
            libc::ioctl(self.0.as_raw_fd(), libc::TIOCMBIC, &bits);
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IndicatorReceipt {
    pub request: u64,
    pub state: bool,
    pub play_correlation: [u8; 16],
}

/// Provider-asserted device evidence, not a globally unique USB serial or a
/// claim that a human observed the LED. ACK follows the firmware GPIO call.
pub struct PicoIndicator {
    port: Port,
    binding: IndicatorBinding,
    acquired: Frame,
    timeout: Duration,
    receipts: [IndicatorReceipt; 8],
    completed: usize,
    failure: Option<IndicatorFailure>,
}

impl PicoIndicator {
    /// Acquire before advertising/planning. Each acquisition creates a new
    /// pool identity; publish that exact pool and the ordinary indicator offer.
    /// `expected_build` must come from the intended built firmware identity,
    /// not from accepting whatever digest the connected endpoint reports.
    pub fn acquire(
        path: &Path,
        host: &HostAdvertisement,
        expected_build: [u8; 32],
        timeout: Duration,
    ) -> Result<Self, IndicatorFailure> {
        if timeout.is_zero() || timeout > Duration::from_secs(5) {
            return Err(IndicatorFailure::InvalidInput);
        }
        let port = Port(
            OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
                .open(path)
                .map_err(|_| IndicatorFailure::Lost)?,
        );
        let file = &port.0;
        // Exclusive TTY acquisition plus cooperative locking. This does not
        // revoke descriptors already held by another process or confine root.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0
            || unsafe { libc::ioctl(file.as_raw_fd(), libc::TIOCEXCL as _) } != 0
        {
            return Err(IndicatorFailure::Lost);
        }
        crate::usb_cdc::configure_cdc_port(file, 1, 0).map_err(|_| IndicatorFailure::Lost)?;
        let bits = libc::TIOCM_DTR;
        if unsafe { libc::ioctl(file.as_raw_fd(), libc::TIOCMBIS, &bits) } != 0 {
            return Err(IndicatorFailure::Lost);
        }
        let mut nonce = [0; 16];
        File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut nonce))
            .map_err(|_| IndicatorFailure::Lost)?;
        Self::acquire_port(port, host, expected_build, nonce, timeout)
    }

    fn acquire_port(
        mut port: Port,
        host: &HostAdvertisement,
        expected_build: [u8; 32],
        nonce: [u8; 16],
        timeout: Duration,
    ) -> Result<Self, IndicatorFailure> {
        if nonce == [0; 16] || expected_build == [0; 32] {
            return Err(IndicatorFailure::InvalidInput);
        }
        let mut hello = [0; BYTES];
        hello[..4].copy_from_slice(b"CIR1");
        hello[4] = 1;
        hello[8..24].copy_from_slice(&nonce);
        let acquired = exchange(&mut port.0, &hello, timeout)?;
        if acquired[..4] != hello[..4]
            || acquired[4] != 2
            || acquired[5..8] != [0; 3]
            || acquired[72..] != [0; 24]
        {
            return Err(IndicatorFailure::MalformedReceipt);
        }
        if acquired[8..24] != nonce
            || acquired[24..40] == [0; 16]
            || acquired[40..72] != expected_build
        {
            return Err(IndicatorFailure::StaleIdentity);
        }
        let mut pool = String::from("pico/indicator-acquisition:");
        for byte in nonce {
            write!(pool, "{byte:02x}").expect("String formatting");
        }
        Ok(Self {
            port,
            binding: IndicatorBinding {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                pool_id: pool.into(),
            },
            acquired,
            timeout,
            receipts: [IndicatorReceipt::default(); 8],
            completed: 0,
            failure: None,
        })
    }

    pub fn device_boot(&self) -> &[u8] {
        &self.acquired[24..40]
    }
    pub fn firmware_digest(&self) -> &[u8] {
        &self.acquired[40..72]
    }
    pub fn receipts(&self) -> &[IndicatorReceipt] {
        &self.receipts[..self.completed]
    }

    fn present_exact(&mut self, request: IndicatorRequest<'_>) -> Result<(), IndicatorFailure> {
        if request.play.host_id != self.binding.host_id
            || request.play.boot_id != self.binding.boot_id
            || request.request.0 as usize != self.completed
            || self.completed == 8
        {
            return Err(IndicatorFailure::StaleIdentity);
        }
        let digest = conduit_core::active_play_digest(
            request.play.plan_id.as_str(),
            request.play.host_id.as_str(),
            request.play.boot_id.as_str(),
            request.play.play_sequence,
        );
        let correlation: [u8; 16] = digest[..16].try_into().expect("fixed digest");
        if self.completed != 0 && self.receipts[0].play_correlation != correlation {
            return Err(IndicatorFailure::StaleIdentity);
        }
        let mut command = self.acquired;
        command[4] = 3;
        command[5] = u8::from(request.state.get());
        command[72..80].copy_from_slice(&u64::from(request.request.0).to_le_bytes());
        command[80..96].copy_from_slice(&correlation);
        let receipt = exchange(&mut self.port.0, &command, self.timeout)?;
        if receipt[..4] != command[..4] || receipt[4] != 4 || receipt[6..8] != [0; 2] {
            return Err(IndicatorFailure::MalformedReceipt);
        }
        if receipt[8..] != command[8..] {
            return Err(IndicatorFailure::StaleIdentity);
        }
        if receipt[5] != command[5] {
            return Err(IndicatorFailure::WrongState);
        }
        self.receipts[self.completed] = IndicatorReceipt {
            request: u64::from(request.request.0),
            state: request.state.get(),
            play_correlation: correlation,
        };
        self.completed += 1;
        Ok(())
    }
}

impl HostedIndicatorAdapter for PicoIndicator {
    fn binding(&self) -> &IndicatorBinding {
        &self.binding
    }
    fn present(&mut self, request: IndicatorRequest<'_>) -> Result<(), IndicatorFailure> {
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        let result = self.present_exact(request);
        if let Err(failure) = result {
            self.failure = Some(failure);
        }
        result
    }
}

fn exchange(
    file: &mut File,
    command: &Frame,
    timeout: Duration,
) -> Result<Frame, IndicatorFailure> {
    let deadline = Instant::now() + timeout;
    let mut written = 0;
    while written != BYTES {
        check_deadline(deadline)?;
        match file.write(&command[written..]) {
            Ok(0) => return Err(IndicatorFailure::Lost),
            Ok(count) => written += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => wait_io(),
            Err(_) => return Err(IndicatorFailure::Lost),
        }
    }
    let mut response = [0; BYTES];
    let mut read = 0;
    while read != BYTES {
        check_deadline(deadline)?;
        match file.read(&mut response[read..]) {
            Ok(0) => return Err(IndicatorFailure::Lost),
            Ok(count) => read += count,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) if error.kind() == ErrorKind::WouldBlock => wait_io(),
            Err(_) => return Err(IndicatorFailure::Lost),
        }
    }
    check_deadline(deadline)?;
    Ok(response)
}

fn check_deadline(deadline: Instant) -> Result<(), IndicatorFailure> {
    if Instant::now() >= deadline {
        Err(IndicatorFailure::Timeout)
    } else {
        Ok(())
    }
}
fn wait_io() {
    std::thread::sleep(Duration::from_millis(1));
}

#[cfg(test)]
mod tests;
