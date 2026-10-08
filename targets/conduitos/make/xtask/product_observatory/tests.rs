use super::*;

fn fixture() -> (String, Value) {
    let identities = conduitos::identity::BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = conduitos::offer::HostOffer::new(
        &identities,
        "build",
        conduitos::offer::CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        512 * 1024,
    );
    let prepared = conduitos::dual_region_plan::prepare(&identities, &offer, "build").unwrap();
    let record = conduitos::boot::BootRecord {
        firmware: conduitos::boot::Firmware::X86Bios,
        timestamp: 1,
        hhdm_offset: 2,
        rsdp_address: None,
        image_physical_start: 3,
        image_length: 4,
        memory_region_count: 5,
        artifact_count: 0,
        framebuffer_count: 0,
        command_line_bytes: 0,
        runtime_arena: conduitos::boot::RuntimeArena {
            physical_start: 6,
            length: 512 * 1024,
        },
    };
    let export = conduitos::observatory::prepare_export(
        &record,
        &identities,
        &offer,
        &prepared,
        "build",
        "image",
        None,
    )
    .unwrap();
    let product = serde_json::json!({
        "host_id": prepared.advertisement.host_id, "boot_id": prepared.advertisement.boot_id,
        "offer_generation": 1, "build_id": "build", "image_id": "image",
        "ordinary_source_document_id": prepared.source_document_id,
        "ordinary_checked_plot_id": prepared.checked_plot_id,
        "ordinary_expanded_plot_id": prepared.expanded_plot_id,
        "ordinary_plan_id": prepared.plan_id, "ordinary_play_id": prepared.active_play.active_play_id,
    });
    (
        format!(
            "{}{}\n",
            conduitos::observatory::EXPORT_PREFIX,
            std::str::from_utf8(export.as_bytes()).unwrap()
        ),
        product,
    )
}

#[test]
fn current_sealed_export_must_match_every_product_identity() {
    let (text, product) = fixture();
    assert!(capture(&text, &product).unwrap().is_some());
    for field in [
        "host_id",
        "boot_id",
        "build_id",
        "image_id",
        "ordinary_source_document_id",
        "ordinary_checked_plot_id",
        "ordinary_expanded_plot_id",
        "ordinary_plan_id",
        "ordinary_play_id",
    ] {
        let mut changed = product.clone();
        changed[field] = "stale".into();
        assert!(capture(&text, &changed).is_err(), "{field}");
        changed[field] = Value::Null;
        assert!(capture(&text, &changed).is_err(), "{field}");
    }
    let mut changed = product.clone();
    changed["offer_generation"] = 2.into();
    assert!(capture(&text, &changed).is_err());
}

#[test]
fn truncated_duplicate_oversized_or_unsealed_exports_cannot_be_promoted() {
    let (text, product) = fixture();
    assert!(capture("", &product).unwrap().is_none());
    assert!(capture(text.trim_end(), &product).unwrap().is_none());
    assert!(capture(&(text.clone() + &text), &product).is_err());
    let mut snapshot = capture(&text, &product).unwrap().unwrap();
    snapshot["plans"][0]["plan_id"] = "unsealed".into();
    let changed = format!("{}{}\n", conduitos::observatory::EXPORT_PREFIX, snapshot);
    assert!(capture(&changed, &product).is_err());
    let oversized = format!(
        "{}{{\"extra\":\"{}\"}}\n",
        conduitos::observatory::EXPORT_PREFIX,
        "x".repeat(conduitos::observatory::MAX_EXPORT_BYTES)
    );
    assert!(capture(&oversized, &product).is_err());
}
