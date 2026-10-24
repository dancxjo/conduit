//! One production command/envelope owner; no scheduler or CPU grant is added.
use super::frame_grant::FrameGrant;
use conduitos_compositor_core::wire::{self, Envelope, Request, Response};

/// Largest existing command:32-byte header and25-byte BeginUpload payload.
pub(super) const COMMAND_BYTES: usize = wire::HEADER_BYTES + 25;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OwnerError {
    Protocol,
    Busy,
    Terminated,
    Exhausted,
}
struct Pending {
    envelope: Envelope,
    request: Request,
    bytes: [u8; COMMAND_BYTES],
    length: usize,
}
pub(super) struct RequestOwner {
    instance: u64,
    ordinal: u64,
    pending: Option<Pending>,
    terminal: bool,
    frame: FrameGrant,
}
impl RequestOwner {
    pub(super) fn new(instance: u64, width: u16, height: u16) -> Self {
        Self {
            instance,
            ordinal: 1,
            pending: None,
            terminal: instance == 0,
            frame: FrameGrant::new(width, height),
        }
    }
    pub(super) fn instance(&self) -> u64 {
        self.instance
    }
    pub(super) fn begin(&mut self, request: Request) -> Result<(), OwnerError> {
        if self.terminal {
            return Err(OwnerError::Terminated);
        }
        if self.pending.is_some() {
            return Err(OwnerError::Busy);
        }
        if self.ordinal == u64::MAX {
            self.quarantine();
            return Err(OwnerError::Exhausted);
        }
        let envelope = Envelope {
            instance: self.instance,
            request: self.ordinal,
        };
        let mut bytes = [0; COMMAND_BYTES];
        let length = match wire::encode_request(envelope, request, &mut bytes) {
            Ok(length) => length,
            Err(_) => {
                self.quarantine();
                return Err(OwnerError::Protocol);
            }
        };
        let admitted = match request {
            Request::AcknowledgeScanout { sequence } => {
                self.frame.ack(sequence).map_err(|_| OwnerError::Protocol)
            }
            Request::FrameStep { .. }
            | Request::Pointer { .. }
            | Request::Keyboard
            | Request::Focused
            | Request::DamagePending => Ok(()),
            _ => {
                self.frame.invalidate();
                Ok(())
            }
        };
        if let Err(error) = admitted {
            self.quarantine();
            return Err(error);
        }
        self.pending = Some(Pending {
            envelope,
            request,
            bytes,
            length,
        });
        Ok(())
    }
    pub(super) fn command(&self) -> Result<&[u8], OwnerError> {
        if self.terminal {
            return Err(OwnerError::Terminated);
        }
        let pending = self.pending.as_ref().ok_or(OwnerError::Protocol)?;
        Ok(&pending.bytes[..pending.length])
    }
    /// Shared pages have unused trailing capacity; the declared packet alone is
    /// passed to the exact decoder. Standalone packet completion rejects extras.
    pub(super) fn complete_exchange(&mut self, page: &[u8]) -> Result<Response, OwnerError> {
        let result = (|| {
            let length = u32::from_le_bytes(
                page.get(8..12)
                    .ok_or(OwnerError::Protocol)?
                    .try_into()
                    .map_err(|_| OwnerError::Protocol)?,
            ) as usize;
            let packet = page.get(..length).ok_or(OwnerError::Protocol)?;
            self.complete_response(packet)
        })();
        if result.is_err() {
            self.quarantine();
        }
        result
    }
    pub(super) fn complete_response(&mut self, packet: &[u8]) -> Result<Response, OwnerError> {
        let result = self.validate_response(packet);
        if result.is_err() {
            self.quarantine();
        }
        result
    }
    fn validate_response(&mut self, packet: &[u8]) -> Result<Response, OwnerError> {
        if self.terminal {
            return Err(OwnerError::Terminated);
        }
        let pending = self.pending.as_ref().ok_or(OwnerError::Protocol)?;
        let (envelope, response) =
            wire::decode_response(packet).map_err(|_| OwnerError::Protocol)?;
        if envelope != pending.envelope || !response_matches(pending.request, response) {
            return Err(OwnerError::Protocol);
        }
        match (pending.request, response) {
            (Request::BeginFrame, Response::FrameStarted(plan)) => {
                self.frame.begin(plan).map_err(|_| OwnerError::Protocol)?
            }
            (Request::BeginFrame, Response::FrameIdle { .. }) => self.frame.invalidate(),
            (Request::FrameStep { sequence, pixels }, Response::Frame(span)) => self
                .frame
                .admit(span, sequence, pixels)
                .map_err(|_| OwnerError::Protocol)?,
            _ => {}
        }
        self.pending = None;
        self.ordinal += 1; // begin refused MAX before creating this command.
        Ok(response)
    }
    /// Fault, cancellation, and transport quarantine all retire the same owner.
    pub(super) fn quarantine(&mut self) {
        self.pending = None;
        self.terminal = true;
        self.frame.invalidate();
    }
}

fn response_matches(request: Request, response: Response) -> bool {
    if matches!(response, Response::Refused(_)) {
        return true;
    }
    matches!(
        (request, response),
        (Request::Admit { .. }, Response::Surface(_))
            | (Request::PrepareUpload { .. }, Response::Prepared(_))
            | (Request::BeginFrame, Response::FrameStarted(_))
            | (Request::BeginFrame, Response::FrameIdle { .. })
            | (Request::FrameStep { .. }, Response::Frame(_))
            | (Request::Pointer { .. }, Response::Pointer(_))
            | (Request::Keyboard, Response::Keyboard(_))
            | (Request::Focused, Response::Focused(_))
            | (Request::DamagePending, Response::DamagePending(_))
            | (
                Request::Remove(_)
                    | Request::Place { .. }
                    | Request::Resize { .. }
                    | Request::Visible { .. }
                    | Request::Focus(_)
                    | Request::Cursor { .. }
                    | Request::BeginUpload { .. }
                    | Request::UploadWindow { .. }
                    | Request::CommitUpload
                    | Request::AbortUpload
                    | Request::AcknowledgeScanout { .. }
                    | Request::FailFrame
                    | Request::Initialize { .. },
                Response::Unit
            )
    )
}

#[cfg(test)]
#[path = "request_owner/tests.rs"]
mod tests;
