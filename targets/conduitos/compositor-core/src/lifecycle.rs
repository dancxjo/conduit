use crate::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub surface: Handle,
    pub revision: u64,
    pub x: u16,
    pub y: u16,
}
impl Compositor<'_> {
    /// Admission creates metadata only; an initial raster upload makes it visible.
    pub fn admit(&mut self, rect: Rect, z: u8) -> Result<Handle, Error> {
        self.idle()?;
        rect.pixels()?;
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
        self.invalidate();
        self.damage.add(self.surfaces[i].rect);
        self.surfaces[i].visible = value;
        Ok(())
    }
    pub fn focus(&mut self, h: Handle) -> Result<(), Error> {
        self.idle()?;
        let i = self.index(h)?;
        self.invalidate();
        if let Some(old) = self.focus {
            let j = self.index(old)?;
            self.damage.add(self.surfaces[j].rect);
        }
        self.focus = Some(h);
        self.damage.add(self.surfaces[i].rect);
        Ok(())
    }
    pub fn cursor(&mut self, position: Option<(u16, u16)>, hover: bool) -> Result<(), Error> {
        self.idle()?;
        if position.is_some_and(|(x, y)| x > i16::MAX as u16 || y > i16::MAX as u16) {
            return Err(Error::Bounds);
        }
        self.invalidate();
        for (x, y) in [self.cursor, position].into_iter().flatten() {
            self.damage.add(Rect {
                x: x as i16,
                y: y as i16,
                width: 10,
                height: 16,
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
