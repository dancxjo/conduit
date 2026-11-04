//! Compile actual emitted code as both an executable and a no_std library.
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
#[path = "compiled_libraries.rs"]
mod compiled_libraries;
static NEXT: AtomicUsize = AtomicUsize::new(0);
#[allow(dead_code)] // Some proof owners supply additional exact dependencies.
pub fn verify(generated: &str, exercise: &str, name: &str) {
    verify_with_libraries(generated, exercise, name, &["conduit_plot"]);
}
pub fn verify_with_libraries(generated: &str, exercise: &str, name: &str, libraries: &[&str]) {
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
    let libraries = compiled_libraries::resolve(&dependencies, libraries);
    for (name, contents, kind) in [
        ("image", format!("{generated}{exercise}"), "bin"),
        ("no_std", format!("#![no_std]\n{generated}"), "rlib"),
    ] {
        let source = directory.join(format!("{name}.rs"));
        let output = directory.join(name);
        fs::write(&source, contents).unwrap();
        let mut compiler = Command::new("rustc");
        compiler
            .args(["--edition=2021", "--crate-type", kind, "-L"])
            .arg(format!("dependency={}", dependencies.display()));
        for library in &libraries {
            compiler.arg("--extern").arg(library);
        }
        let built = compiler
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
            if !ran.stdout.is_empty() {
                eprintln!("{}", String::from_utf8_lossy(&ran.stdout));
            }
        }
    }
    fs::remove_dir_all(directory).unwrap();
}
