//! Derive the immutable canonical result once on the fabrication host.
extern crate alloc;
#[path = "src/append_type_definition.rs"]
mod append_type_definition;
use conduit_core::{kind_id, StructuredInfoType, StructuredInfoValue};
use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=src/append_type_definition.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let result = append_type_definition::definition();
    let index = StructuredInfoType::leaf(kind_id("value/u64")).unwrap();
    let unit = StructuredInfoType::leaf(kind_id("value/empty")).unwrap();
    let bound = |kind: &conduit_core::KindId| match kind.as_str() {
        "value/u64" => Some(8),
        "value/empty" => Some(0),
        _ => None,
    };
    let maximum = result
        .canonical_value_upper_bound(bound)
        .expect("finite result");
    let index_maximum = index.canonical_value_upper_bound(bound).unwrap();
    assert!(maximum <= 512, "finite native result ceiling");
    fs::write(
        out.join("append-result.type"),
        result.canonical_bytes().unwrap(),
    )
    .unwrap();
    fs::write(
        out.join("append-index.type"),
        index.canonical_bytes().unwrap(),
    )
    .unwrap();
    fs::write(
        out.join("append-unit.value"),
        StructuredInfoValue::leaf(unit, Vec::new())
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    fs::write(out.join("append-profile.rs"), format!(
        "pub const APPEND_RESULT_BYTES:u32={maximum};\npub const APPEND_INDEX_BYTES:usize={index_maximum};\npub const APPEND_RESULT_PROFILE:&str={:?};\n", result.profile().unwrap().value_kind().as_str())).unwrap();
}
