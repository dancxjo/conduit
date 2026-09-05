//! Correlated, evidence-honest lifecycle for one finite record delivery.

use alloc::vec::Vec;

use crate::MAXIMUM_TYPED_RECORD_FRAME_BYTES;

pub const MAXIMUM_RECORD_CORRELATION_BYTES: usize = 128;
pub const MAXIMUM_RECORD_RECEIPT_BYTES: usize = 128;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecordDeliveryStateRef<'a> {
    LocallyAccepted,
    FramedQueued { queue_sequence: u64 },
    PartiallySent { sent_bytes: u32, frame_bytes: u32 },
    RemoteAccepted { receipt: &'a [u8] },
    TransportUnavailable { code: u16 },
    Disconnected { code: u16 },
    TimedOut { code: u16 },
    Refused { code: u16 },
    Failed { code: u16 },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum RecordDeliveryState {
    LocallyAccepted,
    FramedQueued { queue_sequence: u64 },
    PartiallySent { sent_bytes: u32 },
    RemoteAccepted,
    TransportUnavailable { code: u16 },
    Disconnected { code: u16 },
    TimedOut { code: u16 },
    Refused { code: u16 },
    Failed { code: u16 },
}

impl RecordDeliveryState {
    const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::RemoteAccepted
                | Self::TransportUnavailable { .. }
                | Self::Disconnected { .. }
                | Self::TimedOut { .. }
                | Self::Refused { .. }
                | Self::Failed { .. }
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecordDeliveryRefusal {
    EmptyCorrelation,
    CorrelationTooLong,
    EmptyFrame,
    FrameTooLarge,
    InvalidTransition,
    InvalidPartialProgress,
    EmptyReceipt,
    ReceiptTooLong,
}

/// Allocation-stable after construction. This tracker reports observations;
/// it owns neither a Line nor retry, timeout, or reconnection policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDeliveryTracker {
    correlation: Vec<u8>,
    receipt: Vec<u8>,
    frame_bytes: u32,
    state: RecordDeliveryState,
}

impl RecordDeliveryTracker {
    pub fn locally_accepted(
        correlation: &[u8],
        frame_bytes: usize,
    ) -> Result<Self, RecordDeliveryRefusal> {
        if correlation.is_empty() {
            return Err(RecordDeliveryRefusal::EmptyCorrelation);
        }
        if correlation.len() > MAXIMUM_RECORD_CORRELATION_BYTES {
            return Err(RecordDeliveryRefusal::CorrelationTooLong);
        }
        if frame_bytes == 0 {
            return Err(RecordDeliveryRefusal::EmptyFrame);
        }
        if frame_bytes > MAXIMUM_TYPED_RECORD_FRAME_BYTES {
            return Err(RecordDeliveryRefusal::FrameTooLarge);
        }
        let frame_bytes = frame_bytes as u32;
        let mut owned_correlation = Vec::with_capacity(MAXIMUM_RECORD_CORRELATION_BYTES);
        owned_correlation.extend_from_slice(correlation);
        Ok(Self {
            correlation: owned_correlation,
            receipt: Vec::with_capacity(MAXIMUM_RECORD_RECEIPT_BYTES),
            frame_bytes,
            state: RecordDeliveryState::LocallyAccepted,
        })
    }

    pub fn framed_queued(&mut self, queue_sequence: u64) -> Result<(), RecordDeliveryRefusal> {
        self.transition(
            matches!(self.state, RecordDeliveryState::LocallyAccepted),
            RecordDeliveryState::FramedQueued { queue_sequence },
        )
    }

    pub fn partially_sent(&mut self, sent_bytes: usize) -> Result<(), RecordDeliveryRefusal> {
        let sent_bytes =
            u32::try_from(sent_bytes).map_err(|_| RecordDeliveryRefusal::InvalidPartialProgress)?;
        let previous = match self.state {
            RecordDeliveryState::FramedQueued { .. } => 0,
            RecordDeliveryState::PartiallySent { sent_bytes } => sent_bytes,
            _ => return Err(RecordDeliveryRefusal::InvalidTransition),
        };
        if sent_bytes <= previous || sent_bytes >= self.frame_bytes {
            return Err(RecordDeliveryRefusal::InvalidPartialProgress);
        }
        self.state = RecordDeliveryState::PartiallySent { sent_bytes };
        Ok(())
    }

    pub fn remote_accepted(&mut self, receipt: &[u8]) -> Result<(), RecordDeliveryRefusal> {
        if !matches!(
            self.state,
            RecordDeliveryState::FramedQueued { .. } | RecordDeliveryState::PartiallySent { .. }
        ) {
            return Err(RecordDeliveryRefusal::InvalidTransition);
        }
        if receipt.is_empty() {
            return Err(RecordDeliveryRefusal::EmptyReceipt);
        }
        if receipt.len() > MAXIMUM_RECORD_RECEIPT_BYTES {
            return Err(RecordDeliveryRefusal::ReceiptTooLong);
        }
        self.receipt.clear();
        self.receipt.extend_from_slice(receipt);
        self.state = RecordDeliveryState::RemoteAccepted;
        Ok(())
    }

    pub fn transport_unavailable(&mut self, code: u16) -> Result<(), RecordDeliveryRefusal> {
        self.fail(RecordDeliveryState::TransportUnavailable { code })
    }

    pub fn disconnected(&mut self, code: u16) -> Result<(), RecordDeliveryRefusal> {
        self.fail(RecordDeliveryState::Disconnected { code })
    }

    pub fn timed_out(&mut self, code: u16) -> Result<(), RecordDeliveryRefusal> {
        self.fail(RecordDeliveryState::TimedOut { code })
    }

    pub fn refused(&mut self, code: u16) -> Result<(), RecordDeliveryRefusal> {
        self.fail(RecordDeliveryState::Refused { code })
    }

    pub fn failed(&mut self, code: u16) -> Result<(), RecordDeliveryRefusal> {
        self.fail(RecordDeliveryState::Failed { code })
    }

    fn fail(&mut self, state: RecordDeliveryState) -> Result<(), RecordDeliveryRefusal> {
        self.transition(!self.state.is_terminal(), state)
    }

    fn transition(
        &mut self,
        allowed: bool,
        state: RecordDeliveryState,
    ) -> Result<(), RecordDeliveryRefusal> {
        if !allowed {
            return Err(RecordDeliveryRefusal::InvalidTransition);
        }
        self.state = state;
        Ok(())
    }

    pub fn correlation(&self) -> &[u8] {
        &self.correlation
    }

    pub const fn frame_bytes(&self) -> u32 {
        self.frame_bytes
    }

    pub const fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    pub fn state(&self) -> RecordDeliveryStateRef<'_> {
        match self.state {
            RecordDeliveryState::LocallyAccepted => RecordDeliveryStateRef::LocallyAccepted,
            RecordDeliveryState::FramedQueued { queue_sequence } => {
                RecordDeliveryStateRef::FramedQueued { queue_sequence }
            }
            RecordDeliveryState::PartiallySent { sent_bytes } => {
                RecordDeliveryStateRef::PartiallySent {
                    sent_bytes,
                    frame_bytes: self.frame_bytes,
                }
            }
            RecordDeliveryState::RemoteAccepted => RecordDeliveryStateRef::RemoteAccepted {
                receipt: &self.receipt,
            },
            RecordDeliveryState::TransportUnavailable { code } => {
                RecordDeliveryStateRef::TransportUnavailable { code }
            }
            RecordDeliveryState::Disconnected { code } => {
                RecordDeliveryStateRef::Disconnected { code }
            }
            RecordDeliveryState::TimedOut { code } => RecordDeliveryStateRef::TimedOut { code },
            RecordDeliveryState::Refused { code } => RecordDeliveryStateRef::Refused { code },
            RecordDeliveryState::Failed { code } => RecordDeliveryStateRef::Failed { code },
        }
    }

    pub fn allocated_capacities(&self) -> (usize, usize) {
        (self.correlation.capacity(), self.receipt.capacity())
    }
}
