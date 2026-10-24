use super::*;
use serde_json::json;
fn proof() -> Value {
    // Contract fixture uses historical measured geometry/addresses and regime.
    // The custody/completion assertions are supplied, not historical Shows.
    json!({"face":{"framebuffer":{"pitch":3200,"height":600}},
      "mapping":{"schema":"conduit.conduitos/loongarch64-framebuffer-mapping@1",
        "cache_policy":"mat-observed-qemu-tcg-shared-ram",
        "store_completion":"volatile-stores-dbar0","exclusive_writer":true,
        "framebuffer_reserved":true,"table_page_custody":"bootloader-reclaimable",
        "physical_memory_type_claimed":false,
        "mapping":{"virtual_start":18446603336450375680_u64,"physical_start":229179392,
          "byte_length":1920000,"checked_pages":469,"descriptors_read":1876,"memory_access_type":2,
          "regime":{"physical_bits":48,"virtual_bits":48,"crmd":176,"pwcl":333763884,
            "pwch":615,"stlbps":12,"pgdh":3942584320_u64,"reduced_address_config":0,
            "direct_windows":[17,0,0,0]}}}})
}
#[test]
fn geometry_page_coverage_and_completion_must_all_agree() {
    assert!(validate(&proof()).is_ok());
    for (field, value) in [
        ("exclusive_writer", json!(false)),
        ("framebuffer_reserved", json!(false)),
        ("table_page_custody", json!("usable")),
        ("store_completion", json!("none")),
        ("physical_memory_type_claimed", json!(true)),
    ] {
        let mut p = proof();
        p["mapping"][field] = value;
        assert!(validate(&p).is_err(), "{field}");
    }
    for (field, value) in [
        ("byte_length", json!(4096)),
        ("checked_pages", json!(468)),
        ("descriptors_read", json!(1877)),
        ("memory_access_type", json!(3)),
        ("physical_start", json!(229179393)),
        ("virtual_start", json!(0x900000000da90000_u64)),
    ] {
        let mut p = proof();
        p["mapping"]["mapping"][field] = value;
        assert!(validate(&p).is_err(), "{field}");
    }
}
#[test]
fn malformed_or_shadowed_paging_evidence_is_not_accepted() {
    for (field, value) in [
        ("physical_bits", json!(64)),
        ("virtual_bits", json!(47)),
        ("reduced_address_config", json!(1)),
        ("crmd", json!(184)),
        ("pwcl", json!(0)),
        ("pwch", json!(0)),
        ("stlbps", json!(14)),
        ("pgdh", json!(3942584321_u64)),
        ("direct_windows", json!([17, 0, 0])),
        ("direct_windows", json!([17, "unknown", 0, 0])),
        (
            "direct_windows",
            json!([17, 17293822569102704641_u64, 0, 0]),
        ),
    ] {
        let mut p = proof();
        p["mapping"]["mapping"]["regime"][field] = value;
        assert!(validate(&p).is_err(), "{field}");
    }
}
#[test]
fn physical_extent_must_fit_the_observed_cpu_width() {
    let mut p = proof();
    p["mapping"]["mapping"]["regime"]["physical_bits"] = json!(32);
    assert!(validate(&p).is_ok());
    p["mapping"]["mapping"]["physical_start"] = json!(0xfffff000_u64);
    assert!(validate(&p).is_err());
    let mut p = proof();
    p["mapping"]["mapping"]["regime"]["pgdh"] = json!(0x100000000_u64);
    p["mapping"]["mapping"]["regime"]["physical_bits"] = json!(32);
    assert!(validate(&p).is_err());
}
