//! Test-only Linux observations/injections; never consumes worker pipe bytes.
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::OpenOptionsExt,
    },
    sync::Arc,
    time::{Duration, Instant},
};
fn bounded(path: impl AsRef<std::path::Path>, maximum: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    File::open(path)
        .unwrap()
        .take(maximum + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() as u64 <= maximum, "bounded proc inventory");
    bytes
}
pub fn children() -> BTreeSet<u32> {
    let bytes = bounded("/proc/thread-self/children", 4096);
    let text = std::str::from_utf8(&bytes).unwrap();
    let children = text
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect::<BTreeSet<_>>();
    assert!(children.len() <= 64, "finite child inventory");
    children
}
fn stat(pid: u32) -> Option<(u64, char)> {
    let mut bytes = Vec::new();
    File::open(format!("/proc/{pid}/stat"))
        .ok()?
        .take(4097)
        .read_to_end(&mut bytes)
        .ok()?;
    assert!(bytes.len() <= 4096, "bounded proc stat");
    let text = std::str::from_utf8(&bytes).unwrap();
    let (_, fields) = text.rsplit_once(") ").unwrap();
    let fields = fields.split_whitespace().collect::<Vec<_>>();
    Some((
        fields[19].parse().unwrap(),
        fields[0].chars().next().unwrap(),
    ))
}
pub struct Worker {
    pub pid: u32,
    pub starttime: u64,
    pidfd: OwnedFd,
}
impl Worker {
    pub fn discover(before: &BTreeSet<u32>) -> Arc<Self> {
        let after = children();
        let added = after.difference(before).copied().collect::<Vec<_>>();
        assert_eq!(added.len(), 1, "exactly one prepared native child");
        let pid = added[0];
        let starttime = stat(pid).expect("prepared child exists").0;
        let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        assert!(
            raw >= 0,
            "pidfd required for race-safe test signals: {}",
            std::io::Error::last_os_error()
        );
        let pidfd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
        assert_eq!(
            stat(pid).unwrap().0,
            starttime,
            "PID identity changed while opening pidfd"
        );
        Arc::new(Self {
            pid,
            starttime,
            pidfd,
        })
    }
    pub fn alive(&self) -> bool {
        stat(self.pid).is_some_and(|(start, _)| start == self.starttime)
    }
    pub fn signal(&self, signal: i32) -> Result<(), String> {
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.pidfd.as_raw_fd(),
                signal,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().to_string())
        }
    }
    pub fn maps(&self) -> Vec<u8> {
        assert!(self.alive());
        bounded(format!("/proc/{}/maps", self.pid), 256 * 1024)
    }
    pub fn stop(&self) {
        self.signal(libc::SIGSTOP).unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            assert!(Instant::now() < until, "worker stop observation timed out");
            let (start, state) = stat(self.pid).expect("worker vanished before stop");
            assert_eq!(start, self.starttime);
            if matches!(state, 'T' | 't') {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn request_observer(&self) -> File {
        assert!(self.alive());
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(format!("/proc/{}/fd/0", self.pid))
            .expect("non-consuming pipe observer")
    }
    pub fn wait_empty(&self, pipe: &File, until: Instant) {
        loop {
            assert!(self.alive(), "worker vanished before admission ACK drained");
            let mut bytes = 0i32;
            assert_eq!(
                unsafe { libc::ioctl(pipe.as_raw_fd(), libc::FIONREAD, &mut bytes) },
                0
            );
            if bytes == 0 {
                return;
            }
            assert!(Instant::now() < until, "preparation ACK did not drain");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn wait_request(&self, pipe: &File, until: Instant) -> Result<i32, String> {
        loop {
            if !self.alive() {
                return Err("worker vanished before queued request".into());
            }
            let mut bytes = 0i32;
            if unsafe { libc::ioctl(pipe.as_raw_fd(), libc::FIONREAD, &mut bytes) } != 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if bytes != 0 {
                return if bytes == 9 {
                    Ok(bytes)
                } else {
                    Err(format!("unexpected queued bytes {bytes}"))
                };
            }
            if Instant::now() >= until {
                return Err("native request barrier timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn wait_exited(&self, until: Instant) {
        loop {
            match stat(self.pid) {
                Some((start, 'Z')) if start == self.starttime => return,
                None => return,
                Some((start, _)) if start != self.starttime => return,
                _ => {}
            }
            assert!(
                Instant::now() < until,
                "SIGKILL did not retire worker before Play"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn wait_gone(&self, until: Instant) -> Result<(), String> {
        while self.alive() {
            if Instant::now() >= until {
                return Err("worker was not reaped within test safety bound".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
    pub fn cleanup(&self) {
        let _ = self.signal(libc::SIGCONT);
        let _ = self.signal(libc::SIGKILL);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cleanup();
    }
}
