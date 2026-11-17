//! Explicit operator acquisition/preparation, launched by cargo xtask. No Mask,
//! worker, Body, grant or installed IMAGE authority is created by this tool.
#[cfg(unix)]
fn main() -> Result<(), String> {
    use conduit_std_host::ambient_wallpaper::{acquisition::*, cache};
    use std::{
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [date_flag, date, output_flag, output] = args.as_slice() else {
        return Err("enter through cargo xtask make host ambient-photos --date YYYY-MM-DD --output DIRECTORY".into());
    };
    if date_flag != "--date" || output_flag != "--output" || date.len() != 10 || output.len() > 2048
    {
        return Err("explicit bounded --date and --output are required".into());
    }
    let latest = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "invalid civil photo date")?;
    if latest.to_string() != *date {
        return Err("photo date must be YYYY-MM-DD".into());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock precedes Unix epoch")?;
    let today = chrono::DateTime::from_timestamp(
        i64::try_from(now.as_secs()).map_err(|_| "clock overflow")?,
        0,
    )
    .ok_or("clock timestamp invalid")?
    .date_naive();
    if latest > today {
        return Err("photo preparation date is in the future".into());
    }
    let requested = Path::new(output);
    let name = requested
        .file_name()
        .ok_or("photo output must name a new cache directory")?;
    let parent = requested
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let output = parent
        .canonicalize()
        .map_err(|_| "photo cache parent must already exist")?
        .join(name);
    if std::fs::symlink_metadata(&output).is_ok() {
        return Err("photo cache destination already exists".into());
    }
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .map_err(|_| "workspace identity")?;
    if output.starts_with(&workspace) && !output.starts_with(workspace.join("target")) {
        return Err("generated photo cache must use ignored target/ output or an external private directory".into());
    }
    let admission = PreparationAdmission::admit(latest, MAXIMUM_RETAINED_BYTES, PREPARATION_BYTES)
        .map_err(|why| format!("photo preparation admission: {why:?}"))?;
    eprintln!(
        "Preparing exactly four real photographs; at most {MAXIMUM_ATTEMPTS} dates, no placeholders."
    );
    let deck = prepare_deck_https(admission)
        .map_err(|why| format!("bounded photo acquisition refused: {why:?}"))?;
    let checksum = cache::write_deck(&output, &deck)?;
    let checksum: String = checksum.iter().map(|byte| format!("{byte:02x}")).collect();
    let photos: Vec<_> = deck
        .photos()
        .iter()
        .map(|p| {
            serde_json::json!({
                "date":p.candidate().date,"title":p.candidate().title,
                "asset_url":p.candidate().asset_url,"credit":p.candidate().credit,
                "license":p.license(),"width":p.width(),"height":p.height()
            })
        })
        .collect();
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({
        "manifest":output.join("manifest.json"),"manifest_sha256":checksum,
        "prepared_photos":photos,"retained_heap_bytes":deck.retained_bytes(),
        "preparation_reservation_bytes":PREPARATION_BYTES,
        "scope":"Prepared immutable cache only; no Body, IMAGE installation, resource grant, or physical proof"
    })).map_err(|_| "photo result encoding")?);
    Ok(())
}
#[cfg(not(unix))]
fn main() -> Result<(), String> {
    Err("ambient-photo preparation requires the Unix cache profile".into())
}
