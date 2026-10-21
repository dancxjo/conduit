use super::Malformed;
pub(crate) struct Writer<'a> {
    pub bytes: &'a mut [u8],
    pub at: usize,
}
impl Writer<'_> {
    pub fn raw(&mut self, bytes: &[u8]) -> Result<(), Malformed> {
        let end = self.at.checked_add(bytes.len()).ok_or(Malformed)?;
        self.bytes
            .get_mut(self.at..end)
            .ok_or(Malformed)?
            .copy_from_slice(bytes);
        self.at = end;
        Ok(())
    }
    pub fn u8(&mut self, n: u8) -> Result<(), Malformed> {
        self.raw(&[n])
    }
    pub fn u16(&mut self, n: u16) -> Result<(), Malformed> {
        self.raw(&n.to_le_bytes())
    }
    pub fn u32(&mut self, n: u32) -> Result<(), Malformed> {
        self.raw(&n.to_le_bytes())
    }
    pub fn u64(&mut self, n: u64) -> Result<(), Malformed> {
        self.raw(&n.to_le_bytes())
    }
}
pub(crate) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub at: usize,
}
impl<'a> Reader<'a> {
    pub fn raw(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        let end = self.at.checked_add(n).ok_or(Malformed)?;
        let bytes = self.bytes.get(self.at..end).ok_or(Malformed)?;
        self.at = end;
        Ok(bytes)
    }
    pub fn u8(&mut self) -> Result<u8, Malformed> {
        Ok(self.raw(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, Malformed> {
        Ok(u16::from_le_bytes(
            self.raw(2)?.try_into().map_err(|_| Malformed)?,
        ))
    }
    pub fn u32(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().map_err(|_| Malformed)?,
        ))
    }
    pub fn u64(&mut self) -> Result<u64, Malformed> {
        Ok(u64::from_le_bytes(
            self.raw(8)?.try_into().map_err(|_| Malformed)?,
        ))
    }
    pub fn flag(&mut self) -> Result<bool, Malformed> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Malformed),
        }
    }
    pub fn finish(self) -> Result<(), Malformed> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(Malformed)
        }
    }
}
