//! Standalone repository entrance for one independent browser Host incarnation.

mod launcher;
mod server;

fn main() -> Result<(), String> {
    let entrance = parse_arguments(std::env::args().skip(1))?;
    let runtime_path = std::env::var_os("CONDUIT_BROWSER_RUNTIME_WASM")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(
                "target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
            )
        });
    let server = server::BrowserHostServer::bind(&runtime_path)?;
    let url = if entrance.book {
        server.book_url()?
    } else {
        server.url()?
    };
    println!("CONDUIT_BROWSER_HOST_URL={url}");
    if entrance.launch {
        launcher::open(&url)?;
    }
    server.serve()
}

#[derive(Debug, PartialEq, Eq)]
struct Entrance {
    launch: bool,
    book: bool,
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Entrance, String> {
    let mut entrance = Entrance {
        launch: true,
        book: false,
    };
    for argument in arguments {
        match argument.as_str() {
            "--no-open" if entrance.launch => entrance.launch = false,
            "--book" if !entrance.book => entrance.book = true,
            _ => return Err("usage: conduit-browser-host [--book] [--no-open]".into()),
        }
    }
    Ok(entrance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_explicit_proof_mode_suppresses_the_launcher() {
        assert_eq!(
            parse_arguments(std::iter::empty()),
            Ok(Entrance {
                launch: true,
                book: false,
            })
        );
        assert_eq!(
            parse_arguments(["--no-open".to_owned()].into_iter()),
            Ok(Entrance {
                launch: false,
                book: false,
            })
        );
        assert_eq!(
            parse_arguments(["--book".to_owned(), "--no-open".to_owned()].into_iter()),
            Ok(Entrance {
                launch: false,
                book: true,
            })
        );
        assert!(parse_arguments(["--port".to_owned(), "4173".to_owned()].into_iter()).is_err());
    }
}
