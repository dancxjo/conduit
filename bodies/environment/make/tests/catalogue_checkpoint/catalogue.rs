//! Test-local preserved application catalogue validator for the Source-only proof.
//! Provenance:057f48d32c bodies/environment/runtime/src/recording/catalog.rs.
//! Exact schema/limits only; no installed reader or publication/ACK claim.
//! A catalogue confers no authority and does not acknowledge its listed writes.
const MAXIMUM_CHUNK_BYTES: usize = 4096;
use conduit_core as c;
use std::string::String;

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
            hex(&mut content, &bytes);
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

fn hex(output: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(DIGITS[usize::from(byte >> 4)] as char);
        output.push(DIGITS[usize::from(byte & 15)] as char);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference(seed: u8) -> c::BoundedResourceRef {
        c::BoundedResourceRef {
            identity: c::ResourceSemanticIdentity::from_digest([seed; 32]),
            content_profile: "text/json-utf8@1".into(),
            access_class: "resource/snapshot@1".into(),
            extent: c::ResourceExtent {
                bytes: 1024,
                items: Some(1),
            },
            lifetime: c::ResourceLifetime {
                version: c::ResourceVersionIdentity::from_digest([seed; 32]),
                expires_at: None,
            },
        }
    }
    #[test]
    fn full_catalogue_preserves_order_and_requires_its_own_capacity() {
        let refs = (1..=16).map(reference).collect::<std::vec::Vec<_>>();
        let catalog = PreparedRecordingCatalog::prepare(&refs, 17, 32 * 1024).unwrap();
        let mut reader = PreparedRecordingCatalogReader::default();
        assert!(reader.is_empty());
        reader.read(catalog.content()).unwrap();
        assert!(!reader.is_empty());
        assert_eq!(reader.len(), 16);
        for (index, reference) in refs.iter().enumerate() {
            assert_eq!(
                reader.encoded_reference(index),
                Some(reference.encode().unwrap().as_slice())
            );
        }
        assert_eq!(
            PreparedRecordingCatalog::prepare(&refs, 16, 32 * 1024).err(),
            Some(RecordingCatalogRefusal::Capacity)
        );
        assert_eq!(
            PreparedRecordingCatalog::prepare(&refs, 17, catalog.required_content_bytes() - 1)
                .err(),
            Some(RecordingCatalogRefusal::Capacity)
        );
        assert!(PreparedRecordingCatalog::prepare(&[], 17, 32 * 1024).is_err());
        let mut over = refs.clone();
        over.push(reference(17));
        assert!(PreparedRecordingCatalog::prepare(&over, 18, 32 * 1024).is_err());
    }
    #[test]
    fn invalid_or_aliased_catalogues_leave_the_prior_view_unchanged() {
        let first = reference(1);
        let catalog =
            PreparedRecordingCatalog::prepare(core::slice::from_ref(&first), 2, 8192).unwrap();
        let mut reader = PreparedRecordingCatalogReader::default();
        reader.read(catalog.content()).unwrap();
        let encoded = first.encode().unwrap();
        let mut alias = first.clone();
        alias.extent.bytes += 1;
        assert_eq!(
            PreparedRecordingCatalog::prepare(&[first.clone(), alias], 3, 8192).err(),
            Some(RecordingCatalogRefusal::Duplicate)
        );
        for invalid in [
            String::from("{}"),
            catalog.content().replace("references", "foreign"),
            format!("{}x", catalog.content()),
            catalog
                .content()
                .replace("references\":[\"", "references\":[\"ZZ"),
        ] {
            assert!(reader.read(&invalid).is_err());
            assert_eq!(reader.len(), 1);
            assert_eq!(reader.encoded_reference(0), Some(encoded.as_slice()));
        }
        let mut wrong = first;
        wrong.content_profile = "value/text".into();
        assert!(PreparedRecordingCatalog::prepare(&[wrong], 2, 8192).is_err());
    }
}
