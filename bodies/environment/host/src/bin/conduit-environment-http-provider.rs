//! Qualified environment worker entrance for the existing confined HTTP Base.
fn main() {
    if let Err(error) = conduit_std_host::isolated_http_provider_main() {
        eprintln!("isolated HTTP provider refused: {error}");
        std::process::exit(1);
    }
}
