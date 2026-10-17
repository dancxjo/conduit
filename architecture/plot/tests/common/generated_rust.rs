//! Compile and execute generated checked-Type consumers, then compile no_std.
use std::{ffi::OsStr, fs, path::Path, process::Command};

pub fn exercise(generated: &str, exercise: &str, expected_tests: usize, fixture: &str) {
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let library = fs::read_dir(&dependencies)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension() == Some(OsStr::new("rlib"))
                && path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .is_some_and(|name| name.starts_with("libconduit_plot-"))
        })
        .max_by_key(|path| {
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
        })
        .expect("compiled Plot library");
    let directory = std::env::temp_dir().join(format!("conduit-{fixture}-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let source = directory.join("bindings.rs");
    fs::write(&source, format!("{}{}", generated, exercise)).unwrap();
    let executable = directory.join("bindings-test");
    let mut compiler = rustc(&dependencies, &library);
    let output = compiler
        .args(["--test"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated consumer compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(&executable).output().unwrap();
    assert!(
        output.status.success(),
        "generated consumer execution:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result = String::from_utf8_lossy(&output.stdout);
    assert!(
        result.contains(&format!("{expected_tests} passed; 0 failed")),
        "{result}"
    );
    println!("{result}");
    let no_std_source = directory.join("bindings_no_std.rs");
    fs::write(&no_std_source, format!("#![no_std]\n{}", generated)).unwrap();
    let output = rustc(&dependencies, &library)
        .arg("--crate-type=lib")
        .arg(&no_std_source)
        .arg("-o")
        .arg(directory.join("libbindings_no_std.rlib"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "generated no_std consumer compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

fn rustc(dependencies: &Path, library: &Path) -> Command {
    let mut compiler = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    compiler
        .arg("--edition=2021")
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg("--extern")
        .arg(format!("conduit_plot={}", library.display()));
    compiler
}
