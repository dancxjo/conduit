//! One correlated existing INFORMATION or ERROR response. Local generations
//! prevent an abandoned session handle from accepting later traffic; these are
//! not remote identities, authentication or a new on-wire session field.
use crate::{Frame, FrameDecoder, Refusal, HEADER};
const INFORMATION: u8 = 131;
const ERROR: u8 = 130;
#[derive(Default)]
pub struct InformationSession {
    generation: u64,
    next_id: u32,
    pending: Option<u32>,
    retired: bool,
}
pub struct InformationRequest {
    generation: u64,
    id: u32,
    command: u8,
    maximum: usize,
}
impl InformationRequest {
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn command(&self) -> u8 {
        self.command
    }
}
impl InformationSession {
    /// Only the existing read-only STATUS / SENSOR_STATUS / DIAGNOSTICS commands.
    /// Their role/build/configuration JSON is validated by the owning adapter.
    pub fn begin(&mut self, command: u8) -> Result<InformationRequest, Refusal> {
        if self.retired {
            return Err(Refusal::Stale);
        }
        let maximum = match command {
            3 => 512,
            6 | 7 => 1536,
            _ => return Err(Refusal::Kind),
        };
        if self.pending.is_some() {
            return Err(Refusal::Pipeline);
        }
        let id = self.next_id.checked_add(1).ok_or(Refusal::Stale)?;
        self.next_id = id;
        self.pending = Some(id);
        Ok(InformationRequest {
            generation: self.generation,
            id,
            command,
            maximum,
        })
    }
    /// The native owner must independently quiesce/drain the actual CDC stream.
    /// Never reset request numbers when reopening: old response IDs remain stale.
    pub fn advance_session(&mut self) -> Result<(), Refusal> {
        self.pending = None;
        let Some(generation) = self.generation.checked_add(1) else {
            self.retired = true;
            return Err(Refusal::Stale);
        };
        self.generation = generation;
        Ok(())
    }
    fn matches(&self, request: &InformationRequest) -> bool {
        self.generation == request.generation && self.pending == Some(request.id)
    }
}
pub struct InformationDecoder<const N: usize> {
    request: InformationRequest,
    frame: FrameDecoder<N>,
    poisoned: bool,
    finished: bool,
}
impl<const N: usize> InformationDecoder<N> {
    pub fn new(request: InformationRequest) -> Result<Self, Refusal> {
        if request.maximum > N.saturating_sub(HEADER) {
            return Err(Refusal::Capacity);
        }
        Ok(Self {
            request,
            frame: FrameDecoder::new(),
            poisoned: false,
            finished: false,
        })
    }
    pub fn accept(&mut self, session: &InformationSession, bytes: &[u8]) -> Result<(), Refusal> {
        if self.poisoned || self.finished || !session.matches(&self.request) {
            return Err(Refusal::Stale);
        }
        let id = self.request.id;
        let maximum = self.request.maximum;
        let result = self.frame.accept(bytes, |header| {
            if header.id != id {
                return Err(Refusal::Stale);
            }
            match header.kind {
                INFORMATION if (1..=maximum).contains(&header.length) => Ok(()),
                ERROR if (1..=128).contains(&header.length) => Ok(()),
                INFORMATION | ERROR => Err(Refusal::Capacity),
                _ => Err(Refusal::Kind),
            }
        });
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub fn complete(&self) -> bool {
        !self.poisoned && !self.finished && self.frame.complete()
    }
    /// Consume this pending response exactly once. ERROR payloads are returned as
    /// ERROR frames for explicit caller refusal, never interpreted as information.
    pub fn finish<'a>(
        &'a mut self,
        session: &mut InformationSession,
    ) -> Result<Frame<'a>, Refusal> {
        if self.poisoned || self.finished || !session.matches(&self.request) {
            return Err(Refusal::Stale);
        }
        if !self.frame.complete() {
            return Err(Refusal::Incomplete);
        }
        session.pending = None;
        self.finished = true;
        self.frame.frame()
    }
}
