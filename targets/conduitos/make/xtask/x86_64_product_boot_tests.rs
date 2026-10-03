use super::{validate, GuestBootSign};
use serde_json::{json, Value};

fn arrival() -> (GuestBootSign, Value) {
    let boot = GuestBootSign {
        schema: "conduit.conduitos.boot-sign/v1".into(),
        status: "accepted".into(),
        arch: "x86_64".into(),
        firmware: "x86-bios".into(),
        profile_id: "profile/exact".into(),
        build_id: "build/exact".into(),
        image_binding: "image/exact".into(),
        offer_generation: 1,
        limine: "12.5.2".into(),
        qemu_profile: "test".into(),
        host_id: "host/fresh".into(),
        boot_id: "boot/fresh".into(),
        memory_regions: 19,
        artifacts: 1,
        framebuffers: 1,
        command_line_bytes: 38,
        runtime_arena_bytes: 16_777_216,
    };
    let journey = json!({
        "status": "world", "profile_id": boot.profile_id, "build_id": boot.build_id,
        "image_id": boot.image_binding, "host_id": boot.host_id, "boot_id": boot.boot_id,
        "presenter_implementation_id": "presentation/renderer-conduitos-native@1",
        "body_id": null, "plan_id": null, "active_play_id": null,
    });
    (boot, journey)
}

#[test]
fn exact_product_arrives_at_zero_body_creche() {
    let (boot, journey) = arrival();
    validate(
        &journey,
        &boot,
        "profile/exact",
        "build/exact",
        "image/exact",
    )
    .unwrap();
}

#[test]
fn arrival_rejects_invented_or_unreported_lifecycle() {
    for field in ["body_id", "plan_id", "active_play_id"] {
        for invented in [false, true] {
            let (boot, mut journey) = arrival();
            if invented {
                journey[field] = json!("unexpected/lifecycle");
            } else {
                journey.as_object_mut().unwrap().remove(field);
            }
            let error = validate(
                &journey,
                &boot,
                "profile/exact",
                "build/exact",
                "image/exact",
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains(&format!("journey.{field}")), "{error}");
        }
    }
}

#[test]
fn arrival_rejects_opened_plot_and_mismatched_product_identity() {
    for field in [
        "status",
        "profile_id",
        "build_id",
        "image_id",
        "host_id",
        "boot_id",
        "presenter_implementation_id",
    ] {
        let (boot, mut journey) = arrival();
        journey[field] = json!(if field == "status" {
            "plot-opened"
        } else {
            "wrong"
        });
        let error = validate(
            &journey,
            &boot,
            "profile/exact",
            "build/exact",
            "image/exact",
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains(&format!("journey.{field}")), "{error}");
    }
    let (mut boot, journey) = arrival();
    boot.image_binding = "image/wrong".into();
    let error = validate(
        &journey,
        &boot,
        "profile/exact",
        "build/exact",
        "image/exact",
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("boot.image_binding"), "{error}");
}
