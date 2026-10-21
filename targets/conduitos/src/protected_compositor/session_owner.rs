//! One Root-owned service with exclusive presenter-local lifecycle namespaces.
//! Session IDs are NOT Host/Boot capabilities, scanout permission or Show proof.
use conduitos_compositor_core::wire::{FrameSpan, PixelBytes, Request, Response};
use conduitos_compositor_core::{Error, Handle, MAX_SURFACES};

/// Private seam for bounded effect tests; the real transport retains its own
/// correlation, response-kind, span/work and hardware confinement checks.
pub(crate) trait Transport {
    type Error;
    fn instance(&self) -> u64;
    fn request(&mut self, request: Request) -> Result<Response, Self::Error>;
    fn upload(&mut self, offset: u32, pixels: &[u32]) -> Result<Response, Self::Error>;
    fn visit_frame<R>(
        &mut self,
        sequence: u64,
        maximum_pixels: u32,
        consume: impl FnOnce(FrameSpan, PixelBytes<'_>) -> R,
    ) -> Result<Result<R, Error>, Self::Error>;
    fn quarantine(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SessionId {
    instance: u64,
    namespace: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Activation {
    pub session: SessionId,
    pub retired: Option<SessionId>,
    pub removed: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnerError<E> {
    Terminated,
    InactiveSession,
    ForeignHandle,
    Capacity,
    Exhausted,
    Protocol,
    Transport(E),
}

pub(crate) struct SessionOwner<T: Transport> {
    transport: T,
    instance: u64,
    namespace: u64,
    active: Option<SessionId>,
    handles: [Option<Handle>; MAX_SURFACES],
    generations: [u64; MAX_SURFACES],
    terminated: bool,
}
impl<T: Transport> SessionOwner<T> {
    pub fn new(mut transport: T) -> Result<Self, OwnerError<T::Error>> {
        let instance = transport.instance();
        if instance == 0 {
            transport.quarantine();
            return Err(OwnerError::Protocol);
        }
        Ok(Self {
            transport,
            instance,
            namespace: 0,
            active: None,
            handles: [None; MAX_SURFACES],
            generations: [0; MAX_SURFACES],
            terminated: false,
        })
    }
    pub fn active_session(&self) -> Option<SessionId> {
        self.active
    }
    pub fn service_instance(&self) -> u64 {
        self.instance
    }
    /// Retire <=8 old handles, clear cursor/hover and invalidate the full frame
    /// before publishing the new namespace (at most ten transport commands).
    /// The retired namespace is fenced before the first fallible Remove. Every
    /// activation allocates a fresh ID; an old ID can never be reactivated.
    /// Finish/abort an upload before handoff: a Busy retirement refusal is
    /// terminal, not an implicit abort, retry or service replacement.
    pub fn activate(&mut self) -> Result<Activation, OwnerError<T::Error>> {
        if self.terminated {
            return Err(OwnerError::Terminated);
        }
        let namespace = self.namespace.checked_add(1).ok_or(OwnerError::Exhausted)?;
        let retired = self.active.take();
        let mut removed = 0;
        for i in 0..MAX_SURFACES {
            if let Some(handle) = self.handles[i] {
                match self.transport.request(Request::Remove(handle)) {
                    Ok(Response::Unit) => {
                        self.handles[i] = None;
                        removed += 1;
                    }
                    Ok(_) => {
                        self.quarantine();
                        return Err(OwnerError::Protocol);
                    }
                    Err(error) => {
                        self.quarantine();
                        return Err(OwnerError::Transport(error));
                    }
                }
            }
        }
        // A logical presenter starts without the prior owner's cursor/hover,
        // and its first frame must repaint stale scanout even with no surfaces.
        // Both reset commands are bounded and happen before publishing the ID.
        for request in [
            Request::Cursor {
                position: None,
                hover: false,
            },
            Request::FailFrame,
        ] {
            match self.transport.request(request) {
                Ok(Response::Unit) => {}
                Ok(_) => {
                    self.quarantine();
                    return Err(OwnerError::Protocol);
                }
                Err(error) => {
                    self.quarantine();
                    return Err(OwnerError::Transport(error));
                }
            }
        }
        self.namespace = namespace;
        let session = SessionId {
            instance: self.instance,
            namespace,
        };
        self.active = Some(session);
        Ok(Activation {
            session,
            retired,
            removed,
        })
    }
    pub fn quarantine(&mut self) {
        if !self.terminated {
            self.transport.quarantine();
        }
        self.terminated = true;
        self.active = None;
        self.handles.fill(None);
    }
    pub fn is_active(&self, session: SessionId) -> bool {
        !self.terminated && session.instance == self.instance && self.active == Some(session)
    }
    fn session(&self, session: SessionId) -> Result<(), OwnerError<T::Error>> {
        if self.terminated {
            return Err(OwnerError::Terminated);
        }
        if !self.is_active(session) {
            return Err(OwnerError::InactiveSession);
        }
        Ok(())
    }
    fn handle(&self, handle: Handle) -> Result<(), OwnerError<T::Error>> {
        if self
            .handles
            .get(usize::from(handle.slot))
            .copied()
            .flatten()
            != Some(handle)
        {
            return Err(OwnerError::ForeignHandle);
        }
        Ok(())
    }
    fn command(&self, session: SessionId, request: Request) -> Result<(), OwnerError<T::Error>> {
        self.session(session)?;
        match request {
            Request::Initialize { .. } => return Err(OwnerError::Protocol),
            Request::Admit { .. } if self.handles.iter().all(Option::is_some) => {
                return Err(OwnerError::Capacity);
            }
            Request::Remove(h) | Request::Focus(h) => self.handle(h)?,
            Request::Place { surface, .. }
            | Request::Resize { surface, .. }
            | Request::Visible { surface, .. }
            | Request::BeginUpload { surface, .. } => self.handle(surface)?,
            _ => {}
        }
        Ok(())
    }
    fn response(
        &mut self,
        request: Request,
        response: Response,
    ) -> Result<Response, OwnerError<T::Error>> {
        match (request, response) {
            (_, Response::Refused(_)) => {}
            (Request::Admit { .. }, Response::Surface(handle)) => {
                let i = usize::from(handle.slot);
                if i >= MAX_SURFACES
                    || self.handles[i].is_some()
                    || handle.generation <= self.generations[i]
                {
                    self.quarantine();
                    return Err(OwnerError::Protocol);
                }
                self.handles[i] = Some(handle);
                self.generations[i] = handle.generation;
            }
            (Request::Remove(handle), Response::Unit) => {
                self.handles[usize::from(handle.slot)] = None
            }
            (Request::Pointer { .. }, Response::Pointer(Some(route))) => {
                if self.handle(route.surface).is_err() {
                    self.quarantine();
                    return Err(OwnerError::Protocol);
                }
            }
            (Request::Focused, Response::Focused(Some(handle))) => {
                if self.handle(handle).is_err() {
                    self.quarantine();
                    return Err(OwnerError::Protocol);
                }
            }
            (Request::Keyboard, Response::Keyboard(Some((handle, _)))) => {
                if self.handle(handle).is_err() {
                    self.quarantine();
                    return Err(OwnerError::Protocol);
                }
            }
            (Request::FrameStep { .. }, Response::Frame(span))
                if !mapped_coverage(&self.handles, span) =>
            {
                self.quarantine();
                return Err(OwnerError::Protocol);
            }
            _ => {} // Response kind/range validation belongs to Transport.
        }
        Ok(response)
    }
    pub fn request(
        &mut self,
        session: SessionId,
        request: Request,
    ) -> Result<Response, OwnerError<T::Error>> {
        self.command(session, request)?;
        match self.transport.request(request) {
            Ok(response) => self.response(request, response),
            Err(error) => {
                self.quarantine();
                Err(OwnerError::Transport(error))
            }
        }
    }
    pub fn upload_chunk(
        &mut self,
        session: SessionId,
        offset: u32,
        pixels: &[u32],
    ) -> Result<Response, OwnerError<T::Error>> {
        self.session(session)?;
        match self.transport.upload(offset, pixels) {
            Ok(response) => Ok(response),
            Err(error) => {
                self.quarantine();
                Err(OwnerError::Transport(error))
            }
        }
    }
    /// Transfer borrows cannot escape this callback or overlap another entry.
    /// A scanout callback must still validate its Base and report a copy failure
    /// through FailFrame/quarantine; this owner cannot mint scanout evidence.
    pub fn with_frame_step<R>(
        &mut self,
        session: SessionId,
        sequence: u64,
        maximum_pixels: u32,
        consume: impl FnOnce(FrameSpan, PixelBytes<'_>) -> R,
    ) -> Result<Result<R, Error>, OwnerError<T::Error>> {
        self.session(session)?;
        let handles = self.handles;
        let result = self
            .transport
            .visit_frame(sequence, maximum_pixels, |span, pixels| {
                if !mapped_coverage(&handles, span) {
                    return Err(());
                }
                Ok(consume(span, pixels))
            });
        match result {
            Ok(Ok(Ok(value))) => Ok(Ok(value)),
            Ok(Ok(Err(()))) => {
                self.quarantine();
                Err(OwnerError::Protocol)
            }
            Ok(Err(error)) => Ok(Err(error)),
            Err(error) => {
                self.quarantine();
                Err(OwnerError::Transport(error))
            }
        }
    }
}

fn mapped_coverage(handles: &[Option<Handle>; MAX_SURFACES], span: FrameSpan) -> bool {
    span.coverage.iter().enumerate().all(|(i, &count)| {
        (count == 0 && span.selected_mask & (1 << i) == 0) || handles[i].is_some()
    })
}

#[cfg(all(target_arch = "x86_64", target_os = "none"))]
impl Transport for super::CompositorService {
    type Error = super::TransportError;
    fn instance(&self) -> u64 {
        super::CompositorService::instance(self)
    }
    fn request(&mut self, request: Request) -> Result<Response, Self::Error> {
        super::CompositorService::request(self, request)
    }
    fn upload(&mut self, offset: u32, pixels: &[u32]) -> Result<Response, Self::Error> {
        self.upload_chunk(offset, pixels)
    }
    fn visit_frame<R>(
        &mut self,
        sequence: u64,
        maximum_pixels: u32,
        consume: impl FnOnce(FrameSpan, PixelBytes<'_>) -> R,
    ) -> Result<Result<R, Error>, Self::Error> {
        match self.frame_step(sequence, maximum_pixels)? {
            Ok(chunk) => Ok(Ok(consume(chunk.span, chunk.pixels))),
            Err(error) => Ok(Err(error)),
        }
    }
    fn quarantine(&mut self) {
        super::CompositorService::quarantine(self)
    }
}
/// Read-only execution accounting; no raw transport or mutable escape.
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
impl SessionOwner<super::CompositorService> {
    pub(crate) fn service_cost(&self) -> crate::protected_region::DomainCost {
        self.transport.cost()
    }
}

#[cfg(test)]
#[path = "session_owner/tests.rs"]
mod tests;

#[cfg(test)]
mod focused_query_tests {
    use super::*;
    struct FocusTransport {
        returned: Option<Handle>,
        quarantined: bool,
    }
    impl Transport for FocusTransport {
        type Error = ();
        fn instance(&self) -> u64 {
            1
        }
        fn request(&mut self, request: Request) -> Result<Response, ()> {
            Ok(match request {
                Request::Focused => Response::Focused(self.returned),
                Request::Admit { .. } => Response::Surface(Handle {
                    slot: 0,
                    generation: 1,
                }),
                _ => Response::Unit,
            })
        }
        fn upload(&mut self, _: u32, _: &[u32]) -> Result<Response, ()> {
            unreachable!()
        }
        fn visit_frame<R>(
            &mut self,
            _: u64,
            _: u32,
            _: impl FnOnce(FrameSpan, PixelBytes<'_>) -> R,
        ) -> Result<Result<R, Error>, ()> {
            unreachable!()
        }
        fn quarantine(&mut self) {
            self.quarantined = true;
        }
    }
    #[test]
    fn logical_focus_query_refuses_foreign_handles_even_without_keyboard_route() {
        let mut owner = SessionOwner::new(FocusTransport {
            returned: None,
            quarantined: false,
        })
        .unwrap();
        let id = owner.activate().unwrap().session;
        assert_eq!(
            owner.request(id, Request::Focused),
            Ok(Response::Focused(None))
        );
        let handle = Handle {
            slot: 0,
            generation: 1,
        };
        owner
            .request(
                id,
                Request::Admit {
                    rect: conduitos_compositor_core::Rect {
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                    },
                    z: 0,
                },
            )
            .unwrap();
        owner.transport.returned = Some(handle);
        assert_eq!(
            owner.request(id, Request::Focused),
            Ok(Response::Focused(Some(handle)))
        );
        owner.transport.returned = Some(Handle {
            slot: 7,
            generation: 1,
        });
        assert_eq!(
            owner.request(id, Request::Focused),
            Err(OwnerError::Protocol)
        );
        assert!(owner.transport.quarantined);
    }
}
