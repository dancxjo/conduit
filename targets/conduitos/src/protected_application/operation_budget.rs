//! One existing Draft RTC allowance for an immutable preparation/publication.
//! Transport phases never replenish it; Source IRQs are not RTC debit.
use conduitos_application_core::{
    MAX_BLOB,
    data::Refusal,
    memory::TRANSFER_BYTES,
    wire::{Envelope, Request, Response},
};

const RTC_TICKS: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Begin,
    Write(u32),
    Preflight,
    Commit,
    Read { offset: u32, total: u32 },
    Done,
    Failed,
}

/// Kept by the existing exclusive Application service, not a second authority.
pub(super) struct OperationBudget {
    envelope: Envelope,
    input_bytes: u32,
    digest: [u8; 32],
    remaining: u32,
    turns: u16,
    maximum_turns: u16,
    phase: Phase,
    pending: Option<Request>,
    observed: bool,
}

impl OperationBudget {
    pub(super) fn new(
        envelope: Envelope,
        input_bytes: usize,
        digest: [u8; 32],
    ) -> Result<Self, Refusal> {
        if envelope.instance == 0
            || envelope.sequence == 0
            || input_bytes == 0
            || input_bytes > MAX_BLOB
        {
            return Err(Refusal::Bounds);
        }
        Ok(Self {
            envelope,
            input_bytes: input_bytes as u32,
            digest,
            remaining: RTC_TICKS,
            turns: 0,
            maximum_turns: (3
                + input_bytes.div_ceil(TRANSFER_BYTES)
                + MAX_BLOB.div_ceil(TRANSFER_BYTES)) as u16,
            phase: Phase::Begin,
            pending: None,
            observed: false,
        })
    }

    pub(super) fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Every zero-IRQ turn must still advance this finite byte/phase sequence.
    pub(super) fn before(&mut self, envelope: Envelope, request: Request) -> Result<u32, Refusal> {
        let valid = envelope == self.envelope
            && self.pending.is_none()
            && self.remaining > 0
            && self.turns < self.maximum_turns
            && match (self.phase, request) {
                (Phase::Begin, Request::Begin { bytes }) => bytes == self.input_bytes,
                (Phase::Write(at), Request::Write { offset, bytes }) => {
                    offset == at && bytes == (self.input_bytes - at).min(TRANSFER_BYTES as u32)
                }
                (Phase::Preflight, Request::Preflight) | (Phase::Commit, Request::Commit) => true,
                (Phase::Read { offset: at, total }, Request::Read { offset, maximum }) => {
                    offset == at && maximum == (total - at).min(TRANSFER_BYTES as u32)
                }
                _ => false,
            };
        if !valid {
            return self.fail();
        }
        self.turns += 1;
        self.pending = Some(request);
        self.observed = false;
        Ok(self.remaining)
    }

    /// Only the native immutable pre-release receipt supplies these facts.
    /// A fault or an invalid observation poisons the entire logical operation.
    pub(super) fn observe(
        &mut self,
        envelope: Envelope,
        admitted: u32,
        consumed: u32,
        yielded: bool,
    ) -> Result<(), Refusal> {
        if envelope != self.envelope
            || self.pending.is_none()
            || self.observed
            || admitted != self.remaining
            || consumed > admitted
            || !yielded
        {
            return self.fail();
        }
        self.remaining -= consumed;
        self.observed = true;
        Ok(())
    }

    pub(super) fn respond(&mut self, response: Response) -> Result<(), Refusal> {
        if !self.observed {
            return self.fail();
        }
        let next = match (self.phase, self.pending, response) {
            (Phase::Begin, Some(Request::Begin { .. }), Response::Ready) => Phase::Write(0),
            (Phase::Write(at), Some(Request::Write { bytes, .. }), Response::Ready) => {
                let written = at + bytes;
                if written == self.input_bytes {
                    Phase::Preflight
                } else {
                    Phase::Write(written)
                }
            }
            (Phase::Preflight, Some(Request::Preflight), Response::Ready) => Phase::Commit,
            (Phase::Commit, Some(Request::Commit), Response::Blob { bytes })
                if bytes > 0 && bytes as usize <= MAX_BLOB =>
            {
                Phase::Read {
                    offset: 0,
                    total: bytes,
                }
            }
            (
                Phase::Read {
                    offset: at,
                    total: expected,
                },
                Some(Request::Read { maximum, .. }),
                Response::Chunk {
                    offset,
                    bytes,
                    total,
                },
            ) if offset == at && total == expected && bytes > 0 && bytes <= maximum => {
                let read = at + bytes;
                if read == total {
                    Phase::Done
                } else {
                    Phase::Read {
                        offset: read,
                        total,
                    }
                }
            }
            _ => return self.fail(),
        };
        let Some(sequence) = self.envelope.sequence.checked_add(1) else {
            return self.fail();
        };
        self.envelope.sequence = sequence;
        self.phase = next;
        self.pending = None;
        self.observed = false;
        if next != Phase::Done && self.remaining == 0 {
            return self.fail();
        }
        Ok(())
    }

    pub(super) fn complete(&self) -> bool {
        self.phase == Phase::Done
    }

    fn fail<T>(&mut self) -> Result<T, Refusal> {
        self.phase = Phase::Failed;
        self.remaining = 0;
        Err(Refusal::Application)
    }
}

#[cfg(test)]
#[path = "operation_budget_tests.rs"]
mod tests;
