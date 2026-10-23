//! Internal provisioning stage of the supported physical owner proof.
#[path = "native_owner/key.rs"]
mod key;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory =
        std::path::PathBuf::from(args.next().ok_or("expected private owner directory")?);
    if args.next().is_some() {
        return Err("expected one private owner directory".into());
    }
    let _key = key::open(&directory, true)?;
    println!("{{\"owner_key_ready\":true,\"private_key_printed\":false}}");
    Ok(())
}
