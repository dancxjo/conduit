//! Root transport for the isolated renderer. This owns no retained raster and
//! grants no display or semantic authority. The admitted presenter must bind
//! numeric surfaces to its checked Face/Mask/Show and validate advisory routes.
pub(crate) mod display_owner;
mod frame_grant;
pub(crate) mod raster_staging;
pub(crate) mod session_owner;
use frame_grant::FrameGrant;

use crate::{
    arch::ServiceDomain,
    protected_region::{DomainBackend, DomainRefusal, DomainReturn},
    service_memory::ServiceImage,
};
use conduitos_compositor_core::wire::{self, Envelope, FrameSpan, PixelBytes, Request, Response};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    Backend(DomainRefusal),
    Protocol,
    Terminated,
    Exhausted,
    Bounds,
}

pub struct CompositorService {
    domain: ServiceDomain,
    next_request: u64,
    frame_grant: FrameGrant,
}

/// A borrowed, validated transfer span. Its lifetime excludes another entry,
/// so the unprivileged renderer cannot change pixels during Root scanout.
pub struct FrameChunk<'a> {
    pub span: FrameSpan,
    pub pixels: PixelBytes<'a>,
}

impl CompositorService {
    pub fn install_embedded(width: u16, height: u16) -> Result<Self, TransportError> {
        #[cfg(conduitos_compositor_image)]
        {
            let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/compositor.elf"));
            let image = ServiceImage::compositor(bytes).map_err(|_| TransportError::Protocol)?;
            Self::install(&image, width, height)
        }
        #[cfg(not(conduitos_compositor_image))]
        {
            let _ = (width, height);
            Err(TransportError::Backend(DomainRefusal::Unsupported))
        }
    }

    pub fn install(
        image: &ServiceImage<'_>,
        width: u16,
        height: u16,
    ) -> Result<Self, TransportError> {
        if width == 0 || height == 0 {
            return Err(TransportError::Bounds);
        }
        let domain = ServiceDomain::install(image).map_err(TransportError::Backend)?;
        let mut service = Self {
            domain,
            next_request: 1,
            frame_grant: FrameGrant::new(width, height),
        };
        if service.request(Request::Initialize { width, height })? != Response::Unit {
            service.quarantine();
            return Err(TransportError::Protocol);
        }
        Ok(service)
    }

    pub(crate) fn cost(&self) -> crate::protected_region::DomainCost {
        self.domain.cost()
    }

    pub fn instance(&self) -> u64 {
        self.domain.instance()
    }

    pub fn quarantine(&mut self) {
        self.frame_grant.invalidate();
        self.domain.quarantine();
    }

    /// One admitted bounded command. The service is hostile: correlation,
    /// response kind and numeric limits are checked before any caller effect.
    /// A Refused response is an explicit command outcome, never a retry grant.
    pub fn request(&mut self, request: Request) -> Result<Response, TransportError> {
        let result = self.exchange(request);
        if result.is_err() {
            self.quarantine();
        }
        result
    }

    fn exchange(&mut self, request: Request) -> Result<Response, TransportError> {
        match request {
            Request::AcknowledgeScanout { sequence } => self
                .frame_grant
                .ack(sequence)
                .map_err(|_| TransportError::Protocol)?,
            Request::FrameStep { .. } | Request::Pointer { .. } | Request::Keyboard => {}
            // Frames are transactions. Other commands cancel a pending Root
            // grant even when the renderer ultimately reports a no-op/refusal.
            _ => self.frame_grant.invalidate(),
        }
        let envelope = Envelope {
            instance: self.instance(),
            request: self.next_request,
        };
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(TransportError::Exhausted)?;
        let exchange = self.domain.exchange().map_err(TransportError::Backend)?;
        exchange.fill(0);
        wire::encode_request(envelope, request, exchange).map_err(|_| TransportError::Protocol)?;
        if self.domain.enter(1).map_err(TransportError::Backend)? != DomainReturn::Yielded {
            return Err(TransportError::Terminated);
        }
        let exchange = self.domain.exchange().map_err(TransportError::Backend)?;
        let length = u32::from_le_bytes(
            exchange
                .get(8..12)
                .ok_or(TransportError::Protocol)?
                .try_into()
                .map_err(|_| TransportError::Protocol)?,
        ) as usize;
        let bytes = exchange.get(..length).ok_or(TransportError::Protocol)?;
        let (returned, response) =
            wire::decode_response(bytes).map_err(|_| TransportError::Protocol)?;
        if returned != envelope || !response_matches(request, response) {
            return Err(TransportError::Protocol);
        }
        match (request, response) {
            (Request::BeginFrame, Response::FrameStarted(plan)) => self
                .frame_grant
                .begin(plan)
                .map_err(|_| TransportError::Protocol)?,
            (Request::BeginFrame, Response::FrameIdle { .. }) => self.frame_grant.invalidate(),
            (Request::FrameStep { sequence, pixels }, Response::Frame(span)) => self
                .frame_grant
                .admit(span, sequence, pixels)
                .map_err(|_| TransportError::Protocol)?,
            _ => {}
        }
        Ok(response)
    }

    pub fn upload_chunk(
        &mut self,
        offset: u32,
        pixels: &[u32],
    ) -> Result<Response, TransportError> {
        let count = u32::try_from(pixels.len()).map_err(|_| TransportError::Protocol)?;
        let window = self
            .domain
            .transfer_window()
            .map_err(TransportError::Backend)?;
        wire::encode_pixels(window, 0, pixels).map_err(|_| TransportError::Protocol)?;
        self.request(Request::UploadWindow {
            offset,
            window_offset: 0,
            pixels: count,
        })
    }

    /// Return pixels only from this call's matching validated Frame response;
    /// callers cannot supply stale window metadata.
    pub fn frame_step(
        &mut self,
        sequence: u64,
        maximum_pixels: u32,
    ) -> Result<Result<FrameChunk<'_>, conduitos_compositor_core::Error>, TransportError> {
        match self.request(Request::FrameStep {
            sequence,
            pixels: maximum_pixels,
        })? {
            Response::Frame(span) => {
                let window = self.domain.output().map_err(TransportError::Backend)?;
                let pixels = wire::decode_pixels(window, span.window_offset, span.pixels)
                    .map_err(|_| TransportError::Protocol)?;
                Ok(Ok(FrameChunk { span, pixels }))
            }
            Response::Refused(error) => Ok(Err(error)),
            _ => Err(TransportError::Protocol),
        }
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
