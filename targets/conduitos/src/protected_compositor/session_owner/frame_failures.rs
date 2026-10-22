//! Host-only glue evidence: actual span budget + session terminal lifecycle.
//! Fake replies do not establish guest execution, Root copies or semantic Show.
use super::*;
use crate::compositor_frame_grant_tests::FrameGrant;
use conduitos_compositor_core::{FramePlan, FrameRect};

struct CheckedFake {
    grant: FrameGrant,
    sequence: u64,
    reply: FrameSpan,
    effects: u32,
    quarantines: u32,
}
impl CheckedFake {
    fn new() -> Self {
        Self {
            grant: FrameGrant::new(2, 1),
            sequence: 0,
            effects: 0,
            quarantines: 0,
            reply: FrameSpan {
                sequence: 0,
                x: 0,
                y: 0,
                pixels: 2,
                window_offset: 0,
                complete: true,
                selected_mask: 0,
                coverage: [0; 8],
            },
        }
    }
}
impl Transport for CheckedFake {
    type Error = ();
    fn instance(&self) -> u64 {
        1
    }
    fn request(&mut self, request: Request) -> Result<Response, ()> {
        self.effects += 1;
        match request {
            Request::BeginFrame => {
                self.sequence += 1;
                self.reply.sequence = self.sequence;
                let mut rects = [FrameRect::default(); 16];
                rects[0] = FrameRect {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 1,
                };
                let plan = FramePlan {
                    sequence: self.sequence,
                    count: 1,
                    rects,
                    cursor_visible: false,
                    focus_visible: false,
                    conservative_fallback: false,
                };
                self.grant.begin(plan).map_err(|_| ())?;
                Ok(Response::FrameStarted(plan))
            }
            Request::FrameStep { sequence, pixels } => {
                self.grant
                    .admit(self.reply, sequence, pixels)
                    .map_err(|_| ())?;
                Ok(Response::Frame(self.reply))
            }
            Request::AcknowledgeScanout { sequence } => {
                self.grant.ack(sequence).map_err(|_| ())?;
                Ok(Response::Unit)
            }
            Request::FailFrame => {
                self.grant.invalidate();
                Ok(Response::Unit)
            }
            _ => Ok(Response::Unit),
        }
    }
    fn upload(&mut self, _: u32, _: &[u32]) -> Result<Response, ()> {
        self.effects += 1;
        Ok(Response::Unit)
    }
    fn visit_frame<R>(
        &mut self,
        sequence: u64,
        pixels: u32,
        consume: impl FnOnce(FrameSpan, PixelBytes<'_>) -> R,
    ) -> Result<Result<R, Error>, ()> {
        let Response::Frame(span) = self.request(Request::FrameStep { sequence, pixels })? else {
            return Err(());
        };
        let bytes = [0; 8];
        Ok(Ok(consume(
            span,
            conduitos_compositor_core::wire::decode_pixels(&bytes, 0, 2).unwrap(),
        )))
    }
    fn quarantine(&mut self) {
        self.quarantines += 1;
        self.grant.invalidate();
    }
}
fn acknowledged() -> (SessionOwner<CheckedFake>, SessionId) {
    let mut owner = SessionOwner::new(CheckedFake::new()).unwrap();
    let id = owner.activate().unwrap().session;
    owner.request(id, Request::BeginFrame).unwrap();
    owner.with_frame_step(id, 1, 2, |_, _| ()).unwrap().unwrap();
    owner
        .request(id, Request::AcknowledgeScanout { sequence: 1 })
        .unwrap();
    (owner, id)
}
fn terminal(owner: &mut SessionOwner<CheckedFake>, id: SessionId) {
    let effects = owner.transport.effects;
    assert!(!owner.is_active(id));
    assert_eq!(owner.transport.quarantines, 1);
    assert_eq!(
        owner.request(id, Request::AcknowledgeScanout { sequence: 1 }),
        Err(OwnerError::Terminated)
    );
    assert_eq!(
        owner.request(id, Request::AcknowledgeScanout { sequence: 2 }),
        Err(OwnerError::Terminated)
    );
    assert_eq!(
        owner.request(id, Request::Keyboard),
        Err(OwnerError::Terminated)
    );
    assert_eq!(owner.upload_chunk(id, 0, &[0]), Err(OwnerError::Terminated));
    assert_eq!(owner.activate(), Err(OwnerError::Terminated));
    owner.quarantine();
    assert_eq!(owner.transport.effects, effects);
    assert_eq!(owner.transport.quarantines, 1);
}
#[test]
fn malformed_completion_or_exhausted_output_fences_prior_ack_before_callback() {
    for variant in 0..7 {
        let (mut owner, id) = acknowledged();
        owner.request(id, Request::BeginFrame).unwrap();
        let mut maximum = 2;
        match variant {
            0 => owner.transport.reply.window_offset = 65536,
            1 => owner.transport.reply.complete = false,
            2 => owner.transport.reply.sequence = 1, // duplicate prior-frame reply
            3 => maximum = 0,
            4 => maximum = 16385,
            5 => owner.transport.reply.coverage[0] = 3,
            6 => owner.transport.reply.pixels = 3,
            _ => unreachable!(),
        }
        let mut consumed = false;
        assert_eq!(
            owner.with_frame_step(id, 2, maximum, |_, _| consumed = true),
            Err(OwnerError::Transport(()))
        );
        assert!(!consumed, "variant {variant}");
        terminal(&mut owner, id);
    }
}
#[test]
fn duplicate_completed_reply_cannot_reopen_frame_or_replay_prior_ack() {
    let (mut owner, id) = acknowledged();
    owner.request(id, Request::BeginFrame).unwrap();
    owner.with_frame_step(id, 2, 2, |_, _| ()).unwrap().unwrap();
    let mut consumed = false;
    assert_eq!(
        owner.with_frame_step(id, 2, 2, |_, _| consumed = true),
        Err(OwnerError::Transport(()))
    );
    assert!(!consumed);
    terminal(&mut owner, id);
}
