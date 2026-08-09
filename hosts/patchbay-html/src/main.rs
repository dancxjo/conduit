use patchbay_html::{demonstration_snapshot, PatchbayHtmlServer};

fn main() -> Result<(), String> {
    let snapshot = demonstration_snapshot()?;
    let server =
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?;
    println!(
        "PATCHBAY_HTML_URL=http://{}",
        server.local_addr().map_err(|error| error.to_string())?
    );
    server.serve().map_err(|error| error.to_string())?;
    Ok(())
}
