//! Explicit replacement after a Root copy failure through the production proxy.
//! This fixture does not inject a CPU fault or acknowledge a semantic Show.
use super::refuse;
use crate::{
    arch,
    display::{DisplayError, DisplayFormat, PixelTarget},
    native_compositor::{CompositorAdmission, NativeCompositor, NativeCompositorError},
    protected_compositor::{
        display_grant::DisplayGrant,
        display_owner::{DisplayOwner, SharedDisplayOwner},
        session_owner::OwnerError,
    },
};
use alloc::vec;
use conduit_core::{BootId, HostBaseId, HostId, ImplementationId, OfferGeneration, PlacementId};
use conduit_face::LayoutRect;
use conduitos_compositor_core::wire::{Request, Response};

fn format() -> DisplayFormat {
    DisplayFormat {
        width: 32,
        height: 24,
        pitch: 128,
        bits_per_pixel: 32,
        red_shift: 16,
        green_shift: 8,
        blue_shift: 0,
    }
}
fn owner() -> SharedDisplayOwner {
    let grant = DisplayGrant::new(
        HostId::from("recovery-host"),
        BootId::from("recovery-boot"),
        OfferGeneration(1),
        HostBaseId::from("recovery-display"),
        format(),
    )
    .unwrap_or_else(|_| refuse("compositor-recovery-grant"));
    DisplayOwner::prepare(grant).unwrap_or_else(|error| {
        use core::fmt::Write;
        let mut detail = crate::sign_format::FixedText::new();
        let _ = writeln!(
            detail,
            "CONDUIT_COMPOSITOR_RECOVERY_ADMISSION error={error:?} live_bytes={} capacity={}",
            crate::allocation::BOOT_ARENA.live_bytes(),
            crate::allocation::BOOT_ARENA.capacity()
        );
        arch::early_write(detail.as_bytes());
        refuse("compositor-recovery-owner")
    })
}
fn presenter(owner: SharedDisplayOwner) -> NativeCompositor {
    let admission = CompositorAdmission::new(
        HostId::from("recovery-host"),
        BootId::from("recovery-boot"),
        OfferGeneration(1),
        ImplementationId::from("presenter/native-graphical@1"),
        HostBaseId::from("recovery-display"),
        vec![PlacementId::from("recovery-placement")],
        vec!["recovery-surface".into()],
    )
    .unwrap_or_else(|_| refuse("compositor-recovery-admission"));
    let mut proxy = NativeCompositor::admitted(admission);
    proxy
        .attach_display_owner(owner)
        .unwrap_or_else(|_| refuse("compositor-recovery-attach"));
    proxy
        .admit_surface(
            "recovery-surface",
            LayoutRect {
                x: 0,
                y: 0,
                width: 16,
                height: 12,
            },
            0,
        )
        .unwrap_or_else(|_| refuse("compositor-recovery-surface"));
    proxy
}
struct Target {
    fail: bool,
    writes: u32,
}
impl PixelTarget for Target {
    fn format(&self) -> DisplayFormat {
        format()
    }
    fn write_pixel(&mut self, _: u32, _: u32, pixel: u32) -> Result<(), DisplayError> {
        if self.fail {
            return Err(DisplayError::Lost);
        }
        // No raster has been uploaded. Fresh activation must clear to black.
        if pixel != 0 {
            refuse("compositor-recovery-fresh-pixel");
        }
        self.writes += 1;
        Ok(())
    }
}
fn already_terminal_cleanup() {
    for callback in [false, true] {
        let old = owner();
        let mut proxy = presenter(old.clone());
        let allocated = crate::allocation::BOOT_ARENA.live_bytes();
        if callback {
            let result = old.borrow_mut().with_raster(1, 1, |sessions, lease| {
                lease.write_pixel(0, 0, 0x00ab_cdef)?;
                sessions.quarantine();
                Err::<(), DisplayError>(DisplayError::Lost)
            });
            if result != Ok(Err(DisplayError::Lost)) {
                refuse("compositor-recovery-callback-result");
            }
        } else {
            old.borrow_mut().sessions.quarantine();
            let mut target = Target {
                fail: false,
                writes: 0,
            };
            if proxy.compose_frame(&mut target).is_ok() || target.writes != 0 {
                refuse("compositor-recovery-terminal-compose");
            }
        }
        if allocated.saturating_sub(crate::allocation::BOOT_ARENA.live_bytes())
            < crate::protected_compositor::display_owner::STAGING_BYTES
            || old.borrow_mut().with_raster(1, 1, |_, _| ()).is_ok()
            || proxy.validate_active_session().is_ok()
        {
            refuse("compositor-recovery-terminal-storage");
        }
    }
}
pub(super) fn run() {
    already_terminal_cleanup();
    let old = owner();
    let mut proxy = presenter(old.clone());
    let session = old
        .borrow()
        .sessions
        .active_session()
        .unwrap_or_else(|| refuse("compositor-recovery-session"));
    let instance = old.borrow().sessions.service_instance();
    let entries = old.borrow().service_cost().entries;
    let allocated = crate::allocation::BOOT_ARENA.live_bytes();
    let mut failed = Target {
        fail: true,
        writes: 0,
    };
    if proxy.compose_frame(&mut failed) != Err(NativeCompositorError::Display(DisplayError::Lost))
        || old.borrow().service_cost().entries <= entries
        || old.borrow().sessions.is_active(session)
        || proxy.validate_active_session().is_ok()
    {
        refuse("compositor-recovery-copy-failure");
    }
    if allocated.saturating_sub(crate::allocation::BOOT_ARENA.live_bytes())
        < crate::protected_compositor::display_owner::STAGING_BYTES
        || old.borrow_mut().with_raster(1, 1, |_, _| ()).is_ok()
    {
        refuse("compositor-recovery-staging-release");
    }
    // Replacement is explicit; old Rc clients and the old proxy remain alive.
    let fresh = owner();
    if fresh.borrow().sessions.service_instance() <= instance {
        refuse("compositor-recovery-instance");
    }
    let mut replacement = presenter(fresh.clone());
    let fresh_session = fresh
        .borrow()
        .sessions
        .active_session()
        .unwrap_or_else(|| refuse("compositor-recovery-fresh-session"));
    let before = fresh.borrow().service_cost();
    if old
        .borrow_mut()
        .sessions
        .request(session, Request::DamagePending)
        != Err(OwnerError::Terminated)
        || old.borrow_mut().sessions.upload_chunk(session, 0, &[1]) != Err(OwnerError::Terminated)
        || proxy.set_cursor_hover(true).is_ok()
        || proxy
            .admit_surface(
                "recovery-surface",
                LayoutRect {
                    x: 0,
                    y: 0,
                    width: 16,
                    height: 12,
                },
                0,
            )
            .is_ok()
        || fresh.borrow().service_cost() != before
    {
        refuse("compositor-recovery-stale-client");
    }
    drop(proxy);
    drop(old);
    let mut target = Target {
        fail: false,
        writes: 0,
    };
    let frame = replacement
        .compose_frame(&mut target)
        .unwrap_or_else(|_| refuse("compositor-recovery-fresh-frame"));
    if target.writes != 32 * 24
        || frame.pixels_written != 32 * 24
        || replacement.receipts().next().is_some()
        || fresh
            .borrow_mut()
            .sessions
            .request(fresh_session, Request::DamagePending)
            != Ok(Response::DamagePending(false))
    {
        refuse("compositor-recovery-fresh-state");
    }
    drop(replacement);
    drop(fresh);
    arch::early_write(b"CONDUIT_COMPOSITOR_RECOVERY_PROOF {\"schema\":\"conduit.conduitos/compositor-copy-recovery-fixture@2\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"fixture\":true,\"root_copy_failure\":true,\"guest_cpu_fault\":false,\"malformed_guest_reply\":false,\"explicit_fresh_owner\":true,\"old_staging_released\":true,\"already_terminal_staging_released\":true,\"callback_terminal_staging_released\":true,\"old_clients_alive_at_replacement\":true,\"stale_clients_refused\":true,\"stale_drop_preserves_replacement\":true,\"semantic_show_acknowledged\":false,\"physical_scanout\":false,\"fresh_black_pixels\":768}\n");
}
