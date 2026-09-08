#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = conduit_std_host::isolated_copy_provider_main() {
        eprintln!("isolated copy Base failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("the isolated copy Base is currently Linux-only");
    std::process::exit(2);
}
