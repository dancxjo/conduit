//! Opt-in, finite preparation diagnostics. These records confer no readiness or authority.
use serde::Serialize;
use std::{cell::Cell, io::Write, time::Instant};

pub(super) const ENABLE: &str = "CONDUIT_HOST_STARTUP_TRACE";
const MAXIMUM_RECORDS: u8 = 40;

#[derive(Clone, Copy)]
pub(super) enum Phase {
    ServiceLock,
    BodyLock,
    Recovery,
    OwnershipSelection,
    RetainedScan,
    Installation,
    BootIdentity,
    HostConstruction,
    EquipmentSelection,
    RuntimeIdentity,
    ImageDigest,
    RuntimeState,
    Catalogs,
    BirthEncounter,
    RuntimePublication,
    OwnedBody,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::ServiceLock => "service-lock",
            Self::BodyLock => "body-lock",
            Self::Recovery => "retained-recovery",
            Self::OwnershipSelection => "ownership-selection",
            Self::RetainedScan => "retained-scan",
            Self::Installation => "installation",
            Self::BootIdentity => "boot-identity",
            Self::HostConstruction => "host-construction",
            Self::EquipmentSelection => "equipment-selection",
            Self::RuntimeIdentity => "runtime-identity",
            Self::ImageDigest => "image-digest",
            Self::RuntimeState => "runtime-state",
            Self::Catalogs => "catalogs",
            Self::BirthEncounter => "birth-encounter",
            Self::RuntimePublication => "runtime-publication",
            Self::OwnedBody => "owned-body",
        }
    }
}

pub(super) struct Trace {
    started: Option<Instant>,
    emitted: Cell<u8>,
    prepared: Cell<bool>,
}

#[derive(Serialize)]
struct Record {
    schema: &'static str,
    pid: u32,
    sequence: u8,
    event: &'static str,
    phase: Option<&'static str>,
    elapsed_us: u64,
    duration_us: Option<u64>,
}

impl Trace {
    pub(super) fn from_environment() -> Self {
        let mut trace = Self::disabled();
        if std::env::var_os(ENABLE).as_deref() == Some(std::ffi::OsStr::new("1")) {
            trace.started = Some(Instant::now());
            trace.emit("start", None, None);
        }
        trace
    }

    pub(super) fn disabled() -> Self {
        Self {
            started: None,
            emitted: Cell::new(0),
            prepared: Cell::new(false),
        }
    }

    pub(super) fn run<T>(
        &self,
        phase: Phase,
        action: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if self.started.is_none() {
            return action();
        }
        let mut span = Span {
            trace: self,
            phase,
            started: Instant::now(),
            ended: false,
        };
        self.emit("begin", Some(phase), None);
        let result = action();
        span.finish(if result.is_ok() { "end" } else { "refused" });
        result
    }

    /// Preparation has finished; the existing stdout announcement and control bind remain separate.
    pub(super) fn prepared(&self) {
        self.prepared.set(true);
        self.emit("prepared", None, None);
    }

    fn emit(&self, event: &'static str, phase: Option<Phase>, duration_us: Option<u64>) {
        let Some(started) = self.started else { return };
        let sequence = self.emitted.get();
        if sequence == MAXIMUM_RECORDS {
            return;
        }
        self.emitted.set(sequence + 1);
        let record = Record {
            schema: "conduit.proof/host-startup@1",
            pid: std::process::id(),
            sequence,
            event,
            phase: phase.map(Phase::name),
            elapsed_us: micros(started.elapsed()),
            duration_us,
        };
        // Fixed names and scalar fields only. Logging errors never replace the action's result.
        let mut stderr = std::io::stderr().lock();
        let _ = stderr.write_all(b"CONDUIT_HOST_STARTUP_PHASE ");
        let _ = serde_json::to_writer(&mut stderr, &record);
        let _ = writeln!(stderr);
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        if !self.prepared.get() {
            self.emit("aborted", None, None);
        }
    }
}

struct Span<'a> {
    trace: &'a Trace,
    phase: Phase,
    started: Instant,
    ended: bool,
}

impl Span<'_> {
    fn finish(&mut self, event: &'static str) {
        self.ended = true;
        self.trace.emit(
            event,
            Some(self.phase),
            Some(micros(self.started.elapsed())),
        );
    }
}

impl Drop for Span<'_> {
    fn drop(&mut self) {
        if !self.ended {
            self.finish("unwound");
        }
    }
}

fn micros(duration: std::time::Duration) -> u64 {
    duration.as_micros().try_into().unwrap_or(u64::MAX)
}
