//! Explicit refabrication binds checked new Source, IMAGE and reviewed release
//! together. It neither creates a Seed nor confers equipment readiness.
use super::*;
pub(in crate::durable_host) fn replacement_for_release(
    retained: Option<Selection>,
    configuration: Option<&Path>,
    image: Option<&Path>,
    release_source: &str,
    release_bytes: &[u8],
) -> Result<Option<Selection>, String> {
    let Some(path) = configuration else {
        return replacement_image(retained, image, release_source);
    };
    let image = image.ok_or("replacement configuration requires --host-image")?;
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".host.conduit"))
    {
        return Err("Host configuration requires canonical .host.conduit source".into());
    }
    let bytes = bounded_read(path, MAXIMUM_SOURCE_BYTES)?;
    let source = std::str::from_utf8(&bytes).map_err(|_| "Host configuration must be UTF-8")?;
    let target = running_target_id()?;
    let mut selected = Selection::from_source(source, target)?;
    let release: serde_json::Value = serde_json::from_slice(release_bytes)
        .map_err(|error| format!("replacement release: {error}"))?;
    if release.get("target_id").and_then(serde_json::Value::as_str) != Some(target) {
        return Err("replacement release target differs from actual running Host".into());
    }
    let image: conduit_host_make::HostImage =
        serde_json::from_slice(&bounded_read(image, 64 * 1024)?)
            .map_err(|error| format!("replacement Host IMAGE: {error}"))?;
    check_seed_image(&selected.source, &image)?;
    if image.manifest.source_identity != release_source || image.manifest.target != target {
        return Err("replacement Source, IMAGE, release and actual Host target must match".into());
    }
    selected.host_image = Some(image);
    selected.validate()?;
    Ok(Some(selected))
}
