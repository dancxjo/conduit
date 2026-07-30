//! Hosted effect commit witnesses and deterministic fault injection.
//!
//! These helpers operate only on resources selected and opened by a host.
//! They do not discover ambient files, processes, or sockets.

use conduit_core::{
    AuthorityTime, EffectAttemptState, Id, InstancePath, ResourceLeaseReason, ResourceLeaseState,
};

/// A deterministic failure point relative to the domain-owned commit call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeterministicEffectFault {
    None,
    BeforeCommit,
    AfterCommitBeforeAcknowledgement,
    DuringCleanup,
}

/// The only successful or non-final dispositions produced by the harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedEffectDisposition {
    Acknowledged,
    FailedBeforeCommit,
    CommitUnknown,
    CleanupPending,
    Cleaned,
}

#[derive(Debug)]
pub enum HostedEffectError<E> {
    Lease(ResourceLeaseReason),
    Effect(ResourceLeaseReason),
    Provider(E),
}

/// Exact use facts supplied by the already resolved run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedLeaseUse<'a> {
    pub resource_binding: Id<'a>,
    pub holder: InstancePath<'a>,
    pub run: Id<'a>,
    pub epoch: u64,
    pub now: AuthorityTime<'a>,
}

/// One-shot deterministic backend used by conformance and provider tests.
///
/// Admission consumes one lease operation and reserves the attempt's complete
/// evidence allowance before `commit` is invoked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeterministicEffectBackend {
    fault: DeterministicEffectFault,
}

impl DeterministicEffectBackend {
    #[must_use]
    pub const fn new(fault: DeterministicEffectFault) -> Self {
        Self { fault }
    }

    pub fn execute<E>(
        &self,
        lease: &mut ResourceLeaseState<'_>,
        attempt: &mut EffectAttemptState<'_>,
        lease_use: HostedLeaseUse<'_>,
        commit: impl FnOnce() -> Result<(), E>,
    ) -> Result<HostedEffectDisposition, HostedEffectError<E>> {
        lease
            .begin_operation(
                lease_use.resource_binding,
                lease_use.holder,
                lease_use.run,
                lease_use.epoch,
                lease_use.now,
            )
            .map_err(HostedEffectError::Lease)?;
        lease
            .reserve_required_evidence(u32::from(attempt.profile().evidence_events_per_attempt))
            .map_err(HostedEffectError::Lease)?;
        attempt.start().map_err(HostedEffectError::Effect)?;

        if self.fault == DeterministicEffectFault::BeforeCommit {
            attempt
                .fail_before_commit()
                .map_err(HostedEffectError::Effect)?;
            return Ok(HostedEffectDisposition::FailedBeforeCommit);
        }

        commit().map_err(HostedEffectError::Provider)?;
        attempt.committed().map_err(HostedEffectError::Effect)?;
        if self.fault == DeterministicEffectFault::AfterCommitBeforeAcknowledgement {
            let _ = attempt.lose_host();
            return Ok(HostedEffectDisposition::CommitUnknown);
        }
        attempt.acknowledge().map_err(HostedEffectError::Effect)?;
        Ok(HostedEffectDisposition::Acknowledged)
    }

    pub fn cleanup<E>(
        &self,
        lease: &mut ResourceLeaseState<'_>,
        attempt: &mut EffectAttemptState<'_>,
        now: AuthorityTime<'_>,
        release_sequence: u64,
        cleanup: impl FnOnce() -> Result<(), E>,
    ) -> Result<HostedEffectDisposition, HostedEffectError<E>> {
        lease.begin_cleanup(now).map_err(HostedEffectError::Lease)?;
        if self.fault == DeterministicEffectFault::DuringCleanup {
            return Ok(HostedEffectDisposition::CleanupPending);
        }
        cleanup().map_err(HostedEffectError::Provider)?;
        attempt
            .cleanup_complete()
            .map_err(HostedEffectError::Effect)?;
        lease
            .complete_cleanup(release_sequence)
            .map_err(HostedEffectError::Lease)?;
        Ok(HostedEffectDisposition::Cleaned)
    }
}

#[cfg(target_os = "linux")]
pub mod linux {
    use std::fs::File;
    use std::io::{self, Write};
    use std::os::unix::net::UnixStream;
    use std::process::{Child, Command};

    /// Write and durably flush an already opened file. The successful
    /// `sync_data` call is this witness's local commit boundary.
    pub fn commit_file(file: &mut File, bytes: &[u8]) -> io::Result<()> {
        file.write_all(bytes)?;
        file.sync_data()
    }

    /// Spawn one already constructed command. Successful `spawn` is the
    /// process-launch commit boundary; it does not claim child completion.
    pub fn commit_process(command: &mut Command) -> io::Result<Child> {
        command.spawn()
    }

    /// Submit all bytes to an already connected local socket. This boundary
    /// proves kernel acceptance only, never remote processing or exactly-once.
    pub fn commit_socket(stream: &mut UnixStream, bytes: &[u8]) -> io::Result<()> {
        stream.write_all(bytes)
    }

    /// Finite process cleanup witness for cancellation/escalation paths.
    pub fn force_kill_and_wait(child: &mut Child) -> io::Result<()> {
        match child.try_wait()? {
            Some(_) => Ok(()),
            None => {
                child.kill()?;
                child.wait().map(|_| ())
            }
        }
    }
}
