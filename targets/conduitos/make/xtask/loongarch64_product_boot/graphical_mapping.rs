//! Bounded collector validation of the scanout mapping evidence contract.
use serde_json::Value;
pub(super) fn validate(proof: &Value) -> Result<(), &'static str> {
    let evidence = &proof["mapping"];
    let m = &evidence["mapping"];
    let f = &proof["face"]["framebuffer"];
    let bytes = f["pitch"]
        .as_u64()
        .and_then(|pitch| {
            f["height"]
                .as_u64()
                .and_then(|height| pitch.checked_mul(height))
        })
        .ok_or("framebuffer extent missing")?;
    let pages = bytes.div_ceil(4096);
    let physical_bits = m["regime"]["physical_bits"]
        .as_u64()
        .filter(|v| (32..=48).contains(v))
        .ok_or("physical address width missing or unsupported")?;
    let physical_limit = 1_u64 << physical_bits;
    let virtual_start = m["virtual_start"]
        .as_u64()
        .ok_or("virtual address missing")?;
    let windows = m["regime"]["direct_windows"]
        .as_array()
        .filter(|v| v.len() == 4)
        .ok_or("direct window inventory missing")?;
    if windows.iter().any(|v| {
        v.as_u64()
            .is_none_or(|v| v & 1 != 0 && v >> 60 == virtual_start >> 60)
    }) {
        return Err("direct window shadows framebuffer page walk");
    }
    let descriptors = m["descriptors_read"].as_u64().unwrap_or(0);
    if virtual_start >> 47 != (1 << 17) - 1
        || virtual_start
            .checked_add(bytes)
            .is_none_or(|end| end == 0 || (end - 1) >> 47 != (1 << 17) - 1)
        || evidence["schema"] != "conduit.conduitos/loongarch64-framebuffer-mapping@1"
        || evidence["cache_policy"] != "mat-observed-qemu-tcg-shared-ram"
        || evidence["store_completion"] != "volatile-stores-dbar0"
        || evidence["exclusive_writer"] != true
        || evidence["framebuffer_reserved"] != true
        || evidence["table_page_custody"] != "bootloader-reclaimable"
        || evidence["physical_memory_type_claimed"] != false
        || bytes == 0
        || bytes > 8 * 1024 * 1024
        || m["byte_length"] != bytes
        || m["checked_pages"] != pages
        || !(pages..=pages * 4).contains(&descriptors)
        || m["memory_access_type"].as_u64().is_none_or(|v| v > 2)
        || m["regime"]["virtual_bits"] != 48
        || m["regime"]["reduced_address_config"] != 0
        || m["regime"]["crmd"]
            .as_u64()
            .is_none_or(|v| v & 0x1fb != 0xb0)
        || m["regime"]["pwcl"] != 333763884_u64
        || m["regime"]["pwch"] != 615_u64
        || m["regime"]["stlbps"] != 12
        || m["regime"]["pgdh"]
            .as_u64()
            .is_none_or(|v| v == 0 || !v.is_multiple_of(4096) || v >= physical_limit)
        || m["physical_start"].as_u64().is_none_or(|v| {
            v == 0
                || !v.is_multiple_of(4096)
                || v.checked_add(bytes).is_none_or(|end| end > physical_limit)
        })
        || m["virtual_start"]
            .as_u64()
            .is_none_or(|v| !v.is_multiple_of(4096))
    {
        return Err("mapping or completion evidence disagrees with admitted scanout");
    }
    Ok(())
}

#[cfg(test)]
#[path = "graphical_mapping_tests.rs"]
mod tests;
