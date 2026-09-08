use alloc::vec::Vec;

use conduit_presentation::LayoutRect;

use super::NativeCompositorError;

pub const MAX_DAMAGE_RECTS: usize = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DamageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RawDamageRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl RawDamageRect {
    pub(super) fn new(
        left: i32,
        top: i32,
        width: u16,
        height: u16,
    ) -> Result<Self, NativeCompositorError> {
        Self::from_layout(LayoutRect {
            x: i16::try_from(left).map_err(|_| NativeCompositorError::InvalidBounds)?,
            y: i16::try_from(top).map_err(|_| NativeCompositorError::InvalidBounds)?,
            width,
            height,
        })
    }
    pub(super) fn from_layout(bounds: LayoutRect) -> Result<Self, NativeCompositorError> {
        if bounds.width == 0 || bounds.height == 0 {
            return Err(NativeCompositorError::InvalidBounds);
        }
        let left = i32::from(bounds.x);
        let top = i32::from(bounds.y);
        let right = left
            .checked_add(i32::from(bounds.width))
            .ok_or(NativeCompositorError::InvalidBounds)?;
        let bottom = top
            .checked_add(i32::from(bounds.height))
            .ok_or(NativeCompositorError::InvalidBounds)?;
        Ok(Self {
            left,
            top,
            right,
            bottom,
        })
    }

    pub(super) fn intersection(self, other: Self) -> Option<Self> {
        let value = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (value.left < value.right && value.top < value.bottom).then_some(value)
    }

    fn touches(self, other: Self) -> bool {
        self.left <= other.right
            && other.left <= self.right
            && self.top <= other.bottom
            && other.top <= self.bottom
    }

    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    pub(super) fn clip(self, width: u32, height: u32) -> Option<DamageRect> {
        let left = i64::from(self.left).max(0).min(i64::from(width));
        let top = i64::from(self.top).max(0).min(i64::from(height));
        let right = i64::from(self.right).max(0).min(i64::from(width));
        let bottom = i64::from(self.bottom).max(0).min(i64::from(height));
        (left < right && top < bottom).then(|| DamageRect {
            x: u32::try_from(left).unwrap_or(0),
            y: u32::try_from(top).unwrap_or(0),
            width: u32::try_from(right - left).unwrap_or(0),
            height: u32::try_from(bottom - top).unwrap_or(0),
        })
    }
}

pub(super) struct DamageState {
    rects: Vec<RawDamageRect>,
    fallback: bool,
}

impl DamageState {
    pub(super) const fn new() -> Self {
        Self {
            rects: Vec::new(),
            fallback: false,
        }
    }

    pub(super) fn add_layout(&mut self, bounds: LayoutRect) -> Result<(), NativeCompositorError> {
        self.add(RawDamageRect::from_layout(bounds)?);
        Ok(())
    }

    pub(super) fn add(&mut self, rect: RawDamageRect) {
        let mut merged = rect;
        let mut index = 0;
        while index < self.rects.len() {
            if merged.touches(self.rects[index]) {
                merged = merged.union(self.rects.remove(index));
                index = 0;
            } else {
                index += 1;
            }
        }
        if self.rects.len() == MAX_DAMAGE_RECTS {
            for existing in self.rects.drain(..) {
                merged = merged.union(existing);
            }
            self.fallback = true;
        }
        self.rects.push(merged);
    }

    pub(super) fn pending(&self) -> &[RawDamageRect] {
        &self.rects
    }

    pub(super) const fn used_fallback(&self) -> bool {
        self.fallback
    }

    pub(super) fn clear(&mut self) {
        self.rects.clear();
        self.fallback = false;
    }
}
