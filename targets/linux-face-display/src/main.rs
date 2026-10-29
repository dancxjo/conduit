//! Read-only Linux framebuffer realization of an exact caller-supplied Face.
mod framebuffer;
mod history_input;
mod input;
mod property_mask;
mod view;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let input_path = PathBuf::from(
        args.next()
            .ok_or("usage: conduit-linux-face-display PRIVATE_FACE_FILE [FRAMEBUFFER]")?,
    );
    let framebuffer_path = PathBuf::from(args.next().unwrap_or_else(|| "/dev/fb0".into()));
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let preferred = match std::env::var("CONDUIT_DISPLAY_TEMPERATURE_UNIT").as_deref() {
        Ok("fahrenheit") => Some(conduit_core::Unit::Fahrenheit),
        Ok("celsius") => Some(conduit_core::Unit::Celsius),
        Err(std::env::VarError::NotPresent) => None,
        _ => return Err("temperature display unit must be fahrenheit or celsius".into()),
    };
    let boot =
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(|e| e.to_string())?;
    let mut fb = framebuffer::Framebuffer::open(&framebuffer_path)?;
    let width = fb.geometry.width as u16;
    let height = fb.geometry.height as u16;
    let mut pixels = vec![0; width as usize * height as usize];
    eprintln!(
        "Linux Face display: {width}x{height} RGB565; read-only; source {}",
        input_path.display()
    );
    eprintln!("Temperature display unit preference: {preferred:?}");
    let history_path = std::env::var_os("CONDUIT_RETAINED_HISTORY_FILE").map(PathBuf::from);
    let start = Instant::now();
    let mut previous = String::new();
    let mut retained = input::RetainedView::default();
    let mut recovered_history = false;
    let mut history = history_input::HistoryInput::new(preferred);
    let mut history_status = String::new();
    loop {
        let now = input::now_ms()?;
        if retained.last(now).is_none() {
            if let Ok(face) = input::read_last_known(&input_path, boot.trim()) {
                retained.accept(face, now);
                recovered_history = true;
            }
        }
        let mut historical_notice = None;
        let (face, status) = match input::read(&input_path, boot.trim()) {
            Ok(face) => {
                let status = format!("Face {} revision {}", face.identity.as_str(), face.revision);
                retained.accept(face.clone(), now);
                recovered_history = false;
                (face, status)
            }
            Err(reason) => {
                if let Some((face, age)) = retained.last(now) {
                    historical_notice = Some(if recovered_history {
                        "LIVE FEED STALE / RECOVERED SAME-BOOT VIEW / SAMPLE AGE NOT REBASED".into()
                    } else {
                        format!(
                            "LIVE FEED STALE / LAST VALID VIEW {}s AGO - {}",
                            age / 1000,
                            reason
                        )
                    });
                    (face.clone(), reason)
                } else {
                    (view::unavailable(&reason), reason)
                }
            }
        };
        let tick = (start.elapsed().as_secs() / 8) as usize;
        if let Some(path) = history_path.as_ref() {
            let status = match history.refresh(path) {
                Ok(_) => String::new(),
                Err(error) => format!(
                    "Retained history input unavailable; last accepted history retained: {error}"
                ),
            };
            if status != history_status {
                if !status.is_empty() {
                    eprintln!("{status}");
                }
                history_status = status;
            }
        }
        let rendered = if tick.is_multiple_of(2) {
            if let Some(history) = history.scene() {
                view::render_graphics(history, width, height, &mut pixels)
            } else {
                view::render(face, width, height, tick, &mut pixels, preferred).map(|_| ())
            }
        } else {
            view::render(face, width, height, tick, &mut pixels, preferred).map(|_| ())
        };
        match rendered {
            Ok(_) => {
                if let Some(notice) = historical_notice.as_deref() {
                    view::render_notice(notice, width, height, &mut pixels)?;
                } else if !history_status.is_empty()
                    && tick.is_multiple_of(2)
                    && history.scene().is_some()
                {
                    view::render_notice(
                        "RETAINED HISTORY / INPUT UNAVAILABLE / LAST ACCEPTED ARTIFACT",
                        width,
                        height,
                        &mut pixels,
                    )?;
                }
                fb.present(&pixels)?;
            }
            Err(reason) => {
                view::render(
                    view::unavailable(&reason),
                    width,
                    height,
                    0,
                    &mut pixels,
                    preferred,
                )?;
                fb.present(&pixels)?;
                eprintln!("{reason}");
            }
        }
        if previous != status {
            eprintln!("{status}");
            previous = status;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Linux Face display stopped: {error}");
        std::process::exit(1);
    }
}
