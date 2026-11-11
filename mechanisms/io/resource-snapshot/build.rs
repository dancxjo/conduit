//! Derive the immutable canonical result once on the fabrication host.
extern crate alloc;
#[path = "src/append_type_definition.rs"]
mod append_type_definition;
#[path = "src/read_type_definition.rs"]
mod read_type_definition;
use conduit_core::{kind_id, StructuredInfoType, StructuredInfoValue};
use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-changed=src/append_type_definition.rs");
    println!("cargo:rerun-if-changed=src/read_type_definition.rs");
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
    build_read_result(&out);
}

fn build_read_result(out: &std::path::Path) {
    let bound = |kind: &conduit_core::KindId| match kind.as_str() {
        "value/u64" => Some(8),
        "value/empty" => Some(0),
        "value/bytes" => Some(200),
        _ => None,
    };
    let definitions = [
        (
            "read-result.type",
            "READ_RESULT_BYTES",
            read_type_definition::definition(),
        ),
        (
            "read-recorded.type",
            "READ_RECORDED_BYTES",
            read_type_definition::recorded_definition(),
        ),
        (
            "read-gap.type",
            "READ_GAP_BYTES",
            read_type_definition::gap_definition(),
        ),
        (
            "read-gap-reason.type",
            "READ_GAP_REASON_BYTES",
            read_type_definition::gap_reason_definition(),
        ),
        (
            "read-count.type",
            "READ_COUNT_BYTES",
            read_type_definition::count_definition(),
        ),
        (
            "read-bytes.type",
            "READ_BYTES_BYTES",
            read_type_definition::bytes_definition(),
        ),
    ];
    let mut profile = String::new();
    for (path, name, definition) in definitions {
        let maximum = definition
            .canonical_value_upper_bound(bound)
            .expect("finite read Type");
        assert!(
            maximum <= 4096,
            "read Type must remain within its reviewed finite envelope"
        );
        fs::write(out.join(path), definition.canonical_bytes().unwrap()).unwrap();
        profile.push_str(&format!("pub const {name}:usize={maximum};\n"));
    }
    profile.push_str(&format!(
        "pub const READ_RESULT_PROFILE:&str={:?};\n",
        read_type_definition::definition()
            .profile()
            .unwrap()
            .value_kind()
            .as_str()
    ));
    fs::write(out.join("read-profile.rs"), profile).unwrap();
    let unit =
        StructuredInfoValue::leaf(read_type_definition::unit_definition(), Vec::new()).unwrap();
    fs::write(out.join("read-unit.value"), unit.canonical_bytes().unwrap()).unwrap();
    let cancelled =
        StructuredInfoValue::variant(read_type_definition::definition(), "cancelled", unit)
            .expect("exact linked cancellation");
    fs::write(
        out.join("read-cancelled.value"),
        cancelled.canonical_bytes().unwrap(),
    )
    .unwrap();
}
