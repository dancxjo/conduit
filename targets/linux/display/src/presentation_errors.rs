//! SDL2's void RenderPresent can log a failed DRM modeset without setting
//! SDL_GetError. Retain a process-lifetime failure epoch and forward logs to
//! the original sink. A prepared display never recovers from a later error.
use sdl2::sys;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Once, OnceLock,
};

static INSTALL: Once = Once::new();
static ORIGINAL: OnceLock<(sys::SDL_LogOutputFunction, usize)> = OnceLock::new();
static ERRORS: AtomicU64 = AtomicU64::new(0);

pub fn epoch() -> u64 {
    INSTALL.call_once(|| {
        let mut callback = None;
        let mut userdata = std::ptr::null_mut();
        // SDL serializes its log callback. The original sink and userdata have
        // their existing process lifetime; our callback owns no borrowed state.
        unsafe { sys::SDL_LogGetOutputFunction(&mut callback, &mut userdata) };
        let _ = ORIGINAL.set((callback, userdata as usize));
        unsafe { sys::SDL_LogSetOutputFunction(Some(capture), std::ptr::null_mut()) };
    });
    ERRORS.load(Ordering::SeqCst)
}

pub fn require_unchanged(epoch: u64) -> Result<(), String> {
    let mut callback = None;
    let mut userdata = std::ptr::null_mut();
    // A replacement/reset of the process-global sink withdraws our evidence.
    unsafe { sys::SDL_LogGetOutputFunction(&mut callback, &mut userdata) };
    let installed = callback
        .is_some_and(|f| std::ptr::fn_addr_eq(f, capture as unsafe extern "C" fn(_, _, _, _)))
        && userdata.is_null();
    if !installed || ERRORS.load(Ordering::SeqCst) != epoch {
        Err("sdl-display-error; display reacquisition required".into())
    } else {
        Ok(())
    }
}

unsafe extern "C" fn capture(
    _: *mut std::ffi::c_void,
    category: std::ffi::c_int,
    priority: sys::SDL_LogPriority,
    message: *const std::ffi::c_char,
) {
    // Refuse conservatively on every SDL error, including uncategorized DRM
    // errors. No string allocation or parsing occurs on this callback path.
    if priority as u32 >= sys::SDL_LogPriority::SDL_LOG_PRIORITY_ERROR as u32 {
        ERRORS.fetch_add(1, Ordering::SeqCst);
    }
    if let Some((Some(callback), userdata)) = ORIGINAL.get() {
        unsafe {
            callback(
                *userdata as *mut std::ffi::c_void,
                category,
                priority,
                message,
            )
        };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn logged_error_invalidates_previous_display_epoch_only() {
        let before = super::epoch();
        super::require_unchanged(before).unwrap();
        sdl2::clear_error();
        sdl2::log::log_error("injected failed DRM modeset");
        assert!(sdl2::get_error().is_empty());
        assert!(super::require_unchanged(before).is_err());
        let reacquired = super::epoch();
        super::require_unchanged(reacquired).unwrap();
        assert!(super::require_unchanged(before).is_err());
    }
}
