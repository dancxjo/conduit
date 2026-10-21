use super::*;
use conduitos_compositor_core::wire::decode_pixels;
use conduitos_compositor_core::{Rect, Route};
use std::{vec, vec::Vec};

struct Fake {
    instance: u64,
    commands: Vec<Request>,
    quarantines: u32,
    fail_at: Option<usize>,
    refuse_at: Option<usize>,
    slots: [Option<Handle>; 8],
    generations: [u64; 8],
    forged_route: Option<Handle>,
    forged_coverage: bool,
}
impl Fake {
    fn new(instance: u64) -> Self {
        Self {
            instance,
            commands: vec![],
            quarantines: 0,
            fail_at: None,
            refuse_at: None,
            slots: [None; 8],
            generations: [0; 8],
            forged_route: None,
            forged_coverage: false,
        }
    }
    fn span(&self) -> FrameSpan {
        let mut coverage = [0; 8];
        if self.forged_coverage {
            coverage[7] = 1;
        }
        FrameSpan {
            sequence: 1,
            x: 0,
            y: 0,
            window_offset: 0,
            pixels: 1,
            complete: true,
            coverage,
        }
    }
}
impl Transport for Fake {
    type Error = u8;
    fn instance(&self) -> u64 {
        self.instance
    }
    fn request(&mut self, r: Request) -> Result<Response, u8> {
        self.commands.push(r);
        let step = self.commands.len();
        if self.fail_at == Some(step) {
            return Err(99);
        }
        if self.refuse_at == Some(step) {
            return Ok(Response::Refused(Error::Busy));
        }
        Ok(match r {
            Request::Admit { .. } => {
                let i = self.slots.iter().position(Option::is_none).unwrap();
                self.generations[i] += 1;
                let h = Handle {
                    slot: i as u8,
                    generation: self.generations[i],
                };
                self.slots[i] = Some(h);
                Response::Surface(h)
            }
            Request::Remove(h) => {
                assert_eq!(self.slots[usize::from(h.slot)], Some(h));
                self.slots[usize::from(h.slot)] = None;
                Response::Unit
            }
            Request::Pointer { .. } => Response::Pointer(self.forged_route.map(|surface| Route {
                surface,
                revision: 1,
                x: 0,
                y: 0,
            })),
            Request::Keyboard => Response::Keyboard(self.forged_route.map(|h| (h, 1))),
            Request::FrameStep { .. } => Response::Frame(self.span()),
            _ => Response::Unit,
        })
    }
    fn upload(&mut self, offset: u32, pixels: &[u32]) -> Result<Response, u8> {
        self.request(Request::UploadWindow {
            offset,
            window_offset: 0,
            pixels: pixels.len() as u32,
        })
    }
    fn visit_frame<R>(
        &mut self,
        sequence: u64,
        maximum_pixels: u32,
        consume: impl FnOnce(FrameSpan, PixelBytes<'_>) -> R,
    ) -> Result<Result<R, Error>, u8> {
        self.request(Request::FrameStep {
            sequence,
            pixels: maximum_pixels,
        })?;
        let bytes = [0; 4];
        Ok(Ok(consume(
            self.span(),
            decode_pixels(&bytes, 0, 1).unwrap(),
        )))
    }
    fn quarantine(&mut self) {
        self.quarantines += 1;
    }
}
fn owner() -> SessionOwner<Fake> {
    SessionOwner::new(Fake::new(7)).unwrap()
}
fn admit(owner: &mut SessionOwner<Fake>, session: SessionId) -> Handle {
    match owner
        .request(
            session,
            Request::Admit {
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                z: 0,
            },
        )
        .unwrap()
    {
        Response::Surface(h) => h,
        _ => panic!("missing handle"),
    }
}
#[test]
fn one_service_slot_is_reused_with_fresh_namespace_and_stale_fences() {
    let mut owner = owner();
    assert_eq!(owner.active_session(), None);
    assert_eq!(owner.service_instance(), 7);
    let a = owner.activate().unwrap();
    assert_eq!(a.retired, None);
    assert_eq!(a.removed, 0);
    let first = admit(&mut owner, a.session);
    let second = admit(&mut owner, a.session);
    let b = owner.activate().unwrap();
    assert_eq!(b.retired, Some(a.session));
    assert_eq!(b.removed, 2);
    assert_ne!(a.session, b.session);
    assert_eq!(
        &owner.transport.commands[2..],
        &[Request::Remove(first), Request::Remove(second)]
    );
    let calls = owner.transport.commands.len();
    assert_eq!(
        owner.request(a.session, Request::Keyboard),
        Err(OwnerError::InactiveSession)
    );
    assert_eq!(
        owner.request(b.session, Request::Focus(first)),
        Err(OwnerError::ForeignHandle)
    );
    assert_eq!(owner.transport.commands.len(), calls);
    let replacement = admit(&mut owner, b.session);
    assert_eq!(replacement.slot, first.slot);
    assert!(replacement.generation > first.generation);
    let c = owner.activate().unwrap();
    assert_ne!(c.session, a.session);
    assert_ne!(c.session, b.session);
    assert!(!owner.is_active(a.session));
    assert!(!owner.is_active(b.session));
}
#[test]
fn foreign_instance_and_inactive_upload_or_frame_refuse_before_effects() {
    let mut owner = owner();
    let a = owner.activate().unwrap().session;
    let mut foreign = SessionOwner::new(Fake::new(8)).unwrap();
    let b = foreign.activate().unwrap().session;
    assert_eq!(
        owner.request(b, Request::Keyboard),
        Err(OwnerError::InactiveSession)
    );
    assert_eq!(
        owner.upload_chunk(b, 0, &[1]),
        Err(OwnerError::InactiveSession)
    );
    assert_eq!(
        owner.with_frame_step(b, 1, 1, |_, _| ()),
        Err(OwnerError::InactiveSession)
    );
    assert_eq!(
        owner.request(
            a,
            Request::Initialize {
                width: 1,
                height: 1
            }
        ),
        Err(OwnerError::Protocol)
    );
    assert!(owner.transport.commands.is_empty());
}
#[test]
fn retirement_failure_quarantines_and_never_retries_or_reinstalls() {
    let mut owner = owner();
    let old = owner.activate().unwrap().session;
    admit(&mut owner, old);
    admit(&mut owner, old);
    admit(&mut owner, old);
    owner.transport.fail_at = Some(5); // One remove succeeds; the next transport call fails.
    assert_eq!(owner.activate(), Err(OwnerError::Transport(99)));
    assert_eq!(owner.transport.commands.len(), 5);
    assert_eq!(owner.transport.quarantines, 1);
    assert_eq!(owner.active_session(), None);
    assert!(owner.handles.iter().all(Option::is_none));
    assert_eq!(owner.activate(), Err(OwnerError::Terminated));
    assert_eq!(
        owner.request(old, Request::Keyboard),
        Err(OwnerError::Terminated)
    );
    owner.quarantine();
    assert_eq!(owner.transport.commands.len(), 5);
    assert_eq!(owner.transport.quarantines, 1);
}
#[test]
fn retirement_refusal_is_not_ignored_and_request_failure_is_terminal() {
    let mut owner = owner();
    let id = owner.activate().unwrap().session;
    admit(&mut owner, id);
    owner.transport.refuse_at = Some(2);
    assert_eq!(owner.activate(), Err(OwnerError::Protocol));
    assert_eq!(owner.transport.quarantines, 1);
    let mut owner = SessionOwner::new(Fake::new(8)).unwrap();
    let id = owner.activate().unwrap().session;
    owner.transport.fail_at = Some(1);
    assert_eq!(
        owner.request(id, Request::Keyboard),
        Err(OwnerError::Transport(99))
    );
    assert_eq!(
        owner.request(id, Request::Keyboard),
        Err(OwnerError::Terminated)
    );
    assert_eq!(owner.transport.commands.len(), 1);
}
#[test]
fn exhaustion_and_capacity_refuse_before_retirement_or_guest_effects() {
    let mut owner = owner();
    owner.namespace = u64::MAX - 1;
    let id = owner.activate().unwrap().session;
    for _ in 0..8 {
        admit(&mut owner, id);
    }
    let calls = owner.transport.commands.len();
    assert_eq!(owner.activate(), Err(OwnerError::Exhausted));
    assert!(owner.is_active(id));
    assert_eq!(
        owner.request(
            id,
            Request::Admit {
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1
                },
                z: 0
            }
        ),
        Err(OwnerError::Capacity)
    );
    assert_eq!(owner.transport.commands.len(), calls);
    assert_eq!(owner.transport.quarantines, 0);
}
#[test]
fn advisory_routes_and_coverage_must_belong_to_active_handle_map() {
    let mut owner = owner();
    let id = owner.activate().unwrap().session;
    let h = admit(&mut owner, id);
    owner.transport.forged_route = Some(h);
    assert!(matches!(
        owner.request(id, Request::Pointer { x: 0, y: 0 }).unwrap(),
        Response::Pointer(Some(_))
    ));
    owner.transport.forged_route = Some(Handle {
        slot: 7,
        generation: 1,
    });
    assert_eq!(
        owner.request(id, Request::Keyboard),
        Err(OwnerError::Protocol)
    );
    assert_eq!(owner.active_session(), None);
    let mut owner = SessionOwner::new(Fake::new(8)).unwrap();
    let id = owner.activate().unwrap().session;
    owner.transport.forged_coverage = true;
    let mut called = false;
    assert_eq!(
        owner.with_frame_step(id, 1, 1, |_, _| {
            called = true;
        }),
        Err(OwnerError::Protocol)
    );
    assert!(!called);
    assert_eq!(owner.transport.quarantines, 1);
    let mut owner = SessionOwner::new(Fake::new(9)).unwrap();
    let id = owner.activate().unwrap().session;
    owner.transport.forged_coverage = true;
    assert_eq!(
        owner.request(
            id,
            Request::FrameStep {
                sequence: 1,
                pixels: 1
            }
        ),
        Err(OwnerError::Protocol)
    );
}
#[test]
fn upload_and_scoped_frame_borrow_use_single_admitted_service_without_escape() {
    let mut owner = owner();
    let id = owner.activate().unwrap().session;
    assert_eq!(owner.upload_chunk(id, 0, &[3]), Ok(Response::Unit));
    assert_eq!(
        owner.with_frame_step(id, 1, 1, |span, pixels| (
            span.pixels,
            pixels.pixels().next()
        )),
        Ok(Ok((1, Some(0))))
    );
    owner.transport.fail_at = Some(3);
    assert_eq!(
        owner.upload_chunk(id, 0, &[3]),
        Err(OwnerError::Transport(99))
    );
    assert_eq!(owner.transport.quarantines, 1);
}
