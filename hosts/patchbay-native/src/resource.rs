//! Native protected file/resource adapter for canonical Form documents.

use patchbay_model::FormEditor;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub fn open_form_resource(path: PathBuf) -> Result<FormEditor, String> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("conduit") {
        return Err("canonical Form paths must end in .conduit".into());
    }
    let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file() {
        return Err("canonical Form resource is not a regular file".into());
    }
    if metadata.len() as usize > patchbay_model::MAX_FORM_SOURCE_BYTES {
        return Err("canonical Form resource exceeds its finite byte bound".into());
    }
    let source = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    FormEditor::from_source(path, source).map_err(|error| error.to_string())
}

pub fn save_form_resource(editor: &mut FormEditor) -> Result<(), String> {
    let view = editor.view();
    let parent = view
        .path
        .parent()
        .ok_or("canonical Form resource has no parent")?;
    let file_name = view
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("canonical Form resource has no file name")?;
    let temporary = parent.join(format!(".{file_name}.patchbay-save"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    if let Err(error) = file
        .write_all(view.source.as_bytes())
        .and_then(|_| file.sync_all())
    {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    fs::rename(&temporary, &view.path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })?;
    editor
        .mark_saved(view.revision)
        .map_err(|error| error.to_string())
}

const MAX_LAYOUT_BYTES: u64 = 64 * 1024;

pub fn open_layout_resource(editor: &FormEditor) -> Result<patchbay_model::PatchbayLayout, String> {
    let path = layout_path(editor);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_LAYOUT_BYTES {
        return Err("Patchbay layout resource is not one bounded regular file".into());
    }
    let encoded = fs::read(&path).map_err(|error| error.to_string())?;
    let layout: patchbay_model::PatchbayLayout =
        serde_json::from_slice(&encoded).map_err(|error| error.to_string())?;
    layout
        .validate()
        .map_err(|error| format!("invalid Patchbay layout: {error:?}"))?;
    Ok(layout)
}

pub fn save_layout_resource(
    editor: &FormEditor,
    layout: &patchbay_model::PatchbayLayout,
) -> Result<(), String> {
    layout
        .validate()
        .map_err(|error| format!("invalid Patchbay layout: {error:?}"))?;
    let path = layout_path(editor);
    let encoded = serde_json::to_vec_pretty(layout).map_err(|error| error.to_string())?;
    if encoded.len() as u64 > MAX_LAYOUT_BYTES {
        return Err("Patchbay layout resource exceeds its finite byte bound".into());
    }
    let temporary = path.with_extension("patchbay-layout-save");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    if let Err(error) = file.write_all(&encoded).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })
}

pub fn layout_path(editor: &FormEditor) -> PathBuf {
    let mut value = editor.view().path.into_os_string();
    value.push(".patchbay.json");
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_and_atomically_saves_canonical_source() {
        let directory =
            std::env::temp_dir().join(format!("patchbay-native-form-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("document.conduit");
        let hello = include_str!("../../../examples/hello.conduit");
        let greet = include_str!("../../../examples/greet.conduit");
        std::fs::write(&path, hello).unwrap();

        let mut editor = open_form_resource(path.clone()).unwrap();
        editor.replace_source(greet.into()).unwrap();
        editor.recheck().unwrap();
        save_form_resource(&mut editor).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), greet);
        assert_eq!(editor.view().saved_revision, editor.view().revision);
        assert!(open_form_resource(directory.join("document.json")).is_err());

        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
