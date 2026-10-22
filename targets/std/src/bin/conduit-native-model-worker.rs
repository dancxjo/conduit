#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
fn main() {
    if let Err(error) = conduit_std_host::native_model_backing::worker_main() {
        eprintln!("native model worker: {error:?}");
        std::process::exit(1);
    }
}
#[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
fn main() {
    eprintln!("native model backing requires Linux x86_64 glibc");
    std::process::exit(1);
}
