//! Checked intent is pure; explicit configuration selects the actual directory
//! identity, and each fresh Boot reopens that exact identity before custody.
use conduit_host_make::{BaseInitialization, CheckedHostConfiguration};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub(super) const IMPLEMENTATION: &str = "linux/body-storage-directory@1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RootIdentity {
    pub canonical_path: PathBuf,
    pub device: u64,
    pub inode: u64,
}
fn initialization(
    checked: &CheckedHostConfiguration,
) -> Result<Option<&BaseInitialization>, String> {
    let mut selected = checked.configuration().bases.iter().filter(|base| {
        base.implementation.as_deref().or_else(|| {
            checked
                .resolved_bases()
                .iter()
                .find(|(kind, _)| kind == &base.kind)
                .map(|(_, implementation)| implementation.as_str())
        }) == Some(IMPLEMENTATION)
    });
    let Some(base) = selected.next() else {
        return Ok(None);
    };
    if selected.next().is_some() {
        return Err("one selected Body storage Base is supported".into());
    }
    base.initialization
        .as_ref()
        .map(Some)
        .ok_or("Body storage declaration has no finite initialization".into())
}
#[cfg(all(feature = "installed-body-storage", target_os = "linux"))]
fn parameters(
    initialization: &BaseInitialization,
) -> Result<
    (
        PathBuf,
        String,
        conduit_std_host::body_storage::BodyStorageBounds,
    ),
    String,
> {
    let catalog = conduit_workspace_make::catalog();
    let contract = catalog
        .initialization_contracts
        .get(IMPLEMENTATION)
        .ok_or("Body storage initialization contract unavailable")?;
    initialization.checked_values(contract)?;
    let mut object = serde_json::to_value(initialization)
        .map_err(|e| e.to_string())?
        .as_object()
        .cloned()
        .ok_or("Body storage initialization is not an object")?;
    let root = object
        .remove("root_directory")
        .and_then(|value| value.as_str().map(PathBuf::from))
        .ok_or("Body storage root absent")?;
    let namespace = object
        .remove("pool_namespace")
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or("Body storage pool namespace absent")?;
    if !root.is_absolute()
        || namespace.is_empty()
        || namespace.len() > 96
        || !namespace
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
    {
        return Err("Body storage needs an explicit absolute root and pool namespace".into());
    }
    let bounds: conduit_std_host::body_storage::BodyStorageBounds =
        serde_json::from_value(serde_json::Value::Object(object)).map_err(|e| e.to_string())?;
    bounds.validate()?;
    Ok((root, namespace, bounds))
}
pub(super) fn validate(initialization: &BaseInitialization) -> Result<(), String> {
    #[cfg(all(feature = "installed-body-storage", target_os = "linux"))]
    {
        return parameters(initialization).map(|_| ());
    }
    #[cfg(not(all(feature = "installed-body-storage", target_os = "linux")))]
    {
        let _ = initialization;
        Err("selected Body storage requires installed-body-storage on Linux".into())
    }
}
pub(super) fn select_identity(
    checked: &CheckedHostConfiguration,
) -> Result<Option<RootIdentity>, String> {
    let Some(initialization) = initialization(checked)? else {
        return Ok(None);
    };
    #[cfg(all(feature = "installed-body-storage", target_os = "linux"))]
    {
        let (root, namespace, bounds) = parameters(initialization)?;
        if bounds.preparation_bytes > checked.configuration().limits.heap_arena_bytes {
            return Err(
                "Body storage preparation constraint exceeds checked Host heap arena".into(),
            );
        }
        let selected = conduit_std_host::body_storage::SelectedBodyStorage::open(
            &root, &namespace, bounds, None,
        )?;
        Ok(Some(RootIdentity {
            canonical_path: selected.identity().canonical_path.clone(),
            device: selected.identity().device,
            inode: selected.identity().inode,
        }))
    }
    #[cfg(not(all(feature = "installed-body-storage", target_os = "linux")))]
    {
        let _ = initialization;
        Err("selected Body storage requires installed-body-storage on Linux".into())
    }
}
pub(super) fn validate_binding(
    checked: &CheckedHostConfiguration,
    identity: Option<&RootIdentity>,
) -> Result<(), String> {
    if initialization(checked)?.is_some() != identity.is_some() {
        return Err("retained Body storage has no exact selected root identity".into());
    }
    if let Some(identity) = identity {
        if !identity.canonical_path.is_absolute() || identity.inode == 0 {
            return Err("retained Body storage root identity invalid".into());
        }
    }
    Ok(())
}
pub(super) fn attach(
    initialization: &BaseInitialization,
    identity: Option<&RootIdentity>,
    host: &mut conduit_std_host::StdHost,
) -> Result<(), String> {
    #[cfg(all(feature = "installed-body-storage", target_os = "linux"))]
    {
        let identity =
            identity.ok_or("Body storage has no explicitly retained directory identity")?;
        let (root, namespace, bounds) = parameters(initialization)?;
        let expected = conduit_std_host::body_storage::BodyStorageRootIdentity {
            canonical_path: identity.canonical_path.clone(),
            device: identity.device,
            inode: identity.inode,
        };
        let selected = conduit_std_host::body_storage::SelectedBodyStorage::open(
            &root,
            &namespace,
            bounds,
            Some(&expected),
        )?;
        host.attach_selected_body_storage(selected)
    }
    #[cfg(not(all(feature = "installed-body-storage", target_os = "linux")))]
    {
        let _ = (initialization, identity, host);
        Err("selected Body storage requires installed-body-storage on Linux".into())
    }
}
