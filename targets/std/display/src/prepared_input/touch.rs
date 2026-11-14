//! Actual provider evidence for SDL's Linux evdev global-coordinate touch path.
//! Verified SDL sources: release-2.32.4 and release-2.32.10, src/core/linux/
//! SDL_evdev.c: AddTouch(item->fd, ...) and SendTouch(item->fd, ..., NULL, ...).
//! https://github.com/libsdl-org/SDL/blob/release-2.32.10/src/core/linux/SDL_evdev.c
//! Numeric IDs are NOT file descriptors for other SDL implementations/versions.
use crate::InitializedDisplay;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Device {
    id: i64,
    direct: bool,
}
fn inventory() -> Result<Option<Device>, ()> {
    let count = sdl2::touch::num_touch_devices();
    if !(0..=16).contains(&count) {
        return Err(());
    }
    let mut found = None;
    for index in 0..count {
        let id = sdl2::touch::touch_device(index);
        if id == -1 {
            continue;
        }
        if found.is_some() {
            return Err(());
        }
        // SAFETY: this is an actual device ID from the initialized SDL inventory
        // on its exclusive event-pump thread; no caller label is interpreted.
        let direct = unsafe { sdl2::sys::SDL_GetTouchDeviceType(id) }
            == sdl2::sys::SDL_TouchDeviceType::SDL_TOUCH_DEVICE_DIRECT;
        found = Some(Device { id, direct });
    }
    Ok(found)
}
fn supported_version() -> bool {
    let v = sdl2::version::version();
    (v.major, v.minor) == (2, 32) && matches!(v.patch, 4 | 10)
}
fn one_fullscreen_output(fullscreen_desktop: bool, displays: i32, index: i32) -> bool {
    fullscreen_desktop && displays == 1 && index == 0
}
fn spatial(display: &InitializedDisplay, window: &sdl2::video::Window) -> bool {
    window.id() == display.window_id()
        && window.subsystem().current_video_driver() == "KMSDRM"
        && one_fullscreen_output(
            window.fullscreen_state() == sdl2::video::FullscreenType::Desktop,
            window.subsystem().num_video_displays().unwrap_or(-1),
            window.display_index().unwrap_or(-1),
        )
        && window.subsystem().display_bounds(0).is_ok_and(|bounds| {
            window.size() == (bounds.width(), bounds.height())
                && window.position() == (bounds.x(), bounds.y())
                && window.size() == display.window_size()
        })
}
/// Immutable evidence owned by the SAME original prepared SDL input lease.
/// One descriptor duplicate at most; no registration map or polling allocation.
pub(crate) struct TouchBinding {
    device: Option<Device>,
    #[cfg(target_os = "linux")]
    evdev: std::sync::Mutex<Option<linux::Evdev>>,
}
impl TouchBinding {
    pub(crate) fn observe(
        display: &InitializedDisplay,
        window: &sdl2::video::Window,
    ) -> Result<Self, ()> {
        // Ordinary window-bound input has no global device-selection policy.
        // In particular, multiple/changed devices on X11/Wayland/dummy cannot
        // invalidate keyboard or mouse readiness through this evdev adapter.
        let device = if cfg!(target_os = "linux") && display.backend() == "KMSDRM" {
            inventory()?.filter(|device| device.direct)
        } else {
            None
        };
        #[cfg(target_os = "linux")]
        let evdev = if display.backend() == "KMSDRM" && device.is_some_and(|d| d.direct) {
            if !supported_version() || !spatial(display, window) {
                return Err(());
            }
            Some(linux::Evdev::capture(device.ok_or(())?.id)?)
        } else {
            None
        };
        #[cfg(not(target_os = "linux"))]
        {
            let _ = display;
        }
        Ok(Self {
            device,
            #[cfg(target_os = "linux")]
            evdev: std::sync::Mutex::new(evdev),
        })
    }
    pub(crate) fn check(
        &self,
        display: &InitializedDisplay,
        window: &sdl2::video::Window,
    ) -> Result<(), ()> {
        if !display.is_live() || window.id() != display.window_id() {
            return Err(());
        }
        #[cfg(target_os = "linux")]
        if let Some(evdev) = self.evdev.lock().map_err(|_| ())?.as_ref() {
            if display.backend() != "KMSDRM"
                || !supported_version()
                || !spatial(display, window)
                || inventory()? != self.device
            {
                return Err(());
            }
            evdev.check(self.device.ok_or(())?.id)?;
        }
        Ok(())
    }
    /// Retire the one physical lifetime witness even if revoked readiness
    /// tokens remain retained for inspection. A concurrent check holds the same
    /// fixed lock; no fd can be closed/reused underneath its liveness checks.
    pub(crate) fn retire(&self) {
        #[cfg(target_os = "linux")]
        {
            let mut owner = self
                .evdev
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            owner.take();
        }
    }
    pub(crate) fn global_matches(&self, id: i64) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.evdev.lock().is_ok_and(|owner| owner.is_some())
                && self.device == Some(Device { id, direct: true })
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = id;
            false
        }
    }
}
#[cfg(target_os = "linux")]
mod linux {
    use std::{
        fs::File,
        os::fd::{AsRawFd, FromRawFd},
    };
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Identity {
        device: libc::dev_t,
        inode: libc::ino_t,
        rdev: libc::dev_t,
    }
    fn identity(fd: i32) -> Result<Identity, ()> {
        let mut stat = core::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: fstat accepts any descriptor and initializes the entire stat
        // value only on success. No path buffer, polling allocation or second
        // descriptor owner is needed for current SDL fd identity validation.
        let m = unsafe {
            if libc::fstat(fd, stat.as_mut_ptr()) != 0 {
                return Err(());
            }
            stat.assume_init()
        };
        if m.st_mode & libc::S_IFMT != libc::S_IFCHR
            || libc::major(m.st_rdev) != 13
            || libc::minor(m.st_rdev) < 64
        {
            return Err(());
        }
        Ok(Identity {
            device: m.st_dev,
            inode: m.st_ino,
            rdev: m.st_rdev,
        })
    }
    fn duplicate(id: i64) -> Result<File, ()> {
        let fd = i32::try_from(id).map_err(|_| ())?;
        if fd < 0 {
            return Err(());
        }
        // SAFETY: fcntl safely refuses nonexistent descriptors. A successful
        // F_DUPFD_CLOEXEC transfers exactly one fresh owned descriptor to File.
        let duplicated = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
        if duplicated < 0 {
            return Err(());
        }
        Ok(unsafe { File::from_raw_fd(duplicated) })
    }
    fn alive(fd: i32) -> Result<(), ()> {
        let mut version = 0_i32;
        // Linux input ABI _IOR('E', 0x01, int). EVIOCGVERSION checks the held
        // evdev object's `exist` flag, returning ENODEV after unplug. Thus a new
        // device reusing the SDL numeric fd cannot revive this ORIGINAL owner.
        let result = unsafe { libc::ioctl(fd, 0x80044501 as libc::c_ulong, &mut version) };
        if result != 0 || version <= 0 {
            return Err(());
        }
        Ok(())
    }
    pub(super) struct Evdev {
        original: File,
        identity: Identity,
    }
    impl Evdev {
        pub(super) fn capture(id: i64) -> Result<Self, ()> {
            let original = duplicate(id)?;
            let identity = identity(original.as_raw_fd())?;
            alive(original.as_raw_fd())?;
            Ok(Self { original, identity })
        }
        pub(super) fn check(&self, id: i64) -> Result<(), ()> {
            alive(self.original.as_raw_fd())?;
            let current = i32::try_from(id).map_err(|_| ())?;
            if identity(current)? != self.identity {
                return Err(());
            }
            alive(current)
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn closed_or_non_evdev_descriptors_cannot_mint_touch_custody() {
            assert!(Evdev::capture(-1).is_err());
            assert!(Evdev::capture(i64::MAX).is_err());
            let null = File::open("/dev/null").unwrap();
            assert!(Evdev::capture(i64::from(null.as_raw_fd())).is_err());
            let temp = duplicate(i64::from(null.as_raw_fd())).unwrap();
            let closed = temp.as_raw_fd();
            drop(temp);
            assert!(Evdev::capture(i64::from(closed)).is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_or_partial_output_requires_explicit_mapping() {
        assert!(one_fullscreen_output(true, 1, 0));
        assert!(!one_fullscreen_output(true, 2, 0));
        assert!(!one_fullscreen_output(true, 1, 1));
        assert!(!one_fullscreen_output(false, 1, 0));
        assert!(!one_fullscreen_output(true, -1, 0));
    }
    #[test]
    fn dummy_observation_never_issues_a_global_evdev_binding() {
        let _serial = crate::SDL_TEST_LOCK.lock().unwrap();
        assert!(sdl2::hint::set("SDL_VIDEODRIVER", "dummy"));
        let sdl = sdl2::init().unwrap();
        let video = sdl.video().unwrap();
        let window = video
            .window("unqualified global touch", 32, 24)
            .build()
            .unwrap();
        let mut canvas = window.into_canvas().software().build().unwrap();
        let textures = canvas.texture_creator();
        let output = crate::Scanout::prepare(&mut canvas, &textures, 32, 24).unwrap();
        let display = output.initialized_display().unwrap();
        let binding = TouchBinding::observe(&display, output.canvas.window()).unwrap();
        assert!(!binding.global_matches(0));
        assert!(!binding.global_matches(8));
        binding.check(&display, output.canvas.window()).unwrap();
        assert!(
            !spatial(&display, output.canvas.window()),
            "a label cannot upgrade dummy into KMSDRM custody"
        );
    }
}
