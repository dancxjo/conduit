//! Nonserializable lifetime observation of one actually initialized SDL scanout.
//! This token grants no Host resource, issuer key, Mask Show or presentation.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);
struct Lease {
    backend: &'static str,
    renderer: &'static str,
    window_id: u32,
    width: u32,
    height: u32,
    window_width: u32,
    window_height: u32,
    instance_nonce: u64,
    live: AtomicBool,
}
/// Only Scanout preparation issues this cloneable observation. Clones share
/// terminal loss; SDL dummy/software observations remain explicitly identifiable.
#[derive(Clone)]
pub struct InitializedDisplay {
    lease: Arc<Lease>,
}
impl InitializedDisplay {
    pub(super) fn prepared(
        canvas: &sdl2::render::Canvas<sdl2::video::Window>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let instance_nonce = NEXT_INSTANCE
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            .map_err(|_| "display-instance-nonce-exhausted")?;
        let (window_width, window_height) = canvas.window().size();
        Ok(Self {
            lease: Arc::new(Lease {
                backend: canvas.window().subsystem().current_video_driver(),
                renderer: canvas.info().name,
                window_id: canvas.window().id(),
                width,
                height,
                window_width,
                window_height,
                instance_nonce,
                live: AtomicBool::new(true),
            }),
        })
    }
    pub fn backend(&self) -> &str {
        self.lease.backend
    }
    pub fn renderer(&self) -> &str {
        self.lease.renderer
    }
    pub fn window_id(&self) -> u32 {
        self.lease.window_id
    }
    pub fn width(&self) -> u32 {
        self.lease.width
    }
    pub fn height(&self) -> u32 {
        self.lease.height
    }
    pub fn window_size(&self) -> (u32, u32) {
        (self.lease.window_width, self.lease.window_height)
    }
    /// Unique only within this process; bind to the actual Host Boot when admitted.
    pub fn instance_nonce(&self) -> u64 {
        self.lease.instance_nonce
    }
    pub fn is_live(&self) -> bool {
        self.lease.live.load(Ordering::Acquire)
    }
    /// Reports only the observed SDL driver, never independently proved DRM output.
    pub fn is_kmsdrm(&self) -> bool {
        self.lease.backend == "kmsdrm"
    }
    pub(super) fn revoke(&self) {
        self.lease.live.store(false, Ordering::Release);
    }
}
