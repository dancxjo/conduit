//! Compiled snapshot meaning; offers and authority remain separately admitted.
use super::*;
pub(super) fn install(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias(
        "ResourceRef",
        conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
    )?;
    startup.insert_value_kind_alias("JsonText", conduit_core::kind_id("text/json-utf8@1"))?;
    conduit_semantic_catalog::install_resource_snapshot_catalogs(startup, profiles)?;
    for publish in [true, false] {
        let kind = conduit_semantic_catalog::resource_snapshot_semantic_contract(publish);
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn installed_snapshot_profiles_check_through_product_catalog() {
        let sources = [
            include_str!("../../../../bodies/environment/profiles/recording-publication.conduit"),
            include_str!("../../../../bodies/environment/profiles/catalog-checkpoint.conduit"),
            include_str!("../../../../bodies/environment/profiles/catalog-seed-read.conduit"),
        ];
        for source in sources {
            let source = source.replace("@REFERENCE@", "00");
            let parsed = super::super::parse(&source).unwrap();
            parsed.expand_entry_for_authoring().unwrap();
        }
        let read = "plot read (\n >> reference: ResourceRef\n content: JsonText >>\n) {\n snapshot: resource/read-json-snapshot(\"00\")\n reference >> snapshot.source\n snapshot.value >> content\n}";
        super::super::parse(read)
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
    }
}
