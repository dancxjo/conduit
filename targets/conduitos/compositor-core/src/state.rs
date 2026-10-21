use crate::{damage::Damage, frame, frame::CompletedFrame};

pub const MAX_SURFACES: usize = 8;
pub const MAX_PIXELS: usize = 2 * 1024 * 1024;
pub const MAX_CHUNK: usize = 16 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Bounds,
    Capacity,
    Stale,
    Busy,
    Incomplete,
    Order,
    NoDamage,
    Exhausted,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}
impl Rect {
    pub(crate) fn pixels(self) -> Result<usize, Error> {
        if self.width == 0 || self.height == 0 {
            return Err(Error::Bounds);
        }
        let pixels = usize::from(self.width) * usize::from(self.height);
        if pixels > MAX_PIXELS {
            return Err(Error::Capacity);
        }
        Ok(pixels)
    }
    pub(crate) fn contains(self, x: u32, y: u32) -> bool {
        let (x, y) = (i64::from(x), i64::from(y));
        x >= i64::from(self.x)
            && x < i64::from(self.x) + i64::from(self.width)
            && y >= i64::from(self.y)
            && y < i64::from(self.y) + i64::from(self.height)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    pub slot: u8,
    pub generation: u64,
}
#[derive(Clone, Copy, Default)]
pub(crate) struct Surface {
    pub(crate) live: bool,
    pub(crate) generation: u64,
    pub(crate) order: u64,
    pub(crate) rect: Rect,
    pub(crate) z: u8,
    pub(crate) visible: bool,
    pub(crate) ready: bool,
    pub(crate) offset: usize,
    pub(crate) revision: u64,
}
#[derive(Clone, Copy)]
pub(crate) struct Upload {
    pub(crate) slot: usize,
    pub(crate) surfaces: [Surface; MAX_SURFACES],
    pub(crate) copy_slot: usize,
    pub(crate) copied: usize,
    pub(crate) written: usize,
}
/// Arenas have disjoint borrows. Their roles swap atomically after an upload;
/// there is one authoritative current raster, never a Root-side retained copy.
pub struct Compositor<'a> {
    pub(crate) pixels: &'a mut [u32],
    pub(crate) scratch: &'a mut [u32],
    pub(crate) surfaces: [Surface; MAX_SURFACES],
    pub(crate) upload: Option<Upload>,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) next_order: u64,
    pub(crate) sequence: u64,
    pub(crate) focus: Option<Handle>,
    pub(crate) cursor: Option<(u16, u16)>,
    pub(crate) hover: bool,
    pub(crate) damage: Damage,
    pub(crate) frame: Option<frame::Progress>,
    pub(crate) completed: Option<CompletedFrame>,
    pub(crate) acknowledged: [bool; MAX_SURFACES],
}
impl<'a> Compositor<'a> {
    pub fn new(
        pixels: &'a mut [u32],
        scratch: &'a mut [u32],
        width: u16,
        height: u16,
    ) -> Result<Self, Error> {
        if pixels.len() != MAX_PIXELS || scratch.len() != MAX_PIXELS {
            return Err(Error::Capacity);
        }
        if width == 0 || height == 0 {
            return Err(Error::Bounds);
        }
        Ok(Self {
            pixels,
            scratch,
            surfaces: [Surface::default(); MAX_SURFACES],
            upload: None,
            width,
            height,
            next_order: 0,
            sequence: 0,
            focus: None,
            cursor: None,
            hover: false,
            damage: Damage::new(),
            frame: None,
            completed: None,
            acknowledged: [false; MAX_SURFACES],
        })
    }
    pub(crate) fn index(&self, handle: Handle) -> Result<usize, Error> {
        let index = usize::from(handle.slot);
        self.surfaces
            .get(index)
            .filter(|s| s.live && s.generation == handle.generation)
            .map(|_| index)
            .ok_or(Error::Stale)
    }
    pub(crate) fn idle(&self) -> Result<(), Error> {
        if self.upload.is_some() {
            Err(Error::Busy)
        } else {
            Ok(())
        }
    }
    pub(crate) fn invalidate(&mut self) {
        self.frame = None;
        self.completed = None;
        self.acknowledged.fill(false);
    }
    pub(crate) fn selected(&self, x: u32, y: u32) -> Option<usize> {
        (0..MAX_SURFACES)
            .filter(|&i| {
                let s = self.surfaces[i];
                s.live && s.visible && s.ready && s.rect.contains(x, y)
            })
            .max_by_key(|&i| (self.surfaces[i].z, self.surfaces[i].order))
    }
}
