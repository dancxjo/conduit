//! Validate an independently compiled, immutable implementation artifact.
//! No inherited Root mappings or writable/executable segments are admitted.

pub const USER_TEXT_START: u64 = 0x0040_0000;
// IA-32 Root boots at low physical/virtual addresses. Its linked image spans
// the 4 MiB range, so the domain uses a disjoint virtual range instead.
pub const IA32_USER_TEXT_START: u64 = 0x4000_0000;
pub const MAXIMUM_IMAGE_BYTES: u64 = 128 * 1024;
const PAGE_BYTES: u64 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageRefusal {
    Malformed,
    WrongTarget,
    InvalidMapping,
}

#[derive(Clone, Copy, Debug)]
pub struct ImageSegment<'a> {
    pub address: u64,
    pub bytes: &'a [u8],
    pub executable: bool,
}

impl ImageSegment<'_> {
    pub fn mapped_bytes(&self) -> u64 {
        (self.bytes.len() as u64).div_ceil(PAGE_BYTES) * PAGE_BYTES
    }
}

pub struct DomainImage<'a> {
    pub entry: u64,
    pub segments: [ImageSegment<'a>; 2],
}

impl<'a> DomainImage<'a> {
    pub fn parse(bytes: &'a [u8], machine: u16) -> Result<Self, ImageRefusal> {
        use ImageRefusal as Refusal;
        let class = if machine == 3 { 1 } else { 2 };
        if bytes.get(..4) != Some(b"\x7fELF")
            || bytes.get(4) != Some(&class)
            || bytes.get(5..7) != Some(&[1, 1])
            || field16(bytes, 16)? != 2
        {
            return Err(Refusal::Malformed);
        }
        if field16(bytes, 18)? != machine {
            return Err(Refusal::WrongTarget);
        }
        let (entry, offset, header_size, size_field, count_field) = if class == 1 {
            (
                u64::from(field32(bytes, 24)?),
                u64::from(field32(bytes, 28)?),
                32,
                42,
                44,
            )
        } else {
            (field64(bytes, 24)?, field64(bytes, 32)?, 56, 54, 56)
        };
        if field16(bytes, size_field)? != header_size as u16 || field16(bytes, count_field)? != 2 {
            return Err(Refusal::Malformed);
        }
        let offset = usize::try_from(offset).map_err(|_| Refusal::Malformed)?;
        let end = offset
            .checked_add(2 * header_size)
            .ok_or(Refusal::Malformed)?;
        let headers = bytes.get(offset..end).ok_or(Refusal::Malformed)?;
        let text_start = if machine == 3 {
            IA32_USER_TEXT_START
        } else {
            USER_TEXT_START
        };
        let first = segment(bytes, &headers[..header_size], class, text_start)?;
        let second = segment(bytes, &headers[header_size..], class, text_start)?;
        if !first.executable
            || second.executable
            || entry < first.address
            || entry >= first.address + first.bytes.len() as u64
            || first.address + first.mapped_bytes() > second.address
        {
            return Err(Refusal::InvalidMapping);
        }
        Ok(Self {
            entry,
            segments: [first, second],
        })
    }
}

fn segment<'a>(
    bytes: &'a [u8],
    header: &[u8],
    class: u8,
    text_start: u64,
) -> Result<ImageSegment<'a>, ImageRefusal> {
    use ImageRefusal as Refusal;
    let (flags, offset, address, length, memory_length, alignment) = if class == 1 {
        (
            field32(header, 24)?,
            u64::from(field32(header, 4)?),
            u64::from(field32(header, 8)?),
            u64::from(field32(header, 16)?),
            u64::from(field32(header, 20)?),
            u64::from(field32(header, 28)?),
        )
    } else {
        (
            field32(header, 4)?,
            field64(header, 8)?,
            field64(header, 16)?,
            field64(header, 32)?,
            field64(header, 40)?,
            field64(header, 48)?,
        )
    };
    if field32(header, 0)? != 1
        || !matches!(flags, 4 | 5)
        || length == 0
        || length > MAXIMUM_IMAGE_BYTES
        || memory_length != length
        || alignment != PAGE_BYTES
        || address % PAGE_BYTES != 0
        || address < text_start
        || address
            .checked_add(length.div_ceil(PAGE_BYTES) * PAGE_BYTES)
            .is_none_or(|end| end > text_start + MAXIMUM_IMAGE_BYTES)
    {
        return Err(Refusal::InvalidMapping);
    }
    let start = usize::try_from(offset).map_err(|_| Refusal::Malformed)?;
    let length = usize::try_from(length).map_err(|_| Refusal::Malformed)?;
    let end = start.checked_add(length).ok_or(Refusal::Malformed)?;
    Ok(ImageSegment {
        address,
        bytes: bytes.get(start..end).ok_or(Refusal::Malformed)?,
        executable: flags == 5,
    })
}

fn field16(bytes: &[u8], offset: usize) -> Result<u16, ImageRefusal> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset + 2)
            .ok_or(ImageRefusal::Malformed)?
            .try_into()
            .map_err(|_| ImageRefusal::Malformed)?,
    ))
}
fn field32(bytes: &[u8], offset: usize) -> Result<u32, ImageRefusal> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .ok_or(ImageRefusal::Malformed)?
            .try_into()
            .map_err(|_| ImageRefusal::Malformed)?,
    ))
}
fn field64(bytes: &[u8], offset: usize) -> Result<u64, ImageRefusal> {
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..offset + 8)
            .ok_or(ImageRefusal::Malformed)?
            .try_into()
            .map_err(|_| ImageRefusal::Malformed)?,
    ))
}

#[cfg(test)]
#[path = "domain_image/tests.rs"]
mod tests;
