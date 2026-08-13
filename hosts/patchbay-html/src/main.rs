use patchbay_html::{cross_host_demonstration_snapshot, PatchbayHtmlServer};

fn main() -> Result<(), String> {
    let documentary_fixture = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--documentary-fixture") if std::env::args().len() == 2 => true,
        Some(argument) => {
            return Err(format!(
                "unknown Patchbay HTML argument {argument}; the public entrance takes no arguments"
            ))
        }
    };
    let server = if documentary_fixture {
        let snapshot = cross_host_demonstration_snapshot().map_err(|error| error.to_string())?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?
    } else {
        PatchbayHtmlServer::bind_browser_front_door_ephemeral()
            .map_err(|error| error.to_string())?
    };
    println!(
        "PATCHBAY_HTML_URL=http://{}",
        server.local_addr().map_err(|error| error.to_string())?
    );
    server.serve().map_err(|error| error.to_string())?;
    Ok(())
}
