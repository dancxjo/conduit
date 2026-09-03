//! Standalone repository entrance for one independent browser Host incarnation.

mod launcher;
mod server;

fn main() -> Result<(), String> {
    let entrance = parse_arguments(std::env::args().skip(1))?;
    let server = match &entrance.application {
        Some(application) => {
            server::BrowserHostServer::bind_application(&application.directory, &application.mount)?
        }
        None => {
            let runtime_path = std::env::var_os("CONDUIT_BROWSER_RUNTIME_WASM")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    std::path::PathBuf::from(
                        "target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm",
                    )
                });
            server::BrowserHostServer::bind(&runtime_path)?
        }
    };
    let url = server.url()?;
    println!("CONDUIT_BROWSER_HOST_URL={url}");
    if entrance.launch {
        launcher::open(&url)?;
    }
    server.serve()
}

#[derive(Debug, PartialEq, Eq)]
struct Entrance {
    launch: bool,
    application: Option<ApplicationEntrance>,
}

#[derive(Debug, PartialEq, Eq)]
struct ApplicationEntrance {
    directory: std::path::PathBuf,
    mount: String,
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Entrance, String> {
    let mut entrance = Entrance {
        launch: true,
        application: None,
    };
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--no-open" if entrance.launch => entrance.launch = false,
            "--application" if entrance.application.is_none() => {
                let directory = arguments
                    .next()
                    .ok_or("--application requires a staged application directory")?;
                if arguments.next().as_deref() != Some("--mount") {
                    return Err("--application requires --mount /PATH/".into());
                }
                let mount = arguments
                    .next()
                    .ok_or("--application requires --mount /PATH/")?;
                entrance.application = Some(ApplicationEntrance {
                    directory: directory.into(),
                    mount,
                });
            }
            _ => return Err(
                "usage: conduit-browser-host [--application DIRECTORY --mount /PATH/] [--no-open]"
                    .into(),
            ),
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
                application: None,
            })
        );
        assert_eq!(
            parse_arguments(["--no-open".to_owned()].into_iter()),
            Ok(Entrance {
                launch: false,
                application: None,
            })
        );
        assert_eq!(
            parse_arguments(
                [
                    "--application".to_owned(),
                    "target/tour-product".to_owned(),
                    "--mount".to_owned(),
                    "/tour/".to_owned(),
                    "--no-open".to_owned(),
                ]
                .into_iter()
            ),
            Ok(Entrance {
                launch: false,
                application: Some(ApplicationEntrance {
                    directory: "target/tour-product".into(),
                    mount: "/tour/".into(),
                }),
            })
        );
        assert!(parse_arguments(["--book".to_owned()].into_iter()).is_err());
        assert!(parse_arguments(["--creche".to_owned()].into_iter()).is_err());
        assert!(
            parse_arguments(["--application".to_owned(), "book".to_owned()].into_iter()).is_err()
        );
        assert!(parse_arguments(["--port".to_owned(), "4173".to_owned()].into_iter()).is_err());
    }
}
