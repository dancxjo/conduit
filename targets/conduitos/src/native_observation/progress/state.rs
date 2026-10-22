//! Fixed storage, atomic bookkeeping and bounded formatting for one Root Boot.
use super::Phase;
use core::{
    fmt::{self, Write},
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
pub(super) const MAX_SPANS: u32 = 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Event {
    Started,
    Finished,
    Refused,
    Abandoned,
}
impl Event {
    fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Finished => "finished",
            Self::Refused => "refused",
            Self::Abandoned => "abandoned",
        }
    }
}
pub(super) struct Token {
    id: u32,
    parent: u32,
    phase: Phase,
    context: &'static str,
    start: u64,
}
pub(super) struct Record {
    pub sequence: u32,
    pub span: u32,
    pub parent: u32,
    pub phase: Phase,
    pub context: &'static str,
    pub ticks: u64,
    pub event: Event,
}
pub(super) struct Tracker {
    next_span: AtomicU32,
    next_event: AtomicU32,
    current: AtomicU32,
    failed: AtomicBool,
}
impl Tracker {
    pub const fn new() -> Self {
        Self {
            next_span: AtomicU32::new(1),
            next_event: AtomicU32::new(0),
            current: AtomicU32::new(0),
            failed: AtomicBool::new(false),
        }
    }
    pub fn fail<T>(&self, reason: &'static str) -> Result<Option<T>, &'static str> {
        if self.failed.swap(true, Ordering::Relaxed) {
            Ok(None)
        } else {
            Err(reason)
        }
    }
    pub fn begin(
        &self,
        phase: Phase,
        context: &'static str,
        ticks: u64,
    ) -> Result<Option<(Token, Record)>, &'static str> {
        if self.failed.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if context.is_empty()
            || context.len() > 64
            || !context
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_/.:".contains(&b))
        {
            return self.fail("context-bound");
        }
        let id = match self
            .next_span
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                (next <= MAX_SPANS).then_some(next + 1)
            }) {
            Ok(id) => id,
            Err(_) => return self.fail("span-bound"),
        };
        let parent = self.current.swap(id, Ordering::Relaxed);
        let sequence = self.next_event.fetch_add(1, Ordering::Relaxed);
        Ok(Some((
            Token {
                id,
                parent,
                phase,
                context,
                start: ticks,
            },
            Record {
                sequence,
                span: id,
                parent,
                phase,
                context,
                ticks,
                event: Event::Started,
            },
        )))
    }
    pub fn close(
        &self,
        token: Token,
        event: Event,
        ticks: u64,
    ) -> Result<Option<Record>, &'static str> {
        if self.failed.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if event == Event::Started || ticks < token.start {
            return self.fail("clock-or-event-invalid");
        }
        if self
            .current
            .compare_exchange(token.id, token.parent, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return self.fail("root-scope-order-invalid");
        }
        let sequence = self.next_event.fetch_add(1, Ordering::Relaxed);
        Ok(Some(Record {
            sequence,
            span: token.id,
            parent: token.parent,
            phase: token.phase,
            context: token.context,
            ticks,
            event,
        }))
    }
}
impl Record {
    pub fn write(&self, line: &mut Line) -> fmt::Result {
        writeln!(
            line,
            "CONDUIT_NATIVE_PROGRESS {{\"schema\":\"conduit.conduitos/native-progress@1\",\"scope\":\"native-root-boot\",\"sequence\":{},\"span\":{},\"parent\":{},\"phase\":\"{}\",\"context\":\"{}\",\"state\":\"{}\",\"clock\":\"x86-tsc-lfence\",\"frequency_hz\":null,\"ticks\":{}}}",
            self.sequence,
            self.span,
            self.parent,
            self.phase.as_str(),
            self.context,
            self.event.as_str(),
            self.ticks
        )
    }
}
pub(super) struct Line {
    bytes: [u8; 512],
    length: usize,
}
impl Line {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 512],
            length: 0,
        }
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}
impl Write for Line {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.length..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
