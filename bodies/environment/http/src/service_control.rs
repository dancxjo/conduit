//! Finite request/acknowledgement for an owned service task, not Body lifecycle
//! or authority. Only the task that owns the actual sockets acknowledges them.
use core::cell::RefCell;
use critical_section::Mutex;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceMode {
    Running,
    Station,
    Stopped,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceState {
    Ready,
    StationReady,
    Stopped,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceControlRefusal {
    Exhausted,
    Failed,
    AlreadyClaimed,
}
#[derive(Clone, Copy)]
pub struct ServiceCommand {
    generation: u32,
    pub mode: ServiceMode,
}
#[derive(Clone, Copy)]
struct ServiceAcknowledgement {
    generation: u32,
    state: ServiceState,
}

struct MailboxState {
    claimed: bool,
    observed: Option<ServiceAcknowledgement>,
}

pub struct ServiceMailbox {
    command: Signal<CriticalSectionRawMutex, ServiceCommand>,
    acknowledgement: Signal<CriticalSectionRawMutex, ServiceAcknowledgement>,
    state: Mutex<RefCell<MailboxState>>,
}
impl Default for ServiceMailbox {
    fn default() -> Self {
        Self::new()
    }
}
impl ServiceMailbox {
    pub const fn new() -> Self {
        Self {
            command: Signal::new(),
            acknowledgement: Signal::new(),
            state: Mutex::new(RefCell::new(MailboxState {
                claimed: false,
                observed: None,
            })),
        }
    }
    /// Claim one control handle for this mailbox's lifetime. Dropping a handle
    /// cannot permit generation reuse while old task acknowledgements may exist.
    pub fn claim_control(&self, maximum: u32) -> Result<ServiceControl<'_>, ServiceControlRefusal> {
        critical_section::with(|cs| {
            let mut state = self.state.borrow(cs).borrow_mut();
            if state.claimed {
                return Err(ServiceControlRefusal::AlreadyClaimed);
            }
            state.claimed = true;
            Ok(ServiceControl {
                mailbox: self,
                generation: 0,
                maximum,
            })
        })
    }
    /// Exactly one retained service task may consume this mailbox. Replacing
    /// pending commands is deliberate: a cancelled request cannot force an
    /// obsolete restart after the owner has requested a newer stop.
    pub async fn next_command(&self) -> ServiceCommand {
        self.command.wait().await
    }
    /// Call Ready only after binding every service; Stopped only after dropping
    /// every socket and in-flight request. Failed revokes prior readiness too.
    pub fn acknowledge(&self, command: ServiceCommand, state: ServiceState) {
        let acknowledgement = ServiceAcknowledgement {
            generation: command.generation,
            state,
        };
        let accepted = critical_section::with(|cs| {
            let mut state = self.state.borrow(cs).borrow_mut();
            if state
                .observed
                .is_some_and(|old| old.generation > command.generation)
            {
                return false;
            }
            state.observed = Some(acknowledgement);
            true
        });
        if !accepted {
            return;
        }
        self.acknowledgement.signal(acknowledgement);
    }
    /// Local service evidence. This is not a Host presence or attachment sign.
    pub fn observed_state(&self) -> Option<ServiceState> {
        critical_section::with(|cs| self.state.borrow(cs).borrow().observed.map(|ack| ack.state))
    }
}

pub struct ServiceControl<'a> {
    mailbox: &'a ServiceMailbox,
    generation: u32,
    maximum: u32,
}
impl<'a> ServiceControl<'a> {
    /// Cancellation retains task ownership. A later call supersedes the pending
    /// desired mode and waits for its own acknowledgement. An old acknowledgement
    /// cannot satisfy a new request. Root supplies any required absolute deadline.
    pub async fn request(&mut self, mode: ServiceMode) -> Result<(), ServiceControlRefusal> {
        if self.generation == self.maximum {
            return Err(ServiceControlRefusal::Exhausted);
        }
        self.generation += 1;
        let command = ServiceCommand {
            generation: self.generation,
            mode,
        };
        self.mailbox.command.signal(command);
        loop {
            let ack = self.mailbox.acknowledgement.wait().await;
            if ack.generation != command.generation {
                continue;
            }
            let expected = match mode {
                ServiceMode::Running => ServiceState::Ready,
                ServiceMode::Station => ServiceState::StationReady,
                ServiceMode::Stopped => ServiceState::Stopped,
            };
            return if ack.state == expected {
                Ok(())
            } else {
                Err(ServiceControlRefusal::Failed)
            };
        }
    }
}
