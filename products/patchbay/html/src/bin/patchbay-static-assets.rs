use conduit_core::SignId;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let snapshot_path = arguments
        .next()
        .ok_or("usage: patchbay-static-assets SNAPSHOT THEME")?;
    let theme_path = arguments
        .next()
        .ok_or("usage: patchbay-static-assets SNAPSHOT THEME")?;
    if arguments.next().is_some() {
        return Err("usage: patchbay-static-assets SNAPSHOT THEME".into());
    }
    let mut snapshot = patchbay_html::front_door_snapshot()?;
    snapshot.mark_available(SignId::from("patchbay-pages/document-ready"))?;
    std::fs::write(snapshot_path, snapshot.encode()?)?;
    std::fs::write(theme_path, patchbay_html::application_theme_css())?;
    Ok(())
}
