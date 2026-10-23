//! Ordered selected generations in one ordinary immutable JSON snapshot.
//! A catalog confers no authority and does not acknowledge its listed writes.
use super::MAXIMUM_CHUNK_BYTES;
use alloc::string::String;
use conduit_core as c;

pub const MAXIMUM_CATALOG_GENERATIONS: usize = 16;
// The pinned resource-ref v1 encoding with these two fixed profile names,
// Some(1) item extent and no expiry is exactly 122 bytes. Core validates it.
const REFERENCE_BYTES: usize = 122;
const PREFIX: &str = "{\"schema\":\"conduit.environment/recording-catalog@1\",\"references\":[";
const SUFFIX: &str = "]}";
#[derive(Debug, PartialEq, Eq)]
pub enum RecordingCatalogRefusal {
    Capacity,
    Encoding,
    Reference(c::ResourceReferenceRefusal),
    Profile,
    Duplicate,
}

pub struct PreparedRecordingCatalog {
    content: String,
    content_bytes: u64,
    records: usize,
}
impl PreparedRecordingCatalog {
    /// These supplied ceilings are constraints, not reservations. Root must
    /// independently reserve all chunks plus this extra catalog record, select
    /// its exact reference and publish through the ordinary snapshot Host Call.
    pub fn prepare(
        references: &[c::BoundedResourceRef],
        maximum_records: usize,
        maximum_content_bytes: u64,
    ) -> Result<Self, RecordingCatalogRefusal> {
        use RecordingCatalogRefusal as E;
        if references.is_empty()
            || references.len() > MAXIMUM_CATALOG_GENERATIONS
            || references.len() + 1 > maximum_records
        {
            return Err(E::Capacity);
        }
        let mut content = String::with_capacity(MAXIMUM_CHUNK_BYTES);
        content.push_str(PREFIX);
        let mut content_bytes = 0_u64;
        for (index, reference) in references.iter().enumerate() {
            let bytes = reference.encode().map_err(E::Reference)?;
            validate(&bytes)?;
            content_bytes = content_bytes
                .checked_add(reference.extent.bytes)
                .ok_or(E::Capacity)?;
            if index != 0 {
                content.push(',');
            }
            content.push('"');
            super::hex(&mut content, &bytes);
            content.push('"');
        }
        content.push_str(SUFFIX);
        if content.len() > MAXIMUM_CHUNK_BYTES {
            return Err(E::Capacity);
        }
        PreparedRecordingCatalogReader::default().read(&content)?;
        content_bytes = content_bytes
            .checked_add(content.len() as u64)
            .filter(|bytes| *bytes <= maximum_content_bytes)
            .ok_or(E::Capacity)?;
        Ok(Self {
            content,
            content_bytes,
            records: references.len() + 1,
        })
    }
    pub fn content(&self) -> &str {
        &self.content
    }
    pub fn required_records(&self) -> usize {
        self.records
    }
    pub fn required_content_bytes(&self) -> u64 {
        self.content_bytes
    }
}

/// Complete validation precedes replacement. Decode typed owned references only
/// during subsequent preparation; reading this finite catalog allocates nothing.
pub struct PreparedRecordingCatalogReader {
    references: [[u8; REFERENCE_BYTES]; MAXIMUM_CATALOG_GENERATIONS],
    count: usize,
}
impl Default for PreparedRecordingCatalogReader {
    fn default() -> Self {
        Self {
            references: [[0; REFERENCE_BYTES]; MAXIMUM_CATALOG_GENERATIONS],
            count: 0,
        }
    }
}
impl PreparedRecordingCatalogReader {
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn encoded_reference(&self, index: usize) -> Option<&[u8]> {
        (index < self.count).then(|| self.references[index].as_slice())
    }
    pub fn read(&mut self, content: &str) -> Result<(), RecordingCatalogRefusal> {
        use RecordingCatalogRefusal as E;
        if content.len() > MAXIMUM_CHUNK_BYTES {
            return Err(E::Capacity);
        }
        let body = content
            .strip_prefix(PREFIX)
            .and_then(|s| s.strip_suffix(SUFFIX))
            .ok_or(E::Encoding)?;
        let mut candidate = [[0; REFERENCE_BYTES]; MAXIMUM_CATALOG_GENERATIONS];
        let mut count = 0;
        for field in body.split(',') {
            if count == MAXIMUM_CATALOG_GENERATIONS {
                return Err(E::Capacity);
            }
            let encoded = field
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .ok_or(E::Encoding)?;
            if encoded.len() != REFERENCE_BYTES * 2 {
                return Err(E::Encoding);
            }
            for (slot, pair) in candidate[count]
                .iter_mut()
                .zip(encoded.as_bytes().as_chunks::<2>().0)
            {
                let digit = |v| match v {
                    b'0'..=b'9' => Ok(v - b'0'),
                    b'a'..=b'f' => Ok(v - b'a' + 10),
                    _ => Err(E::Encoding),
                };
                *slot = (digit(pair[0])? << 4) | digit(pair[1])?;
            }
            let current = validate(&candidate[count])?;
            for previous in &candidate[..count] {
                let previous = validate(previous)?;
                if previous.identity == current.identity && previous.version == current.version {
                    return Err(E::Duplicate);
                }
            }
            count += 1;
        }
        self.references = candidate;
        self.count = count;
        Ok(())
    }
}
fn validate(bytes: &[u8]) -> Result<c::EncodedResourceReference<'_>, RecordingCatalogRefusal> {
    use RecordingCatalogRefusal as E;
    let reference = c::BoundedResourceRef::validate_encoded(bytes).map_err(E::Reference)?;
    if bytes.first() != Some(&1)
        || bytes.len() != REFERENCE_BYTES
        || reference.content_profile != "text/json-utf8@1"
        || reference.access_class != "resource/snapshot@1"
        || reference.extent.items != Some(1)
        || reference.extent.bytes == 0
        || reference.extent.bytes > MAXIMUM_CHUNK_BYTES as u64
        || reference.has_expiry
    {
        return Err(E::Profile);
    }
    Ok(reference)
}
