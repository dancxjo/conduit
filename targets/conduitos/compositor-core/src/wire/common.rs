use super::*;
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        let end = self.position.checked_add(n).ok_or(CodecError::Length)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(CodecError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }
    pub fn done(&self) -> Result<(), CodecError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(CodecError::Length)
        }
    }
    pub fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CodecError::Value),
        }
    }
    pub fn u16(&mut self) -> Result<u16, CodecError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u32(&mut self) -> Result<u32, CodecError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64(&mut self) -> Result<u64, CodecError> {
        let b = self.take(8)?;
        let mut v = [0; 8];
        v.copy_from_slice(b);
        Ok(u64::from_le_bytes(v))
    }
    pub fn handle(&mut self) -> Result<Handle, CodecError> {
        let h = Handle {
            slot: self.u8()?,
            generation: self.u64()?,
        };
        valid_handle(h)?;
        Ok(h)
    }
    pub fn rect(&mut self) -> Result<Rect, CodecError> {
        let r = Rect {
            x: self.u16()? as i16,
            y: self.u16()? as i16,
            width: self.u16()?,
            height: self.u16()?,
        };
        valid_rect(r)?;
        Ok(r)
    }
}
pub(super) struct Writer<'a> {
    bytes: &'a mut [u8],
    pub position: usize,
}
impl<'a> Writer<'a> {
    pub fn new(bytes: &'a mut [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    pub fn put(&mut self, b: &[u8]) -> Result<(), CodecError> {
        let end = self
            .position
            .checked_add(b.len())
            .ok_or(CodecError::Length)?;
        self.bytes
            .get_mut(self.position..end)
            .ok_or(CodecError::Truncated)?
            .copy_from_slice(b);
        self.position = end;
        Ok(())
    }
    pub fn u8(&mut self, v: u8) -> Result<(), CodecError> {
        self.put(&[v])
    }
    pub fn bool(&mut self, v: bool) -> Result<(), CodecError> {
        self.u8(u8::from(v))
    }
    pub fn u16(&mut self, v: u16) -> Result<(), CodecError> {
        self.put(&v.to_le_bytes())
    }
    pub fn u32(&mut self, v: u32) -> Result<(), CodecError> {
        self.put(&v.to_le_bytes())
    }
    pub fn u64(&mut self, v: u64) -> Result<(), CodecError> {
        self.put(&v.to_le_bytes())
    }
    pub fn handle(&mut self, h: Handle) -> Result<(), CodecError> {
        valid_handle(h)?;
        self.u8(h.slot)?;
        self.u64(h.generation)
    }
    pub fn rect(&mut self, r: Rect) -> Result<(), CodecError> {
        valid_rect(r)?;
        self.u16(r.x as u16)?;
        self.u16(r.y as u16)?;
        self.u16(r.width)?;
        self.u16(r.height)
    }
}
pub(super) fn header(bytes: &[u8]) -> Result<(Envelope, u16, Reader<'_>), CodecError> {
    if bytes.len() > EXCHANGE_BYTES {
        return Err(CodecError::Length);
    }
    let mut r = Reader::new(bytes);
    if r.take(4)? != b"CCMP" {
        return Err(CodecError::Version);
    }
    if r.u16()? != 2 {
        return Err(CodecError::Version);
    }
    let operation = r.u16()?;
    if u64::from(r.u32()?) != bytes.len() as u64 {
        return Err(CodecError::Length);
    }
    if r.u32()? != 0 {
        return Err(CodecError::Reserved);
    }
    let e = Envelope {
        instance: r.u64()?,
        request: r.u64()?,
    };
    nonzero(e.instance)?;
    nonzero(e.request)?;
    Ok((e, operation, r))
}
pub(super) fn begin(
    bytes: &mut [u8],
    e: Envelope,
    op: u16,
    length: usize,
) -> Result<Writer<'_>, CodecError> {
    if length > EXCHANGE_BYTES {
        return Err(CodecError::Length);
    }
    nonzero(e.instance)?;
    nonzero(e.request)?;
    let mut w = Writer::new(bytes.get_mut(..length).ok_or(CodecError::Truncated)?);
    w.put(b"CCMP")?;
    w.u16(2)?;
    w.u16(op)?;
    w.u32(length as u32)?;
    w.u32(0)?;
    w.u64(e.instance)?;
    w.u64(e.request)?;
    Ok(w)
}
