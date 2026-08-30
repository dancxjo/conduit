//! Exact Linux character-device Base for the iRobot Create Open Interface.
//!
//! This is a physical provider below portable robotics meaning. Opening a path
//! does not advertise a robot capability or prove that a Create responded.

use conduit_create_oi::{CreateUartProvider, UartProfile};
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::PathBuf;

pub const MAXIMUM_CREATE_UART_WRITE_WAIT_MS: u32 = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdCreateUartObservation {
    pub base_id: String,
    pub device_path: PathBuf,
    pub profile: UartProfile,
    pub maximum_write_wait_ms: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StdCreateUartIdentity {
    pub base_id: String,
    pub device_path: PathBuf,
    pub device_number: u64,
    pub profile: UartProfile,
    pub maximum_write_wait_ms: u32,
}

#[derive(Debug)]
pub enum StdCreateUartOpenError {
    MissingBaseIdentity,
    WrongProfile(UartProfile),
    InvalidWriteWait,
    PathContainsNul,
    Metadata(io::Error),
    NotCharacterDevice,
    Open(io::Error),
    ObserveDeviceIdentity(io::Error),
    PathIdentityChanged,
    ObserveTermios(io::Error),
    ConfigureTermios(io::Error),
    VerifyTermios,
}

#[derive(Debug)]
pub enum StdCreateUartIoError {
    Poll(io::Error),
    DescriptorFailure,
    Read(io::Error),
    Write(io::Error),
    WriteTimedOut,
}

pub struct StdCreateUartBase {
    fd: OwnedFd,
    identity: StdCreateUartIdentity,
    saved_termios: libc::termios,
    available: bool,
}

impl StdCreateUartBase {
    pub fn open(observation: StdCreateUartObservation) -> Result<Self, StdCreateUartOpenError> {
        if observation.base_id.is_empty() {
            return Err(StdCreateUartOpenError::MissingBaseIdentity);
        }
        if !observation.profile.is_create_oi() {
            return Err(StdCreateUartOpenError::WrongProfile(observation.profile));
        }
        if observation.maximum_write_wait_ms == 0
            || observation.maximum_write_wait_ms > MAXIMUM_CREATE_UART_WRITE_WAIT_MS
        {
            return Err(StdCreateUartOpenError::InvalidWriteWait);
        }
        let metadata = std::fs::metadata(&observation.device_path)
            .map_err(StdCreateUartOpenError::Metadata)?;
        if !metadata.file_type().is_char_device() {
            return Err(StdCreateUartOpenError::NotCharacterDevice);
        }
        let path = CString::new(observation.device_path.as_os_str().as_bytes())
            .map_err(|_| StdCreateUartOpenError::PathContainsNul)?;
        let raw_fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDWR | libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if raw_fd < 0 {
            return Err(StdCreateUartOpenError::Open(io::Error::last_os_error()));
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };
        let opened_device_number = opened_character_device_number(fd.as_raw_fd())?;
        if opened_device_number != metadata.rdev() {
            return Err(StdCreateUartOpenError::PathIdentityChanged);
        }
        let saved_termios = observe_termios(fd.as_raw_fd())?;
        configure_create_oi(fd.as_raw_fd(), &saved_termios)?;
        verify_create_oi(fd.as_raw_fd())?;
        Ok(Self {
            fd,
            identity: StdCreateUartIdentity {
                base_id: observation.base_id,
                device_path: observation.device_path,
                device_number: opened_device_number,
                profile: observation.profile,
                maximum_write_wait_ms: observation.maximum_write_wait_ms,
            },
            saved_termios,
            available: true,
        })
    }

    pub fn identity(&self) -> &StdCreateUartIdentity {
        &self.identity
    }

    pub fn close(&mut self) {
        self.available = false;
    }
}

impl Drop for StdCreateUartBase {
    fn drop(&mut self) {
        let _ = unsafe { libc::tcsetattr(self.fd.as_raw_fd(), libc::TCSANOW, &self.saved_termios) };
    }
}

impl CreateUartProvider for StdCreateUartBase {
    type Error = StdCreateUartIoError;

    fn is_available(&self) -> bool {
        self.available
    }

    fn profile(&self) -> UartProfile {
        self.identity.profile
    }

    fn write_all(&mut self, mut bytes: &[u8]) -> Result<(), Self::Error> {
        if !self.available {
            return Err(StdCreateUartIoError::DescriptorFailure);
        }
        let deadline_tick = monotonic_millis()?
            .checked_add(u64::from(self.identity.maximum_write_wait_ms))
            .ok_or(StdCreateUartIoError::DescriptorFailure)?;
        while !bytes.is_empty() {
            if !wait_fd_until(self.fd.as_raw_fd(), libc::POLLOUT, deadline_tick)? {
                return Err(StdCreateUartIoError::WriteTimedOut);
            }
            let written =
                unsafe { libc::write(self.fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
            if written < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted
                    || error.kind() == io::ErrorKind::WouldBlock
                {
                    continue;
                }
                return Err(StdCreateUartIoError::Write(error));
            }
            if written == 0 {
                return Err(StdCreateUartIoError::DescriptorFailure);
            }
            bytes = &bytes[written as usize..];
        }
        Ok(())
    }

    fn read_byte(&mut self, deadline_tick: u64) -> Result<Option<u8>, Self::Error> {
        if !self.available {
            return Err(StdCreateUartIoError::DescriptorFailure);
        }
        if !wait_fd_until(self.fd.as_raw_fd(), libc::POLLIN, deadline_tick)? {
            return Ok(None);
        }
        let mut byte = 0_u8;
        loop {
            let read = unsafe { libc::read(self.fd.as_raw_fd(), (&mut byte as *mut u8).cast(), 1) };
            if read == 1 {
                return Ok(Some(byte));
            }
            if read == 0 {
                return Ok(None);
            }
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.kind() == io::ErrorKind::WouldBlock {
                return Ok(None);
            }
            return Err(StdCreateUartIoError::Read(error));
        }
    }
}

pub fn monotonic_millis() -> Result<u64, StdCreateUartIoError> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0
        || time.tv_sec < 0
        || time.tv_nsec < 0
    {
        return Err(StdCreateUartIoError::DescriptorFailure);
    }
    let seconds =
        u64::try_from(time.tv_sec).map_err(|_| StdCreateUartIoError::DescriptorFailure)?;
    let nanos = u64::try_from(time.tv_nsec).map_err(|_| StdCreateUartIoError::DescriptorFailure)?;
    seconds
        .checked_mul(1_000)
        .and_then(|millis| millis.checked_add(nanos / 1_000_000))
        .ok_or(StdCreateUartIoError::DescriptorFailure)
}

fn observe_termios(fd: RawFd) -> Result<libc::termios, StdCreateUartOpenError> {
    let mut termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
        return Err(StdCreateUartOpenError::ObserveTermios(
            io::Error::last_os_error(),
        ));
    }
    Ok(termios)
}

fn opened_character_device_number(fd: RawFd) -> Result<u64, StdCreateUartOpenError> {
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(fd, &mut stat) } != 0 {
        return Err(StdCreateUartOpenError::ObserveDeviceIdentity(
            io::Error::last_os_error(),
        ));
    }
    if stat.st_mode & libc::S_IFMT != libc::S_IFCHR {
        return Err(StdCreateUartOpenError::NotCharacterDevice);
    }
    Ok(stat.st_rdev)
}

fn configure_create_oi(fd: RawFd, observed: &libc::termios) -> Result<(), StdCreateUartOpenError> {
    let mut termios = *observed;
    unsafe { libc::cfmakeraw(&mut termios) };
    termios.c_cflag &= !(libc::CSIZE | libc::CSTOPB | libc::PARENB);
    termios.c_cflag |= libc::CS8 | libc::CLOCAL | libc::CREAD;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        termios.c_cflag &= !libc::CRTSCTS;
    }
    termios.c_cc[libc::VMIN] = 0;
    termios.c_cc[libc::VTIME] = 0;
    if unsafe { libc::cfsetispeed(&mut termios, libc::B57600) } != 0
        || unsafe { libc::cfsetospeed(&mut termios, libc::B57600) } != 0
        || unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } != 0
        || unsafe { libc::tcflush(fd, libc::TCIOFLUSH) } != 0
    {
        return Err(StdCreateUartOpenError::ConfigureTermios(
            io::Error::last_os_error(),
        ));
    }
    Ok(())
}

fn verify_create_oi(fd: RawFd) -> Result<(), StdCreateUartOpenError> {
    let termios = observe_termios(fd)?;
    let exact_flags = termios.c_cflag & libc::CSIZE == libc::CS8
        && termios.c_cflag & libc::CSTOPB == 0
        && termios.c_cflag & libc::PARENB == 0
        && termios.c_cflag & (libc::CLOCAL | libc::CREAD) == libc::CLOCAL | libc::CREAD;
    let exact_speed = unsafe { libc::cfgetispeed(&termios) } == libc::B57600
        && unsafe { libc::cfgetospeed(&termios) } == libc::B57600;
    if exact_flags && exact_speed && termios.c_cc[libc::VMIN] == 0 && termios.c_cc[libc::VTIME] == 0
    {
        Ok(())
    } else {
        Err(StdCreateUartOpenError::VerifyTermios)
    }
}

fn wait_fd_until(
    fd: RawFd,
    events: libc::c_short,
    deadline_tick: u64,
) -> Result<bool, StdCreateUartIoError> {
    let mut poll_fd = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    loop {
        let now = monotonic_millis()?;
        if deadline_tick <= now {
            return Ok(false);
        }
        let timeout_ms = (deadline_tick - now).min(i32::MAX as u64) as libc::c_int;
        let result = unsafe { libc::poll(&mut poll_fd, 1, timeout_ms) };
        if result > 0 {
            if poll_fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                return Err(StdCreateUartIoError::DescriptorFailure);
            }
            return Ok(poll_fd.revents & events != 0);
        }
        if result == 0 {
            return Ok(false);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(StdCreateUartIoError::Poll(error));
        }
    }
}

#[cfg(test)]
#[path = "std_create_uart_tests.rs"]
mod tests;
