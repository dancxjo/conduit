fn main() -> Result<(), String> {
    let snapshot = patchbay_html::learned_demonstration_snapshot()?;
    let server = patchbay_html::PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .map_err(|error| error.to_string())?;
    println!(
        "PATCHBAY_HTML_URL=http://{}",
        server.local_addr().map_err(|error| error.to_string())?
    );
    server.serve().map_err(|error| error.to_string())
}
