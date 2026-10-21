use crate::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub surface: Handle,
    /// Renderer-local upload sequence; Root separately binds semantic Face/Show.
    pub revision: u64,
    pub x: u16,
    pub y: u16,
}
impl Compositor<'_> {
    /// Admission creates metadata only; an initial raster upload makes it visible.
    pub fn admit(&mut self, rect: Rect, z: u8) -> Result<Handle, Error> {
        self.idle()?;
        let reserved = self
            .surfaces
            .iter()
            .filter(|s| s.live)
            .try_fold(rect.pixels()?, |sum, s| {
                Ok::<_, Error>(sum + s.rect.pixels()?)
            })?;
        if reserved > MAX_PIXELS {
            return Err(Error::Capacity);
        }
        let i = self
            .surfaces
            .iter()
            .position(|s| !s.live)
            .ok_or(Error::Capacity)?;
        let generation = self.surfaces[i]
            .generation
            .checked_add(1)
            .ok_or(Error::Exhausted)?;
        let order = self.next_order.checked_add(1).ok_or(Error::Exhausted)?;
        self.invalidate();
        self.acknowledged[i] = false;
        self.next_order = order;
        self.surfaces[i] = Surface {
            live: true,
            generation,
            order,
            rect,
            z,
            visible: true,
            ..Surface::default()
        };
        Ok(Handle {
            slot: i as u8,
            generation,
        })
    }
    pub fn remove(&mut self, h: Handle) -> Result<(), Error> {
        self.idle()?;
        let i = self.index(h)?;
        self.invalidate();
        self.damage.add(self.surfaces[i].rect);
        self.acknowledged[i] = false;
        self.surfaces[i].live = false;
        self.surfaces[i].ready = false;
        if self.focus == Some(h) {
            self.focus = None;
        }
        Ok(())
    }
    /// Resize belongs to a staged raster replacement, never an unchecked placement.
    pub fn place(&mut self, h: Handle, x: i16, y: i16, z: u8) -> Result<(), Error> {
        self.idle()?;
        let i = self.index(h)?;
        let old = self.surfaces[i].rect;
        if old.x == x && old.y == y && self.surfaces[i].z == z {
            return Ok(());
        }
        self.invalidate();
        self.damage.add(old);
        self.surfaces[i].rect.x = x;
        self.surfaces[i].rect.y = y;
        self.surfaces[i].z = z;
        self.damage.add(self.surfaces[i].rect);
        Ok(())
    }
    pub fn visible(&mut self, h: Handle, value: bool) -> Result<(), Error> {
        self.idle()?;
        let i = self.index(h)?;
        if self.surfaces[i].visible == value {
            return Ok(());
        }
        self.invalidate();
        self.damage.add(self.surfaces[i].rect);
        self.surfaces[i].visible = value;
        if !value && self.focus == Some(h) {
            self.focus = None;
        }
        Ok(())
    }
    pub fn focus(&mut self, h: Handle) -> Result<(), Error> {
        self.idle()?;
        let i = self.index(h)?;
        if !self.surfaces[i].visible || !self.surfaces[i].ready || !self.acknowledged[i] {
            return Err(Error::Incomplete);
        }
        if self.focus == Some(h) {
            return Ok(());
        }
        self.invalidate();
        if let Some(old) = self.focus {
            let j = self.index(old)?;
            self.damage.add(self.surfaces[j].rect);
        }
        self.focus = Some(h);
        self.damage.add(self.surfaces[i].rect);
        Ok(())
    }
    pub fn cursor(&mut self, position: Option<(u32, u32)>, hover: bool) -> Result<(), Error> {
        self.idle()?;
        if position.is_some_and(|(x, y)| x > i32::MAX as u32 - 10 || y > i32::MAX as u32 - 16) {
            return Err(Error::Bounds);
        }
        if self.cursor == position && self.hover == hover {
            return Ok(());
        }
        self.invalidate();
        for (x, y) in [self.cursor, position].into_iter().flatten() {
            self.damage.add_box(crate::damage::BoxRect {
                left: x as i32,
                top: y as i32,
                right: x as i32 + 10,
                bottom: y as i32 + 16,
            });
        }
        self.cursor = position;
        self.hover = hover;
        Ok(())
    }
    /// Advisory route only after explicit successful scanout acknowledgement.
    /// The semantic owner still validates its exact Show and typed interaction.
    pub fn pointer(&self, x: u32, y: u32) -> Option<Route> {
        // A visible but pending top surface blocks input to covered siblings,
        // matching native input routing even though its raster is not drawn.
        let i = (0..MAX_SURFACES)
            .filter(|&i| {
                let s = self.surfaces[i];
                s.live && s.visible && s.rect.contains(x, y)
            })
            .max_by_key(|&i| (self.surfaces[i].z, self.surfaces[i].order))?;
        if !self.surfaces[i].ready || !self.acknowledged[i] {
            return None;
        }
        let s = self.surfaces[i];
        Some(Route {
            surface: Handle {
                slot: i as u8,
                generation: s.generation,
            },
            revision: s.revision,
            x: (i64::from(x) - i64::from(s.rect.x)) as u16,
            y: (i64::from(y) - i64::from(s.rect.y)) as u16,
        })
    }
    pub fn keyboard(&self) -> Option<(Handle, u64)> {
        let h = self.focus?;
        let i = self.index(h).ok()?;
        let s = self.surfaces[i];
        (self.acknowledged[i] && s.visible && s.ready).then_some((h, s.revision))
    }
}
