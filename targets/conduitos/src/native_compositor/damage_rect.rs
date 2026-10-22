//! Scanout receipt geometry; damage planning belongs to the selected renderer.
pub const MAX_DAMAGE_RECTS: usize = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
