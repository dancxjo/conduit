fn main() -> Result<(), String> {
    let snapshot = conduit_browser_patchbay_workbench::learned_demonstration_snapshot()?;
    let server = conduit_browser_patchbay_workbench::PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .map_err(|error| error.to_string())?;
    println!(
        "PATCHBAY_HTML_URL=http://{}",
        server.local_addr().map_err(|error| error.to_string())?
    );
    server.serve().map_err(|error| error.to_string())
}
