//! Embed explicitly selected local synthetic proof inputs; never download data.
use std::{
    env, fs,
    path::{Path, PathBuf},
};
pub fn generate() {
    println!("cargo:rerun-if-env-changed=CONDUITOS_NUMERIC_FIXTURE_DIR");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("numeric_fixture.rs");
    let Some(directory) = env::var_os("CONDUITOS_NUMERIC_FIXTURE_DIR") else {
        fs::write(output,"pub const MATERIALS: Option<super::Materials<'static>> = None;\npub const FILES: &[super::resources::File<'static>] = &[];\n").unwrap();
        return;
    };
    let directory = fs::canonicalize(directory).expect("selected numeric fixture directory");
    let retained = |name: &str, maximum: usize| {
        let path = fs::canonicalize(directory.join(name)).expect("retained numeric fixture file");
        assert_eq!(
            path.parent(),
            Some(directory.as_path()),
            "fixture path must remain direct child"
        );
        let bytes = fs::metadata(&path).unwrap().len();
        assert!(
            bytes > 0 && bytes <= maximum as u64,
            "fixture byte capacity"
        );
        println!("cargo:rerun-if-changed={}", path.display());
        path
    };
    let source = retained("checked-epoch-source.conduit", 1024 * 1024);
    let image = retained("sealed-epoch-plan.json", 64 * 1024 * 1024);
    let definition = retained("native-definition.conduit", 1024 * 1024);
    let recipe = retained("preparation-recipe.json", 2 * 1024 * 1024);
    let mut files = Vec::new();
    for entry in fs::read_dir(&directory).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().expect("ASCII fixture name");
        if name.ends_with(".bin") {
            assert!(
                name.len() <= 128
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
                    && !name.contains("..")
            );
            files.push((name.clone(), retained(&name, 1024 * 1024)));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(files.len() <= 72);
    let mut code = format!(
        "pub const MATERIALS: Option<super::Materials<'static>> = Some(super::Materials {{source: include_bytes!({:?}),reference_image: include_bytes!({:?}),native_definition: include_str!({:?}),recipe: include_bytes!({:?})}});\npub const FILES: &[super::resources::File<'static>] = &[\n",
        utf8(&source),
        utf8(&image),
        utf8(&definition),
        utf8(&recipe)
    );
    for (name, path) in files {
        code.push_str(&format!(
            "super::resources::File {{name:{name:?},bytes:include_bytes!({:?})}},\n",
            utf8(&path)
        ));
    }
    code.push_str("];\n");
    fs::write(output, code).unwrap();
}
fn utf8(path: &Path) -> &str {
    path.to_str().expect("fixture path UTF8")
}
