//! Join actual canonical build metadata to Boot/world and current component Show.
//! Logical resolved-image binding and final bootable ISO identity stay distinct.
use super::*;
pub(super) const PRESENTER: &str = "face/renderer-conduitos-component@1";
fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|s| !s.is_empty())
}

pub(super) fn validate_world(
    metadata: &Value,
    boot: &Value,
    world: &Value,
    source: &str,
    iso: &str,
) -> Result<(), ConduitosError> {
    let target = &metadata["target_build_manifest"];
    let resolved = &metadata["resolved_image"]["manifest"];
    if target["schema"] != "conduit.host/target-build-manifest@3"
        || target["artifact_role"] != "product-host"
        || target["source_identity"] != source
        || target["target"] != "conduitos/x86_64/pc"
        || target["image"]["sha256"] != iso
        || target["image_id"] != format!("image:sha256:{iso}")
        || resolved["schema"] != "conduit.host/build-manifest@3"
        || resolved != &target["resolved_build"]
        || resolved["source_identity"] != source
        || target["resolved_description_binding"] != resolved["image_id"]
        || ["profile_id", "build_id"]
            .iter()
            .any(|key| !nonempty(&target[*key]) || target[*key] != resolved[*key])
        || !nonempty(&resolved["image_id"])
        || ["profile_id", "build_id"]
            .iter()
            .any(|key| metadata["resolved_image"]["payload"][*key] != resolved[*key])
        || boot["schema"] != "conduit.conduitos.boot-sign/v1"
        || boot["status"] != "accepted"
        || boot["arch"] != "x86_64"
        || boot["qemu_profile"] != "q35-single-cpu-512m-normal-product"
        || boot["image_binding"] != resolved["image_id"]
        || world["image_id"] != resolved["image_id"]
        || world["status"] != "world"
        || !world["body_id"].is_null()
        || ["profile_id", "build_id"]
            .iter()
            .any(|key| boot[*key] != target[*key] || world[*key] != target[*key])
        || ["host_id", "boot_id"]
            .iter()
            .any(|key| !nonempty(&boot[*key]) || world[*key] != boot[*key])
        || boot["offer_generation"].as_u64().is_none_or(|n| n == 0)
        || world["offer_generation"] != boot["offer_generation"]
    {
        return Err(refusal(
            "controls Source/build/profile/logical image/Boot/world custody differs",
        ));
    }
    Ok(())
}
pub(super) fn component(record: &Value) -> Result<(), ConduitosError> {
    let show = &record["show"];
    let manifestation = &show["manifestation"];
    if manifestation["presenter_implementation_id"] != PRESENTER
        || manifestation["presenter_capability_id"] != "renderer-conduitos-component"
        || manifestation["target_subject"] != "conduitos/creche/face"
        || ["source_document_id", "checked_plot_id", "expanded_plot_id"]
            .iter()
            .any(|key| !nonempty(&show["mask_plot"][*key]))
        || [
            "host_id",
            "boot_id",
            "face_id",
            "face_revision",
            "offer_generation",
            "manifestation_id",
            "lifecycle",
        ]
        .iter()
        .any(|key| show[*key] != manifestation[*key])
        || show["mask_plan_id"] != manifestation["plan_id"]
        || show["mask_active_play_id"] != manifestation["active_play_id"]
    {
        return Err(refusal(
            "current Crèche route lacks component presenter/checked Mask Source/Show chain",
        ));
    }
    Ok(())
}
pub(super) fn validate(
    metadata: &Value,
    boot: &Value,
    world: &Value,
    record: &Value,
    source: &str,
    iso: &str,
) -> Result<(), ConduitosError> {
    validate_world(metadata, boot, world, source, iso)?;
    component(record)?;
    if world["face_id"] != record["face"]["identity"]
        || world["manifestation_id"] != record["show"]["manifestation_id"]
        || world["presenter_implementation_id"] != PRESENTER
        || ["host_id", "boot_id", "offer_generation"]
            .iter()
            .any(|key| record["show"][*key] != boot[*key])
    {
        return Err(refusal(
            "ready world is not the actual acknowledged component presentation",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Value, Value, Value, Value) {
        // Identity receipt fixture only: production separately verifies actual
        // resolved-image bytes with HostMake and validates full Face/Show.
        let resolved = json!({"schema":"conduit.host/build-manifest@3","source_identity":"source","profile_id":"profile","build_id":"build","image_id":"image:logical"});
        let metadata = json!({"target_build_manifest":{"schema":"conduit.host/target-build-manifest@3","artifact_role":"product-host","source_identity":"source","target":"conduitos/x86_64/pc","image":{"sha256":"iso"},"image_id":"image:sha256:iso","resolved_description_binding":"image:logical","profile_id":"profile","build_id":"build","resolved_build":resolved},"resolved_image":{"manifest":resolved,"payload":{"profile_id":"profile","build_id":"build"}}});
        let boot = json!({"schema":"conduit.conduitos.boot-sign/v1","status":"accepted","arch":"x86_64","qemu_profile":"q35-single-cpu-512m-normal-product","profile_id":"profile","build_id":"build","image_binding":"image:logical","host_id":"host","boot_id":"boot","offer_generation":1});
        let world = json!({"status":"world","body_id":null,"profile_id":"profile","build_id":"build","image_id":"image:logical","host_id":"host","boot_id":"boot","offer_generation":1,"face_id":"face","manifestation_id":"manifestation","presenter_implementation_id":PRESENTER});
        let manifestation = json!({"host_id":"host","boot_id":"boot","offer_generation":1,"face_id":"face","face_revision":1,"manifestation_id":"manifestation","lifecycle":"Available","presenter_implementation_id":PRESENTER,"presenter_capability_id":"renderer-conduitos-component","target_subject":"conduitos/creche/face","plan_id":"plan","active_play_id":"play"});
        let record = json!({"face":{"identity":"face"},"show":{"host_id":"host","boot_id":"boot","offer_generation":1,"face_id":"face","face_revision":1,"manifestation_id":"manifestation","lifecycle":"Available","mask_plan_id":"plan","mask_active_play_id":"play","mask_plot":{"source_document_id":"source/doc","checked_plot_id":"checked","expanded_plot_id":"expanded"},"manifestation":manifestation}});
        (metadata, boot, world, record)
    }
    #[test]
    fn logical_binding_is_not_final_iso_and_all_guest_ids_must_match() {
        let (metadata, boot, world, record) = fixture();
        assert!(validate(&metadata, &boot, &world, &record, "source", "iso").is_ok());
        for key in ["profile_id", "build_id", "image_id", "host_id", "boot_id"] {
            let mut wrong = world.clone();
            wrong[key] = json!("foreign");
            assert!(validate(&metadata, &boot, &wrong, &record, "source", "iso").is_err());
        }
        let mut wrong = boot.clone();
        wrong["image_binding"] = metadata["target_build_manifest"]["image_id"].clone();
        assert!(validate(&metadata, &wrong, &world, &record, "source", "iso").is_err());
        assert!(validate(&metadata, &boot, &world, &record, "stale-source", "iso").is_err());
        assert!(validate(&metadata, &boot, &world, &record, "source", "wrong-iso").is_err());
        let mut wrong = metadata.clone();
        wrong["resolved_image"]["manifest"]["build_id"] = json!("stale");
        assert!(validate(&wrong, &boot, &world, &record, "source", "iso").is_err());
    }
    #[test]
    fn generic_or_stale_presenter_is_not_evidence_of_component_route() {
        let (metadata, boot, world, record) = fixture();
        for key in [
            "presenter_implementation_id",
            "presenter_capability_id",
            "plan_id",
            "face_id",
        ] {
            let mut wrong = record.clone();
            wrong["show"]["manifestation"][key] = json!("generic-or-stale");
            assert!(validate(&metadata, &boot, &world, &wrong, "source", "iso").is_err());
        }
        let mut wrong = record;
        wrong["show"]["mask_plot"]["checked_plot_id"] = Value::Null;
        assert!(validate(&metadata, &boot, &world, &wrong, "source", "iso").is_err());
    }
}
