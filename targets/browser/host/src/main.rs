//! Standalone repository entrance for one independent browser Host incarnation.

mod launcher;
mod server;

fn main() -> Result<(), String> {
    let launch = parse_arguments(std::env::args().skip(1))?;
    let runtime_path = std::env::var_os("CONDUIT_BROWSER_RUNTIME_WASM")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(
                "target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
            )
        });
    let server = server::BrowserHostServer::bind(&runtime_path)?;
    let url = server.url()?;
    println!("CONDUIT_BROWSER_HOST_URL={url}");
    if launch {
        launcher::open(&url)?;
    }
    server.serve()
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<bool, String> {
    let arguments = arguments.collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => Ok(true),
        [argument] if argument == "--no-open" => Ok(false),
        _ => Err("usage: conduit-browser-host [--no-open]".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_explicit_proof_mode_suppresses_the_launcher() {
        assert_eq!(parse_arguments(std::iter::empty()), Ok(true));
        assert_eq!(
            parse_arguments(["--no-open".to_owned()].into_iter()),
            Ok(false)
        );
        assert!(parse_arguments(["--port".to_owned(), "4173".to_owned()].into_iter()).is_err());
    }
}
