//! Compile actual emitted code as both an executable and a no_std library.
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub fn verify(generated: &str, exercise: &str, name: &str) {
    let directory = std::env::temp_dir().join(format!(
        "conduit-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).unwrap();
    let dependencies = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let library = fs::read_dir(&dependencies)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("libconduit_plot-")
                && entry.path().extension().is_some_and(|ext| ext == "rlib")
        })
        .max_by_key(|entry| entry.metadata().unwrap().modified().unwrap())
        .unwrap()
        .path();
    for (name, contents, kind) in [
        ("image", format!("{generated}{exercise}"), "bin"),
        ("no_std", format!("#![no_std]\n{generated}"), "rlib"),
    ] {
        let source = directory.join(format!("{name}.rs"));
        let output = directory.join(name);
        fs::write(&source, contents).unwrap();
        let built = Command::new("rustc")
            .args(["--edition=2021", "--crate-type", kind, "-L"])
            .arg(format!("dependency={}", dependencies.display()))
            .arg("--extern")
            .arg(format!("conduit_plot={}", library.display()))
            .arg(&source)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        if kind == "bin" {
            let ran = Command::new(output).output().unwrap();
            assert!(
                ran.status.success(),
                "{}",
                String::from_utf8_lossy(&ran.stderr)
            );
        }
    }
    fs::remove_dir_all(directory).unwrap();
}
