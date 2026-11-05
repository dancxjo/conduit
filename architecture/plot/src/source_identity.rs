//! Exact authored-byte identity without lexing, parsing or catalogue access.
use crate::prelude::String;
use conduit_core::SourceDocumentId;
use sha2::{Digest, Sha256};

/// The canonical parser's Source identity, including invalid/unfinished Source.
/// Whitespace, comments and UTF-8 bytes remain significant. Only the returned
/// 64-byte identity is allocated; Source bytes are borrowed and hashed in place.
/// This performs no checking, byte/work admission, Source access authorization,
/// catalogue lookup or construction of an executable checked owner.
pub fn canonical_source_document_id(source: &str) -> SourceDocumentId {
    let mut hash = Sha256::new();
    hash.update(b"canonical-source:");
    hash.update(source.as_bytes());
    let mut encoded = String::with_capacity(64);
    for byte in hash.finalize() {
        encoded.push(crate::hex(byte >> 4));
        encoded.push(crate::hex(byte & 0x0f));
    }
    SourceDocumentId::from(encoded)
}
