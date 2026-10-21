//! Explicit replacement after a Root copy failure through the production proxy.
//! This fixture does not inject a CPU fault or acknowledge a semantic Show.
use super::refuse;
use crate::{
    arch,
    display::{DisplayError, DisplayFormat, PixelTarget},
    native_compositor::{CompositorAdmission, NativeCompositor, NativeCompositorError},
    protected_compositor::{
        display_grant::{DisplayGrant, SharedDisplayGrant},
        display_owner::{DisplayOwner, SharedDisplayOwner},
        session_owner::OwnerError,
    },
};
use alloc::{rc::Rc, vec};
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
fn grant() -> SharedDisplayGrant {
    let grant = DisplayGrant::new(
        HostId::from("recovery-host"),
        BootId::from("recovery-boot"),
        OfferGeneration(1),
        HostBaseId::from("recovery-display"),
        format(),
    )
    .unwrap_or_else(|_| refuse("compositor-recovery-grant"));
    Rc::new(grant)
}
fn owner(grant: &SharedDisplayGrant) -> SharedDisplayOwner {
    // Root retains the same checked current display binding across explicit
    // recovery. No new strings or Rc metadata precede the staging reservation.
    DisplayOwner::prepare_shared(grant.clone()).unwrap_or_else(|error| {
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
fn pending_presenter(owner: SharedDisplayOwner) -> NativeCompositor {
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
}
fn presenter(owner: SharedDisplayOwner) -> NativeCompositor {
    let mut proxy = pending_presenter(owner);
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
fn already_terminal_cleanup(grant: &SharedDisplayGrant) {
    for callback in [false, true] {
        let old = owner(grant);
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
fn lifecycle_terminal_cleanup(grant: &SharedDisplayGrant) {
    for activation in [true, false] {
        let display = owner(grant);
        let mut proxy = if activation {
            pending_presenter(display.clone())
        } else {
            presenter(display.clone())
        };
        let allocated = crate::allocation::BOOT_ARENA.live_bytes();
        display.borrow_mut().sessions.quarantine();
        if activation {
            if proxy.admit_surface(
                "recovery-surface",
                LayoutRect {
                    x: 0,
                    y: 0,
                    width: 16,
                    height: 12,
                },
                0,
            ) != Err(NativeCompositorError::ServiceLost)
            {
                refuse("compositor-recovery-terminal-activation");
            }
        } else {
            proxy
                .remove_surface("recovery-surface")
                .unwrap_or_else(|_| refuse("compositor-recovery-terminal-remove"));
        }
        if allocated.saturating_sub(crate::allocation::BOOT_ARENA.live_bytes())
            < crate::protected_compositor::display_owner::STAGING_BYTES
            || display.borrow_mut().with_raster(1, 1, |_, _| ()).is_ok()
        {
            refuse("compositor-recovery-lifecycle-storage");
        }
    }
    let display = owner(grant);
    let mut retired = presenter(display.clone());
    let mut current = presenter(display.clone()); // explicit new presenter handoff
    let session = display
        .borrow()
        .sessions
        .active_session()
        .unwrap_or_else(|| refuse("compositor-recovery-handoff-session"));
    let cost = display.borrow().service_cost();
    retired
        .remove_surface("recovery-surface")
        .unwrap_or_else(|_| refuse("compositor-recovery-retired-remove"));
    if !display.borrow().sessions.is_active(session)
        || display.borrow().service_cost() != cost
        || display.borrow_mut().with_raster(1, 1, |_, _| ()).is_err()
    {
        refuse("compositor-recovery-retired-owner");
    }
    let mut target = Target {
        fail: false,
        writes: 0,
    };
    current
        .compose_frame(&mut target)
        .unwrap_or_else(|_| refuse("compositor-recovery-current-after-remove"));
}
pub(super) fn run() {
    let grant = grant();
    already_terminal_cleanup(&grant);
    lifecycle_terminal_cleanup(&grant);
    let old = owner(&grant);
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
    // Clone from Root's retained immutable grant, not from a reconstructed
    // identity admission after the large staging hole has been released.
    let attempts = crate::allocation::BOOT_ARENA.attempts();
    let retained = grant.clone();
    if !Rc::ptr_eq(&retained, &grant) || crate::allocation::BOOT_ARENA.attempts() != attempts {
        refuse("compositor-recovery-shared-grant");
    }
    let fresh = owner(&retained);
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
    replacement
        .remove_surface("recovery-surface")
        .unwrap_or_else(|_| refuse("compositor-recovery-last-remove"));
    target.writes = 0;
    let empty = replacement
        .compose_frame(&mut target)
        .unwrap_or_else(|_| refuse("compositor-recovery-empty-frame"));
    if empty.pixels_written != 16 * 12
        || target.writes != 16 * 12
        || empty.surfaces_composed != 0
        || replacement.focused_surface().is_some()
        || !fresh.borrow().sessions.is_active(fresh_session)
        || !matches!(
            replacement.route_keyboard(
                &serde_json::from_str("\"fixture/retired\"")
                    .unwrap_or_else(|_| refuse("compositor-recovery-retired-id")),
            ),
            Ok(crate::native_compositor::InputRoute::NoTarget)
        )
    {
        refuse("compositor-recovery-empty-scene");
    }
    drop(replacement);
    drop(fresh);
    arch::early_write(b"CONDUIT_COMPOSITOR_RECOVERY_PROOF {\"schema\":\"conduit.conduitos/compositor-copy-recovery-fixture@4\",\"status\":\"completed\",\"proof_class\":\"freestanding-emulator\",\"fixture\":true,\"root_copy_failure\":true,\"guest_cpu_fault\":false,\"malformed_guest_reply\":false,\"explicit_fresh_owner\":true,\"old_staging_released\":true,\"shared_current_display_grant\":true,\"last_remove_empty_scene_pixels\":192,\"terminal_activation_storage_released\":true,\"terminal_remove_storage_released\":true,\"retired_remove_preserves_current_owner\":true,\"already_terminal_staging_released\":true,\"callback_terminal_staging_released\":true,\"old_clients_alive_at_replacement\":true,\"stale_clients_refused\":true,\"stale_drop_preserves_replacement\":true,\"semantic_show_acknowledged\":false,\"physical_scanout\":false,\"fresh_black_pixels\":768}\n");
}
