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
    let server = server::BrowserHostServer::bind(&runtime_path, entrance.surface.into())?;
    let url = match entrance.surface {
        Surface::Host => server.url()?,
        Surface::Book => server.book_url()?,
        Surface::Creche => server.creche_url()?,
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
    surface: Surface,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Surface {
    Host,
    Book,
    Creche,
}

impl From<Surface> for server::ProductSurface {
    fn from(value: Surface) -> Self {
        match value {
            Surface::Host => Self::Host,
            Surface::Book => Self::Book,
            Surface::Creche => Self::Creche,
        }
    }
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Entrance, String> {
    let mut entrance = Entrance {
        launch: true,
        surface: Surface::Host,
    };
    for argument in arguments {
        match argument.as_str() {
            "--no-open" if entrance.launch => entrance.launch = false,
            "--book" if entrance.surface == Surface::Host => entrance.surface = Surface::Book,
            "--creche" if entrance.surface == Surface::Host => entrance.surface = Surface::Creche,
            _ => return Err("usage: conduit-browser-host [--book | --creche] [--no-open]".into()),
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
                surface: Surface::Host,
            })
        );
        assert_eq!(
            parse_arguments(["--no-open".to_owned()].into_iter()),
            Ok(Entrance {
                launch: false,
                surface: Surface::Host,
            })
        );
        assert_eq!(
            parse_arguments(["--book".to_owned(), "--no-open".to_owned()].into_iter()),
            Ok(Entrance {
                launch: false,
                surface: Surface::Book,
            })
        );
        assert_eq!(
            parse_arguments(["--creche".to_owned(), "--no-open".to_owned()].into_iter()),
            Ok(Entrance {
                launch: false,
                surface: Surface::Creche,
            })
        );
        assert!(parse_arguments(["--book".to_owned(), "--creche".to_owned()].into_iter()).is_err());
        assert!(parse_arguments(["--port".to_owned(), "4173".to_owned()].into_iter()).is_err());
    }
}
