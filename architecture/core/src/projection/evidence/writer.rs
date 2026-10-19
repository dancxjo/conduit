use super::ProjectionEvidenceRefusal as E;
enum Sink<'a> {
    Measure,
    Write(&'a mut [u8]),
    Compare(&'a [u8]),
}
/// Bounded canonical little-endian primitives for a reviewed domain codec.
/// Text is nonempty UTF-8, at most 192 bytes. Byte strings have u32 lengths.
pub struct ProjectionEvidenceWriter<'a> {
    sink: Sink<'a>,
    position: usize,
    limit: usize,
}
impl<'a> ProjectionEvidenceWriter<'a> {
    pub(super) fn measuring(limit: usize) -> Self {
        Self {
            sink: Sink::Measure,
            position: 0,
            limit,
        }
    }
    pub(super) fn writing(bytes: &'a mut [u8]) -> Self {
        let limit = bytes.len();
        Self {
            sink: Sink::Write(bytes),
            position: 0,
            limit,
        }
    }
    pub(super) fn comparing(bytes: &'a [u8]) -> Self {
        Self {
            sink: Sink::Compare(bytes),
            position: 0,
            limit: bytes.len(),
        }
    }
    pub fn len(&self) -> usize {
        self.position
    }
    pub fn is_empty(&self) -> bool {
        self.position == 0
    }
    fn reserve(&mut self, length: usize) -> Result<usize, E> {
        let start = self.position;
        self.position = self
            .position
            .checked_add(length)
            .filter(|n| *n <= self.limit)
            .ok_or(E::Extent)?;
        Ok(start)
    }
    pub fn raw(&mut self, bytes: &[u8]) -> Result<(), E> {
        let start = self.reserve(bytes.len())?;
        let end = self.position;
        match &mut self.sink {
            Sink::Measure => (),
            Sink::Write(out) => out[start..end].copy_from_slice(bytes),
            Sink::Compare(input) => {
                if input[start..end] != *bytes {
                    return Err(E::EvidenceMismatch);
                }
            }
        }
        Ok(())
    }
    pub fn tag(&mut self, value: u8) -> Result<(), E> {
        self.raw(&[value])
    }
    pub fn u32(&mut self, value: u32) -> Result<(), E> {
        self.raw(&value.to_le_bytes())
    }
    pub fn u64(&mut self, value: u64) -> Result<(), E> {
        self.raw(&value.to_le_bytes())
    }
    pub fn i64(&mut self, value: i64) -> Result<(), E> {
        self.raw(&value.to_le_bytes())
    }
    pub fn bytes(&mut self, value: &[u8]) -> Result<(), E> {
        self.u32(u32::try_from(value.len()).map_err(|_| E::Extent)?)?;
        self.raw(value)
    }
    pub fn text(&mut self, value: &str) -> Result<(), E> {
        if value.is_empty() || value.len() > 192 {
            return Err(E::Extent);
        }
        self.raw(&(value.len() as u16).to_le_bytes())?;
        self.raw(value.as_bytes())
    }
    pub(super) fn optional_text(&mut self, text: Option<&str>) -> Result<(), E> {
        match text {
            None => self.tag(0),
            Some(s) => {
                self.tag(1)?;
                self.text(s)
            }
        }
    }
    pub(super) fn count(&mut self, count: usize) -> Result<(), E> {
        self.tag(u8::try_from(count).map_err(|_| E::Extent)?)
    }
    pub(super) fn field<F>(&mut self, limit: usize, encode: F) -> Result<(), E>
    where
        F: Fn(&mut ProjectionEvidenceWriter<'_>) -> Result<(), E>,
    {
        let mut size = ProjectionEvidenceWriter::measuring(limit);
        encode(&mut size)?;
        let length = size.len();
        self.u32(u32::try_from(length).map_err(|_| E::Extent)?)?;
        let start = self.reserve(length)?;
        let end = self.position;
        let actual = match &mut self.sink {
            Sink::Measure => return Ok(()),
            Sink::Write(out) => {
                let mut child = ProjectionEvidenceWriter::writing(&mut out[start..end]);
                encode(&mut child)?;
                child.len()
            }
            Sink::Compare(input) => {
                let mut child = ProjectionEvidenceWriter::comparing(&input[start..end]);
                encode(&mut child)?;
                child.len()
            }
        };
        if actual != length {
            return Err(E::DomainEncoding);
        }
        Ok(())
    }
}
