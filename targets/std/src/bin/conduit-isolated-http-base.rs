fn main() {
    if let Err(error) = conduit_std_host::isolated_http_provider_main() {
        eprintln!("isolated HTTP Base: {error}");
        std::process::exit(1);
    }
}
