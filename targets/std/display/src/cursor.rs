//! Exclusive KMSDRM uses the compositor's pointer, not a second SDL cursor.
//! Keep SDL alive and restore the previous visibility before video teardown,
//! including callback errors and Rust unwinding.
pub(super) struct HiddenCursor {
    mouse: sdl2::mouse::MouseUtil,
    was_visible: bool,
}
impl HiddenCursor {
    pub(super) fn prepare(sdl: &sdl2::Sdl) -> Result<Self, String> {
        let mouse = sdl.mouse();
        let guard = Self {
            was_visible: mouse.is_cursor_showing(),
            mouse,
        };
        guard.mouse.show_cursor(false);
        if guard.mouse.is_cursor_showing() {
            return Err("kmsdrm-sdl-cursor-hide-refused".into());
        }
        Ok(guard)
    }
}
impl Drop for HiddenCursor {
    fn drop(&mut self) {
        self.mouse.show_cursor(self.was_visible);
    }
}
