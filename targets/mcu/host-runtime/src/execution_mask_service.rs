//! Original renderer request/frame custody, parameterized by prepared encoding.
use super::*;

pub(super) struct Staged {
    show: f::Show,
    pages: PreparedAlternatingShow,
}
pub(super) struct Pending {
    request: RequestId,
    lease: c::BaseOperationLease,
    show: f::Show,
    pages: PreparedAlternatingShow,
    started: u64,
}
pub(super) struct Retained {
    show: f::Show,
    pages: PreparedAlternatingShow,
    started: u64,
}
/// Monotonic completion identity within one renderer service. Root pairs it
/// with the exact output Play/Host Call; a prior frame cannot acknowledge a
/// later pending frame even when its bytes happen to be identical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTicket(u32);
pub struct OutputFrame<'a> {
    pub ticket: FrameTicket,
    pub canonical: &'a [u8],
}
pub struct RendererService<
    'a,
    C,
    E: PreparedFrameEncoder = Tm1637Encoder,
    const FRAME: usize = { conduit_tm1637_protocol::DISPLAY_FRAME_BYTES },
> {
    pub(super) ready: &'a mut ReadyRenderer<C, E, FRAME>,
    pub(super) reader: Reader<'a>,
    pub(super) context: f::PreparedRendererContext,
    pub(super) active: c::ActivePlayIdentity,
    pub(super) node: NodeId,
    pub(super) operation: c::BoundBaseOperation,
    pub(super) next_request: u32,
    pub(super) pending: Option<Pending>,
    pub(super) staged: Option<Staged>,
    pub(super) face_input_len: usize,
    pub(super) retained: Option<Retained>,
    pub(super) last_clock: u64,
    pub(super) frames: u32,
    pub(super) frame_pending: bool,
    pub(super) last_frame: [u8; FRAME],
    pub(super) last_frame_len: usize,
}
impl<C: RendererClock, E: PreparedFrameEncoder, const FRAME: usize>
    RendererService<'_, C, E, FRAME>
{
    /// Includes simultaneous pending/retained Shows; the shared Face Reader's
    /// publication owner is counted once by the enclosing group, not here.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let mut bytes = self
            .ready
            .retained_backing_bytes()?
            .checked_add(self.context.retained_backing_bytes()?)?
            .checked_add(self.operation.retained_backing_bytes()?)?;
        for text in [
            &self.active.active_play_id.0,
            &self.active.plan_id.0,
            &self.active.host_id.0,
            &self.active.boot_id.0,
        ] {
            bytes = bytes.checked_add(text.capacity())?;
        }
        if let Some(staged) = &self.staged {
            bytes = bytes.checked_add(staged.show.retained_backing_bytes()?)?;
        }
        if let Some(pending) = &self.pending {
            bytes = bytes.checked_add(pending.show.retained_backing_bytes()?)?;
        }
        if let Some(retained) = &self.retained {
            bytes = bytes.checked_add(retained.show.retained_backing_bytes()?)?;
        }
        Some(bytes)
    }
    /// Freeze the exact serialized Face and prepared pages before placing its
    /// Value on the Mask Fore. A later publication cannot substitute a different
    /// revision while the admitted kernel advances to its renderer Host Call.
    /// All cold preparation and transient overlap remain funded by Root.
    pub fn stage_face(&mut self, prepared_sign: c::SignId) -> Result<&[u8], Refusal> {
        if self.pending.is_some() || self.staged.is_some() || self.frame_pending {
            return Err(Refusal::Pending);
        }
        let ready = &mut self.ready;
        let context = &self.context;
        let active = &self.active;
        let (show, pages, length) = self
            .reader
            .with_retained(|face, _| {
                let bytes = f::postcard_wire::encode_face(face, ready.wire.face_mut())
                    .map_err(|_| Refusal::Encoding)?
                    .len();
                let pages = ready.mask.prepare_show(face).map_err(|_| Refusal::Face)?;
                let show = context
                    .prepared_show(
                        face,
                        active.clone(),
                        "environment".into(),
                        "local-segmented-display".into(),
                        prepared_sign,
                    )
                    .map_err(|_| Refusal::Face)?;
                Ok((show, pages, bytes))
            })
            .map_err(|_| Refusal::Face)??;
        // The enclosing Root separately funds transient construction overlap;
        // retained old/new Shows must both fit this exact installed ceiling.
        let retained = self
            .retained_backing_bytes()
            .and_then(|n| n.checked_add(show.retained_backing_bytes()?))
            .and_then(|n| u64::try_from(n).ok());
        if retained.is_none_or(|n| n > self.ready.bounds.resource_bytes) {
            return Err(Refusal::Bounds);
        }
        self.staged = Some(Staged { show, pages });
        self.face_input_len = length;
        Ok(&self.ready.wire.bytes()[..length])
    }
    /// Cold per-Face preparation under the enclosing admitted preparation owner.
    /// It validates the exact kernel input; it performs no output or clock wait.
    pub fn prepare_request(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        prepared_sign: c::SignId,
    ) -> Result<(), Refusal> {
        if node != self.node
            || call != HostCallId(0)
            || request != RequestId(self.next_request)
            || self.pending.is_some()
            || self.frame_pending
        {
            return Err(Refusal::Scope);
        }
        let next = self.next_request.checked_add(1).ok_or(Refusal::Bounds)?;
        let now = self.ready.clock.now_ms();
        if now < self.last_clock {
            return Err(Refusal::Clock);
        }
        if self.staged.is_none() {
            self.stage_face(prepared_sign)?;
        }
        if input != &self.ready.wire.bytes()[..self.face_input_len] {
            return Err(Refusal::Face);
        }
        let Staged { show, pages } = self.staged.take().ok_or(Refusal::Pending)?;
        let lease = self.operation.authorize().map_err(Refusal::Capability)?;
        self.next_request = next;
        self.last_clock = now;
        self.pending = Some(Pending {
            request,
            lease,
            show,
            pages,
            started: now,
        });
        Ok(())
    }
    /// Frame Info must be enqueued through the exact admitted output Plot Fore.
    /// Repeated polling cannot create another frame until Root acknowledges it.
    pub fn frame(&mut self) -> Result<Option<OutputFrame<'_>>, Refusal> {
        if self.frame_pending {
            return Ok(None);
        }
        if self.frames == self.ready.bounds.maximum_frames {
            return Err(Refusal::Bounds);
        }
        let now = self.ready.clock.now_ms();
        if now < self.last_clock {
            return Err(Refusal::Clock);
        }
        let (pages, started) = if let Some(pending) = &self.pending {
            (&pending.pages, pending.started)
        } else if let Some(retained) = &self.retained {
            (&retained.pages, retained.started)
        } else {
            return Ok(None);
        };
        self.last_clock = now;
        let frame = self
            .ready
            .mask
            .render_prepared(pages, now.checked_sub(started).ok_or(Refusal::Clock)?)
            .map_err(|_| Refusal::Face)?;
        if self.pending.is_none() && frame.canonical == &self.last_frame[..self.last_frame_len] {
            return Ok(None);
        }
        if frame.canonical.len() > self.last_frame.len() {
            return Err(Refusal::Bounds);
        }
        self.last_frame[..frame.canonical.len()].copy_from_slice(frame.canonical);
        self.last_frame_len = frame.canonical.len();
        self.frame_pending = true;
        self.frames += 1;
        Ok(Some(OutputFrame {
            ticket: FrameTicket(self.frames),
            canonical: frame.canonical,
        }))
    }
    /// # Safety
    /// Root observed completion of this exact pending frame by the admitted
    /// output fragment's original electrical Host Call. Transport delivery alone
    /// is insufficient. Refusal/cancellation must call revoke instead.
    pub unsafe fn acknowledge_frame(
        &mut self,
        ticket: FrameTicket,
        available_sign: c::SignId,
    ) -> Result<Option<(RequestId, &[u8])>, Refusal> {
        if !self.frame_pending || ticket != FrameTicket(self.frames) {
            return Err(Refusal::Pending);
        }
        self.frame_pending = false;
        let Some(pending) = self.pending.take() else {
            return Ok(None);
        };
        let prepared = (|| {
            let show = pending
                .show
                .into_transition(f::ManifestationLifecycle::Available, available_sign)
                .map_err(|_| Refusal::Output)?;
            let length = f::postcard_wire::encode_show(&show, self.ready.wire.show_mut())
                .map_err(|_| Refusal::Encoding)?
                .len();
            Ok((show, length))
        })();
        let (show, length) = match prepared {
            Ok(value) => value,
            Err(error) => {
                let _ = self.operation.revoke();
                self.retained = None;
                return Err(error);
            }
        };
        if let Err(error) = self.operation.complete(pending.lease, length as u32) {
            let _ = self.operation.revoke();
            self.retained = None;
            return Err(Refusal::Capability(error));
        }
        self.retained = Some(Retained {
            show,
            pages: pending.pages,
            started: pending.started,
        });
        Ok(Some((pending.request, &self.ready.wire.bytes()[..length])))
    }
    pub fn face_changed(&self) -> Result<bool, Refusal> {
        self.reader
            .with_retained(|face, status| {
                status.source_sequence > 0
                    && self
                        .retained
                        .as_ref()
                        .is_none_or(|old| old.show.face_id != face.identity)
            })
            .map_err(|_| Refusal::Face)
    }
    pub fn maximum_show_bytes(&self) -> u32 {
        self.ready.bounds.show_bytes
    }
    pub fn current_show(&self) -> Option<&f::Show> {
        self.retained.as_ref().map(|value| &value.show)
    }
    pub fn revoke(&mut self) -> Result<(), Refusal> {
        self.frame_pending = false;
        self.pending = None;
        self.staged = None;
        self.face_input_len = 0;
        self.retained = None;
        self.operation.revoke().map_err(Refusal::Capability)
    }
}

impl<C, E: PreparedFrameEncoder, const FRAME: usize> Drop for RendererService<'_, C, E, FRAME> {
    fn drop(&mut self) {
        let _ = self.operation.revoke();
        self.ready.wire.clear();
    }
}

#[cfg(test)]
#[path = "execution_mask_service/tests.rs"]
mod tests;
