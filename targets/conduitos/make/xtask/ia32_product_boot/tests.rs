use super::*;

#[test]
fn incomplete_product_sign_is_not_accepted_early() {
    assert_eq!(
        complete_line("CONDUIT_IA32_PRODUCT {\"schema\":", PREFIX),
        None
    );
    assert_eq!(
        complete_line(
            "firmware\0CONDUIT_IA32_PRODUCT {\"schema\":\"complete\"}\n",
            PREFIX
        ),
        Some("{\"schema\":\"complete\"}")
    );
}

#[test]
fn exact_product_sign_rejects_stale_bindings() {
    let exact = serde_json::json!({
        "schema": "conduit.conduitos/ia32-product@1",
        "status": "ready",
        "profile_id": "profile",
        "build_id": "build",
        "image_id": "image",
        "host_id": "host",
        "boot_id": "boot",
        "body_id": null,
        "interactive_local_control": false,
        "long_lived": true,
        "semantic_result": "HELLO, CONDUITOS",
        "presenter_implementation_id": "presenter/ia32-linear-debugcon@1",
        "ordinary_plan_id": "plan",
        "ordinary_play_id": "play"
    });
    assert!(validate_sign(&exact, "profile", "build", "image").is_ok());
    let mut stale = exact;
    stale["image_id"] = "stale".into();
    assert!(validate_sign(&stale, "profile", "build", "image").is_err());
}
