//! Receipt-shape negatives; live entry separately uses the actual full decoder.
use super::*;
fn fixture() -> (Value, Value, Value, Value) {
    let choice = CHOICES[0];
    let boot = json!({"host_id":"host","boot_id":"boot","offer_generation":1});
    let before = json!({"observation_sequence":1,"play_sequence":1,"face":{"identity":"face","revision":1},"show":{"show_id":"old-show","mask_plan_id":"old-plan"}});
    let fresh = json!({"observation_sequence":2,"play_sequence":2,"face":{"identity":"face","revision":1},"show":{"host_id":"host","boot_id":"boot","offer_generation":1,"show_id":"new-show","mask_plan_id":"new-plan","mask_active_play_id":"play","manifestation_id":"manifestation","manifestation":{"placement_id":"placement"},"mask_plot":{"source_document_id":"document"}}});
    let configuration = json!({"root":choice.root,"invocation_path":[choice.root,"component"],"implementation_id":custody::PRESENTER,"execution_profile_id":"conduitos/bounded-mask@1","placement_id":"placement",
        "component":{"source_document_id":"document","checked_plot_id":"component-checked","fore_id":"fore"},"recipe_canonical_bytes":2313,"recipe_canonical_sha256":"a".repeat(64)});
    let expression = json!({"outcome":"Selected","platform_forced_colors":false,"accessibility_overrides":{"forced_colors":false,"reduced_motion":true,"text_scale":false},
        "expression":{"version":1,"density":choice.density,"palette":"phosphor","text_scale_percent":choice.scale,"identity":choice.identity,"motion":"none"}});
    let r = json!({"schema":"conduit.conduitos/component-realization@1","selection_entrance":"local-creche","local_scanout_acknowledged":true,"owner_show_acknowledged":false,"fallback":null,
        "previous_show_id":"old-show","show_id":"new-show","mask_plan_id":"new-plan","active_play_id":"play","manifestation_id":"manifestation","face_id":"face","face_revision":1,
        "host_id":"host","boot_id":"boot","offer_generation":1,"render":{"commands":3},"configuration":configuration,"expression":expression});
    (r, before, fresh, boot)
}
#[test]
fn foreign_boot_stale_show_and_wrong_configuration_refuse() {
    let (r, before, fresh, boot) = fixture();
    assert!(fields(&r, &before, &fresh, &boot, CHOICES[0]).is_ok());
    for key in [
        "boot_id",
        "host_id",
        "previous_show_id",
        "show_id",
        "face_id",
        "mask_plan_id",
        "active_play_id",
        "manifestation_id",
    ] {
        let mut wrong = r.clone();
        wrong[key] = json!("foreign-or-stale");
        assert!(
            fields(&wrong, &before, &fresh, &boot, CHOICES[0]).is_err(),
            "{key}"
        );
    }
    for key in [
        "root",
        "implementation_id",
        "execution_profile_id",
        "placement_id",
        "recipe_canonical_sha256",
    ] {
        let mut wrong = r.clone();
        wrong["configuration"][key] = json!("forged");
        assert!(
            fields(&wrong, &before, &fresh, &boot, CHOICES[0]).is_err(),
            "{key}"
        );
    }
    let mut fallback = r.clone();
    fallback["fallback"] = json!({"reason":"not-the-chosen-recipe"});
    assert!(fields(&fallback, &before, &fresh, &boot, CHOICES[0]).is_err());
    assert!(fields(&r, &before, &fresh, &boot, CHOICES[1]).is_err());
    let mut wrong = r;
    wrong["expression"]["expression"]["palette"] = json!("engineering");
    assert!(fields(&wrong, &before, &fresh, &boot, CHOICES[0]).is_err());
}
#[test]
fn missing_duplicate_or_replayed_records_and_changed_source_refuse() {
    assert!(new_show(&[json!("arrival"), json!("first")], &json!("arrival")).is_err());
    assert!(new_show(&[json!("arrival"), json!("first")], &json!("second")).is_ok());
    assert!(one_new(&[], 0).is_err());
    assert!(one_new(&[json!({}), json!({})], 0).is_err());
    assert!(one_new(&[json!({})], 1).is_err());
    assert!(one_new(&[json!({})], 0).is_ok());
    let (_, before, _, _) = fixture();
    assert!(source_unchanged(&before, &before, [0, 0, 0], [0, 0, 0]).is_ok());
    for changed in [[1, 0, 0], [0, 1, 0], [0, 0, 1]] {
        assert!(source_unchanged(&before, &before, [0, 0, 0], changed).is_err());
    }
    let mut after = before.clone();
    after["face"]["revision"] = json!(2);
    assert!(source_unchanged(&before, &after, [0, 0, 0], [0, 0, 0]).is_err());
}
