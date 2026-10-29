//! Allocation-free sequential scan custody. No SDK status is invented here.
/// Raw SDK observation: unknown authentication values remain unknown to callers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Record {
    /// Original SSID bytes, with unused trailing storage zeroed.
    pub ssid: [u8; 32],
    /// Initialized SSID extent; never greater than thirty-two.
    pub ssid_len: u8,
    /// Signal strength reported by the SDK, in dBm.
    pub rssi: i8,
    /// Raw SDK authentication code, including unrecognized future suites.
    pub authentication: u32,
}
impl Record {
    /// Copy one SDK SSID up to its first NUL; refuse an extent over 32 bytes.
    pub fn from_sdk(ssid: &[u8], rssi: i8, authentication: u32) -> Option<Self> {
        let length = ssid
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(ssid.len());
        if length > 32 {
            return None;
        }
        let mut record = Self {
            ssid: [0; 32],
            ssid_len: length as u8,
            rssi,
            authentication,
        };
        record.ssid[..length].copy_from_slice(&ssid[..length]);
        Some(record)
    }
}
/// Exact SDK ScanDone fields, observed under exclusive sequential custody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    /// SDK terminal status; zero denotes successful completion.
    pub status: u32,
    /// Record count carried by the SDK terminal event.
    pub number: u8,
    /// SDK event identifier, observed rather than guessed before scan start.
    pub scan_id: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Running,
    Terminal(Completion),
    Uncorrelated,
}
#[derive(Debug)]
pub struct Custody {
    phase: Phase,
    stop_requested: bool,
    poisoned: bool,
}
impl Custody {
    pub const fn quiescent() -> Self {
        Self {
            phase: Phase::Idle,
            stop_requested: false,
            poisoned: false,
        }
    }
    pub fn stop_requested(&self) -> bool {
        self.stop_requested
    }
    pub fn is_quiescent(&self) -> bool {
        self.phase == Phase::Idle
    }
    pub fn is_quarantined(&self) -> bool {
        self.poisoned
    }
    pub fn begin(&mut self) -> bool {
        if !self.is_quiescent() {
            return false;
        }
        self.phase = Phase::Running;
        self.stop_requested = false;
        self.poisoned = false;
        true
    }
    pub fn quarantine(&mut self) {
        self.poisoned = true;
        if self.phase == Phase::Idle {
            self.phase = Phase::Uncorrelated;
        }
    }
    /// True only once, and only while the accepted operation may still run.
    /// SDK stop return alone cannot change phase or prove termination.
    pub fn request_stop(&mut self) -> bool {
        if self.is_quiescent() || self.completion().is_some() || self.stop_requested {
            return false;
        }
        self.stop_requested = true;
        true
    }
    pub fn observed(&mut self, completion: Completion) {
        match self.phase {
            Phase::Running => self.phase = Phase::Terminal(completion),
            // An event without a retained invocation cannot establish a new
            // correlation. Publication poison survives a real terminal event.
            _ => self.quarantine(),
        }
    }
    pub fn completion(&self) -> Option<Completion> {
        match self.phase {
            Phase::Terminal(event) => Some(event),
            _ => None,
        }
    }
    pub fn cleared(&mut self) -> bool {
        if self.completion().is_none() {
            return false;
        }
        self.phase = Phase::Idle;
        true
    }
}
/// Copy only the admitted prefix. Overflow is reported by the caller using the
/// actual SDK total; no omitted record is silently represented as observed.
pub fn copy_prefix<T, E>(
    out: &mut [T],
    total: u16,
    read: &mut impl FnMut() -> Result<T, E>,
) -> Result<usize, E> {
    let count = usize::from(total).min(out.len()).min(16);
    for slot in &mut out[..count] {
        *slot = read()?;
    }
    Ok(count)
}
#[cfg(test)]
#[path = "custody_tests.rs"]
mod tests;
