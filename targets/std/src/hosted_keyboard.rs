//! Generic admitted host-operation boundary for installed keyboard sources.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedKeyboardPoll {
    Pending,
    Event(conduit_human::KeyEvent),
    Cancelled,
    Failed(u16),
}

/// A platform adapter supplies observations only. Planning, scheduling,
/// routing, pressure, and terminal truth remain in the ordinary kernel path.
pub trait HostedKeyboardAdapter: Send {
    fn poll_next(&mut self) -> HostedKeyboardPoll;
}
