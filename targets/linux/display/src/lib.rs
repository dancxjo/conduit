//! Linux scanout adapter below the ordinary Face graphics Show. This module
//! owns no Body, observation, Mask policy, scheduler, or recording authority.
mod initialized;
mod input;
mod presentation_errors;
pub use input::{NativeInput, NativeInputRefusal};
mod surface;
use conduit_face::GraphicsScene;
use conduitos::display::{DisplayError, DisplayFormat, DisplayReceipt, PixelTarget};
pub use initialized::{DisplayClaimRefusal, DisplayHostClaim, InitializedDisplay};
pub use sdl2::EventPump;
use sdl2::{
    pixels::PixelFormatEnum,
    render::{Canvas, Texture, TextureCreator},
    video::{Window, WindowContext},
};
pub use surface::Surface;

// SDL reports the direct backend with this exact case-sensitive identity.
const KMSDRM_DRIVER: &str = "KMSDRM";
fn is_kmsdrm_backend(driver: &str) -> bool {
    driver == KMSDRM_DRIVER
}

/// Prepared texture and finite CPU surface. The caller owns renderer lifetime
/// and must call `lose` when its admitted display custody is withdrawn.
pub struct Scanout<'a> {
    canvas: &'a mut Canvas<Window>,
    texture: Texture<'a>,
    surface: Surface,
    live: bool,
    input_ready: bool,
    error_epoch: u64,
    initialized: InitializedDisplay,
}
impl<'a> Scanout<'a> {
    pub fn prepare(
        canvas: &'a mut Canvas<Window>,
        textures: &'a TextureCreator<WindowContext>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let error_epoch = presentation_errors::epoch();
        let surface = Surface::prepare(width, height).map_err(str::to_owned)?;
        let mut texture = textures
            .create_texture_streaming(PixelFormatEnum::ARGB8888, width, height)
            .map_err(|e| e.to_string())?;
        texture.set_blend_mode(sdl2::render::BlendMode::None);
        let initialized = InitializedDisplay::prepared(canvas, width, height)?;
        Ok(Self {
            canvas,
            texture,
            surface,
            live: true,
            input_ready: false,
            error_epoch,
            initialized,
        })
    }
    /// Observe this exact live preparation; neither a CPU Surface nor copied
    /// advertisement bytes can construct an InitializedDisplay.
    pub fn initialized_display(&self) -> Result<InitializedDisplay, String> {
        if !self.live || !self.initialized.is_live() {
            return Err("display-custody-lost".into());
        }
        Ok(self.initialized.clone())
    }
    /// Bind Host attachment custody to this exact live SDL preparation.
    pub fn matches_initialized(&self, initialized: &InitializedDisplay) -> bool {
        self.live && self.initialized.is_live() && self.initialized.same_instance(initialized)
    }
    pub fn present(&mut self, scene: &GraphicsScene) -> Result<DisplayReceipt, String> {
        if !self.live || !self.initialized.is_live() {
            return Err("display-custody-lost".into());
        }
        let result = self.present_inner(scene);
        if result.is_err() {
            self.lose();
        }
        result
    }
    fn present_inner(&mut self, scene: &GraphicsScene) -> Result<DisplayReceipt, String> {
        let receipt = self.surface.render(scene).map_err(str::to_owned)?;
        self.commit_pixels()?;
        Ok(receipt)
    }
    fn commit_pixels(&mut self) -> Result<(), String> {
        if !self.live || !self.initialized.is_live() {
            return Err("display-custody-lost".into());
        }
        let result = self.commit_pixels_inner();
        if result.is_err() {
            self.lose();
        }
        result
    }
    fn commit_pixels_inner(&mut self) -> Result<(), String> {
        self.texture
            .update(None, self.surface.bytes(), self.surface.pitch())
            .map_err(|e| e.to_string())?;
        self.canvas.copy(&self.texture, None, None)?;
        presentation_errors::require_unchanged(self.error_epoch)?;
        self.canvas.present();
        presentation_errors::require_unchanged(self.error_epoch)
    }
    /// Revocation is terminal for this prepared scanout. Reacquisition requires
    /// a new preparation; device availability alone cannot restore authority.
    pub fn lose(&mut self) {
        self.initialized.revoke();
        self.live = false;
        self.surface.lose();
    }
}

impl conduitos::product_bases::InitializedNativeOutput for Scanout<'_> {
    fn require_live(&self) -> Result<(), DisplayError> {
        if self.live && self.initialized.is_live() {
            Ok(())
        } else {
            Err(DisplayError::Lost)
        }
    }
}

impl Drop for Scanout<'_> {
    fn drop(&mut self) {
        self.lose();
    }
}

/// Compositor writes reuse the one prepared Surface. Only commit performs SDL
/// upload/copy/present; successful CPU writes cannot acknowledge a Mask Show.
impl PixelTarget for Scanout<'_> {
    fn format(&self) -> DisplayFormat {
        self.surface.format()
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if !self.live || !self.initialized.is_live() {
            return Err(DisplayError::Lost);
        }
        let result = self.surface.write_pixel(x, y, pixel);
        if result.is_err() {
            self.lose();
        }
        result
    }
    fn commit_frame(&mut self) -> Result<(), DisplayError> {
        self.commit_pixels().map_err(|_| DisplayError::Lost)
    }
}

/// Preparation entrance for an actual direct Linux display. Refuse any other
/// SDL backend, including implicit desktop fallbacks. This is not a Host offer:
/// the enclosing Host must admit the actual display resource and present call.
pub fn with_kmsdrm<T>(
    width: u32,
    height: u32,
    run: impl FnOnce(&mut Scanout<'_>, &mut sdl2::EventPump) -> Result<T, String>,
) -> Result<T, String> {
    let error_epoch = presentation_errors::epoch();
    if !sdl2::hint::set("SDL_VIDEODRIVER", KMSDRM_DRIVER) {
        return Err("kmsdrm-driver-selection-refused".into());
    }
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    if !is_kmsdrm_backend(video.current_video_driver()) {
        return Err("kmsdrm-driver-unavailable".into());
    }
    let window = video
        .window("Conduit", width, height)
        .fullscreen_desktop()
        .build()
        .map_err(|e| e.to_string())?;
    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    // KMSDRM can recreate its EGL surfaces on the first swap. Complete that
    // initialization before issuing display readiness or drawing a Mask Show;
    // otherwise the first content frame can disappear until another repaint.
    canvas.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();
    presentation_errors::require_unchanged(error_epoch)?;
    let textures = canvas.texture_creator();
    let mut scanout = Scanout::prepare(&mut canvas, &textures, width, height)?;
    let mut events = sdl.event_pump()?;
    run(&mut scanout, &mut events)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod input_queue_tests;
