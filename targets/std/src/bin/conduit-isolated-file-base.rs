#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = conduit_std_host::isolated_base::provider_main() {
        eprintln!("isolated file Base failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("the isolated file Base proof is currently Linux-only");
    std::process::exit(2);
}
