use crate::*;
impl Compositor<'_> {
    /// One transaction replaces exactly one raster. Other committed rasters are
    /// copied to scratch in finite steps; their current arena remains untouched.
    pub fn begin_upload(&mut self, h: Handle, revision: u64, rect: Rect) -> Result<(), Error> {
        self.idle()?;
        let slot = self.index(h)?;
        rect.pixels()?;
        if revision <= self.surfaces[slot].revision {
            return Err(Error::Stale);
        }
        let mut surfaces = self.surfaces;
        surfaces[slot].rect = rect;
        surfaces[slot].revision = revision;
        surfaces[slot].ready = true;
        let mut offset = 0;
        for s in &mut surfaces {
            if s.live && s.ready {
                let n = s.rect.pixels()?;
                s.offset = offset;
                offset = offset.checked_add(n).ok_or(Error::Capacity)?;
            }
        }
        if offset > MAX_PIXELS {
            return Err(Error::Capacity);
        }
        self.invalidate();
        self.upload = Some(Upload {
            slot,
            surfaces,
            copy_slot: 0,
            copied: 0,
            written: 0,
        });
        Ok(())
    }
    /// Returns true once unchanged surfaces have been preserved. Work <= limit.
    pub fn prepare_upload(&mut self, limit: usize) -> Result<bool, Error> {
        if limit == 0 || limit > MAX_CHUNK {
            return Err(Error::Bounds);
        }
        let u = self.upload.as_mut().ok_or(Error::Order)?;
        let mut remaining = limit;
        while u.copy_slot < MAX_SURFACES && remaining > 0 {
            let i = u.copy_slot;
            let s = u.surfaces[i];
            if i == u.slot || !s.live || !s.ready {
                u.copy_slot += 1;
                u.copied = 0;
                continue;
            }
            let count = s.rect.pixels()?;
            let n = (count - u.copied).min(remaining);
            let src = self.surfaces[i].offset + u.copied;
            let dst = s.offset + u.copied;
            self.scratch[dst..dst + n].copy_from_slice(&self.pixels[src..src + n]);
            u.copied += n;
            remaining -= n;
            if u.copied == count {
                u.copy_slot += 1;
                u.copied = 0;
            }
        }
        Ok(u.copy_slot == MAX_SURFACES)
    }
    pub fn upload_chunk(&mut self, offset: usize, pixels: &[u32]) -> Result<(), Error> {
        let u = self.upload.as_mut().ok_or(Error::Order)?;
        if u.copy_slot != MAX_SURFACES {
            return Err(Error::Order);
        }
        if offset != u.written {
            return Err(Error::Order);
        }
        let s = u.surfaces[u.slot];
        let count = s.rect.pixels()?;
        if pixels.is_empty() || pixels.len() > MAX_CHUNK || pixels.len() > count - u.written {
            return Err(Error::Bounds);
        }
        let start = s.offset + u.written;
        self.scratch[start..start + pixels.len()].copy_from_slice(pixels);
        u.written += pixels.len();
        Ok(())
    }
    pub fn commit_upload(&mut self) -> Result<(), Error> {
        let u = self.upload.ok_or(Error::Order)?;
        if u.copy_slot != MAX_SURFACES || u.written != u.surfaces[u.slot].rect.pixels()? {
            return Err(Error::Incomplete);
        }
        self.damage.add(self.surfaces[u.slot].rect);
        self.damage.add(u.surfaces[u.slot].rect);
        core::mem::swap(&mut self.pixels, &mut self.scratch);
        self.surfaces = u.surfaces;
        self.upload = None;
        Ok(())
    }
    pub fn abort_upload(&mut self) -> Result<(), Error> {
        let u = self.upload.take().ok_or(Error::Order)?;
        self.invalidate();
        self.damage.add(self.surfaces[u.slot].rect);
        Ok(())
    }
}
