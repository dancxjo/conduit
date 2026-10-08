use conduit_plot::syntax_source_document_identity;
use sha2::{Digest, Sha256};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn literal_source_identity_retains_existing_hash_with_constant_heap_storage() {
    let large = "type Sample = U8\n".repeat(16384);
    for source in [
        "",
        "type Sample = U8\n",
        "type Sample = U8\r\n",
        "α\0β\n",
        large.as_str(),
    ] {
        let original = format!(
            "{:x}",
            Sha256::digest(format!("canonical-source:{source}").as_bytes())
        );
        let (identity, observed) =
            allocation_probe::observe(|| syntax_source_document_identity(source));
        assert_eq!(identity.as_str(), original);
        assert_eq!(observed.allocations, 1);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.peak_bytes, 64);
    }
}
