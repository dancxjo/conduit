use conduit_core::SignId;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let snapshot_path = arguments
        .next()
        .ok_or("usage: patchbay-static-assets SNAPSHOT THEME [RUNTIME DESTINATION]")?;
    let theme_path = arguments
        .next()
        .ok_or("usage: patchbay-static-assets SNAPSHOT THEME [RUNTIME DESTINATION]")?;
    let package = match arguments.next() {
        Some(runtime) => Some((
            runtime,
            arguments
                .next()
                .ok_or("package staging requires RUNTIME DESTINATION")?,
        )),
        None => None,
    };
    if arguments.next().is_some() {
        return Err("usage: patchbay-static-assets SNAPSHOT THEME [RUNTIME DESTINATION]".into());
    }
    let mut snapshot = patchbay_html::front_door_snapshot()?;
    snapshot.mark_available(SignId::from("patchbay-pages/document-ready"))?;
    std::fs::write(snapshot_path, snapshot.encode()?)?;
    std::fs::write(theme_path, patchbay_html::application_theme_css())?;
    if let Some((runtime, destination)) = package {
        patchbay_html::application_resources::stage(
            std::path::Path::new(&destination),
            &std::fs::read(runtime)?,
        )?;
    }
    Ok(())
}
