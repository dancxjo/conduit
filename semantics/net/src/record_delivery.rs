//! Correlated, evidence-honest lifecycle for one finite record delivery.

use crate::MAXIMUM_TYPED_RECORD_FRAME_BYTES;

pub const MAXIMUM_RECORD_CORRELATION_BYTES: usize = 128;
pub const MAXIMUM_RECORD_RECEIPT_BYTES: usize = 128;
pub const MAXIMUM_RECORD_DELIVERY_WIRE_BYTES: usize =
    7 + MAXIMUM_RECORD_CORRELATION_BYTES + 1 + MAXIMUM_RECORD_RECEIPT_BYTES;
pub const RECORD_DELIVERY_WIRE_VERSION: u8 = 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecordDeliveryObservationEvent<'a> {
    LocallyAccepted,
    FramedQueued { queue_sequence: u64 },
    PartiallySent { sent_bytes: u32 },
    RemoteAccepted { receipt: &'a [u8] },
    TransportUnavailable { code: u16 },
    Disconnected { code: u16 },
    TimedOut { code: u16 },
    Refused { code: u16 },
    Failed { code: u16 },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RecordDeliveryObservationRef<'a> {
    pub correlation: &'a [u8],
    pub frame_bytes: u32,
    pub event: RecordDeliveryObservationEvent<'a>,
}

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
    OutputTooSmall,
    MalformedWire,
    ObservationIdentityMismatch,
}

/// Allocation-stable after construction. This tracker reports observations;
/// it owns neither a Line nor retry, timeout, or reconnection policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDeliveryTracker {
    correlation: [u8; MAXIMUM_RECORD_CORRELATION_BYTES],
    correlation_len: u8,
    receipt: [u8; MAXIMUM_RECORD_RECEIPT_BYTES],
    receipt_len: u8,
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
        let mut owned_correlation = [0; MAXIMUM_RECORD_CORRELATION_BYTES];
        owned_correlation[..correlation.len()].copy_from_slice(correlation);
        Ok(Self {
            correlation: owned_correlation,
            correlation_len: correlation.len() as u8,
            receipt: [0; MAXIMUM_RECORD_RECEIPT_BYTES],
            receipt_len: 0,
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
        self.receipt[..receipt.len()].copy_from_slice(receipt);
        self.receipt_len = receipt.len() as u8;
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
        &self.correlation[..usize::from(self.correlation_len)]
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
                receipt: &self.receipt[..usize::from(self.receipt_len)],
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

    pub fn observation(&self) -> RecordDeliveryObservationRef<'_> {
        let event = match self.state() {
            RecordDeliveryStateRef::LocallyAccepted => {
                RecordDeliveryObservationEvent::LocallyAccepted
            }
            RecordDeliveryStateRef::FramedQueued { queue_sequence } => {
                RecordDeliveryObservationEvent::FramedQueued { queue_sequence }
            }
            RecordDeliveryStateRef::PartiallySent { sent_bytes, .. } => {
                RecordDeliveryObservationEvent::PartiallySent { sent_bytes }
            }
            RecordDeliveryStateRef::RemoteAccepted { receipt } => {
                RecordDeliveryObservationEvent::RemoteAccepted { receipt }
            }
            RecordDeliveryStateRef::TransportUnavailable { code } => {
                RecordDeliveryObservationEvent::TransportUnavailable { code }
            }
            RecordDeliveryStateRef::Disconnected { code } => {
                RecordDeliveryObservationEvent::Disconnected { code }
            }
            RecordDeliveryStateRef::TimedOut { code } => {
                RecordDeliveryObservationEvent::TimedOut { code }
            }
            RecordDeliveryStateRef::Refused { code } => {
                RecordDeliveryObservationEvent::Refused { code }
            }
            RecordDeliveryStateRef::Failed { code } => {
                RecordDeliveryObservationEvent::Failed { code }
            }
        };
        RecordDeliveryObservationRef {
            correlation: self.correlation(),
            frame_bytes: self.frame_bytes,
            event,
        }
    }

    pub fn apply(
        &mut self,
        observation: RecordDeliveryObservationRef<'_>,
    ) -> Result<(), RecordDeliveryRefusal> {
        if observation.correlation != self.correlation()
            || observation.frame_bytes != self.frame_bytes
        {
            return Err(RecordDeliveryRefusal::ObservationIdentityMismatch);
        }
        match observation.event {
            RecordDeliveryObservationEvent::LocallyAccepted => {
                Err(RecordDeliveryRefusal::InvalidTransition)
            }
            RecordDeliveryObservationEvent::FramedQueued { queue_sequence } => {
                self.framed_queued(queue_sequence)
            }
            RecordDeliveryObservationEvent::PartiallySent { sent_bytes } => {
                self.partially_sent(sent_bytes as usize)
            }
            RecordDeliveryObservationEvent::RemoteAccepted { receipt } => {
                self.remote_accepted(receipt)
            }
            RecordDeliveryObservationEvent::TransportUnavailable { code } => {
                self.transport_unavailable(code)
            }
            RecordDeliveryObservationEvent::Disconnected { code } => self.disconnected(code),
            RecordDeliveryObservationEvent::TimedOut { code } => self.timed_out(code),
            RecordDeliveryObservationEvent::Refused { code } => self.refused(code),
            RecordDeliveryObservationEvent::Failed { code } => self.failed(code),
        }
    }

    pub fn allocated_capacities(&self) -> (usize, usize) {
        (self.correlation.len(), self.receipt.len())
    }
}

pub fn encode_record_delivery_observation_into(
    observation: RecordDeliveryObservationRef<'_>,
    output: &mut [u8],
) -> Result<usize, RecordDeliveryRefusal> {
    validate_identity(observation.correlation, observation.frame_bytes)?;
    let event_bytes = match observation.event {
        RecordDeliveryObservationEvent::LocallyAccepted => 0,
        RecordDeliveryObservationEvent::FramedQueued { .. } => 8,
        RecordDeliveryObservationEvent::PartiallySent { .. } => 4,
        RecordDeliveryObservationEvent::RemoteAccepted { receipt } => {
            if receipt.is_empty() {
                return Err(RecordDeliveryRefusal::EmptyReceipt);
            }
            if receipt.len() > MAXIMUM_RECORD_RECEIPT_BYTES {
                return Err(RecordDeliveryRefusal::ReceiptTooLong);
            }
            1 + receipt.len()
        }
        _ => 2,
    };
    let length = 7 + observation.correlation.len() + event_bytes;
    if output.len() < length {
        return Err(RecordDeliveryRefusal::OutputTooSmall);
    }
    output[0] = RECORD_DELIVERY_WIRE_VERSION;
    output[1] = observation.correlation.len() as u8;
    output[2..6].copy_from_slice(&observation.frame_bytes.to_le_bytes());
    output[6] = event_tag(observation.event);
    let event_start = 7 + observation.correlation.len();
    output[7..event_start].copy_from_slice(observation.correlation);
    match observation.event {
        RecordDeliveryObservationEvent::FramedQueued { queue_sequence } => {
            output[event_start..event_start + 8].copy_from_slice(&queue_sequence.to_le_bytes());
        }
        RecordDeliveryObservationEvent::PartiallySent { sent_bytes } => {
            output[event_start..event_start + 4].copy_from_slice(&sent_bytes.to_le_bytes());
        }
        RecordDeliveryObservationEvent::RemoteAccepted { receipt } => {
            output[event_start] = receipt.len() as u8;
            output[event_start + 1..length].copy_from_slice(receipt);
        }
        RecordDeliveryObservationEvent::TransportUnavailable { code }
        | RecordDeliveryObservationEvent::Disconnected { code }
        | RecordDeliveryObservationEvent::TimedOut { code }
        | RecordDeliveryObservationEvent::Refused { code }
        | RecordDeliveryObservationEvent::Failed { code } => {
            output[event_start..event_start + 2].copy_from_slice(&code.to_le_bytes());
        }
        RecordDeliveryObservationEvent::LocallyAccepted => {}
    }
    Ok(length)
}

pub fn decode_record_delivery_observation(
    input: &[u8],
) -> Result<RecordDeliveryObservationRef<'_>, RecordDeliveryRefusal> {
    if input.len() < 7 || input[0] != RECORD_DELIVERY_WIRE_VERSION {
        return Err(RecordDeliveryRefusal::MalformedWire);
    }
    let correlation_len = usize::from(input[1]);
    let event_start = 7_usize
        .checked_add(correlation_len)
        .ok_or(RecordDeliveryRefusal::CorrelationTooLong)?;
    let correlation = input
        .get(7..event_start)
        .ok_or(RecordDeliveryRefusal::CorrelationTooLong)?;
    let frame_bytes = u32::from_le_bytes(
        input[2..6]
            .try_into()
            .map_err(|_| RecordDeliveryRefusal::MalformedWire)?,
    );
    validate_identity(correlation, frame_bytes)?;
    let rest = input
        .get(event_start..)
        .ok_or(RecordDeliveryRefusal::MalformedWire)?;
    let event = decode_event(input[6], rest)?;
    Ok(RecordDeliveryObservationRef {
        correlation,
        frame_bytes,
        event,
    })
}

fn validate_identity(correlation: &[u8], frame_bytes: u32) -> Result<(), RecordDeliveryRefusal> {
    if correlation.is_empty() {
        return Err(RecordDeliveryRefusal::EmptyCorrelation);
    }
    if correlation.len() > MAXIMUM_RECORD_CORRELATION_BYTES {
        return Err(RecordDeliveryRefusal::CorrelationTooLong);
    }
    if frame_bytes == 0 {
        return Err(RecordDeliveryRefusal::EmptyFrame);
    }
    if usize::try_from(frame_bytes).map_or(true, |bytes| bytes > MAXIMUM_TYPED_RECORD_FRAME_BYTES) {
        return Err(RecordDeliveryRefusal::FrameTooLarge);
    }
    Ok(())
}

fn event_tag(event: RecordDeliveryObservationEvent<'_>) -> u8 {
    match event {
        RecordDeliveryObservationEvent::LocallyAccepted => 0,
        RecordDeliveryObservationEvent::FramedQueued { .. } => 1,
        RecordDeliveryObservationEvent::PartiallySent { .. } => 2,
        RecordDeliveryObservationEvent::RemoteAccepted { .. } => 3,
        RecordDeliveryObservationEvent::TransportUnavailable { .. } => 4,
        RecordDeliveryObservationEvent::Disconnected { .. } => 5,
        RecordDeliveryObservationEvent::TimedOut { .. } => 6,
        RecordDeliveryObservationEvent::Refused { .. } => 7,
        RecordDeliveryObservationEvent::Failed { .. } => 8,
    }
}

fn decode_event(
    tag: u8,
    bytes: &[u8],
) -> Result<RecordDeliveryObservationEvent<'_>, RecordDeliveryRefusal> {
    Ok(match tag {
        0 if bytes.is_empty() => RecordDeliveryObservationEvent::LocallyAccepted,
        1 if bytes.len() == 8 => RecordDeliveryObservationEvent::FramedQueued {
            queue_sequence: u64::from_le_bytes(bytes.try_into().unwrap()),
        },
        2 if bytes.len() == 4 => RecordDeliveryObservationEvent::PartiallySent {
            sent_bytes: u32::from_le_bytes(bytes.try_into().unwrap()),
        },
        3 if bytes.len() > 1 && usize::from(bytes[0]) + 1 == bytes.len() => {
            RecordDeliveryObservationEvent::RemoteAccepted {
                receipt: &bytes[1..],
            }
        }
        4..=8 if bytes.len() == 2 => {
            let code = u16::from_le_bytes(bytes.try_into().unwrap());
            match tag {
                4 => RecordDeliveryObservationEvent::TransportUnavailable { code },
                5 => RecordDeliveryObservationEvent::Disconnected { code },
                6 => RecordDeliveryObservationEvent::TimedOut { code },
                7 => RecordDeliveryObservationEvent::Refused { code },
                _ => RecordDeliveryObservationEvent::Failed { code },
            }
        }
        _ => return Err(RecordDeliveryRefusal::MalformedWire),
    })
}
