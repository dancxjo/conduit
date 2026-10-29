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
    let mut history = history_input::HistoryInput::new_at_size(
        preferred,
        width,
        height.saturating_sub(view::STATUS_STRIP_HEIGHT as u16),
    );
    let mut history_status = String::new();
    loop {
        let now = input::now_ms()?;
        if retained.last(now).is_none()
            && let Ok(face) = input::read_last_known(&input_path, boot.trim())
        {
            retained.accept(face, now);
            recovered_history = true;
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
                        "LAST KNOWN READING · LIVE UPDATES DELAYED".into()
                    } else {
                        format!(
                            "LAST UPDATE RECEIVED {}s AGO · LIVE UPDATES DELAYED",
                            age / 1000
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
            if status != history_status && !status.is_empty() {
                eprintln!("{status}");
            }
            history_status = status;
        }
        let history_page = tick.is_multiple_of(2) && history.scene().is_some();
        let face_tick = face_page_tick(tick, history.scene().is_some());
        let rendered = if history_page {
            if let Some(history) = history.scene() {
                view::render_graphics(history, width, height, &mut pixels)
            } else {
                view::render(face, width, height, face_tick, &mut pixels, preferred).map(|_| ())
            }
        } else {
            view::render(face, width, height, face_tick, &mut pixels, preferred).map(|_| ())
        };
        match rendered {
            Ok(_) => {
                if history_page {
                    let status = if !history_status.is_empty() {
                        "HISTORY UPDATES DELAYED"
                    } else if historical_notice.is_some() {
                        "LIVE UPDATES DELAYED"
                    } else {
                        "DEVICE ELAPSED TIME"
                    };
                    let notice = format!(
                        "{} / {status}",
                        history.label().unwrap_or("RETAINED HISTORY")
                    );
                    view::render_notice(&notice, width, height, &mut pixels)?;
                } else if let Some(notice) = historical_notice.as_deref() {
                    view::render_notice(notice, width, height, &mut pixels)?;
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
fn face_page_tick(tick: usize, history_present: bool) -> usize {
    if history_present { tick / 2 } else { tick }
}
#[cfg(test)]
mod paging_tests {
    #[test]
    fn historical_interleaving_visits_every_face_page() {
        for pages in [2, 3, 5] {
            let shown: Vec<_> = (0..pages * 4)
                .filter(|tick| tick % 2 == 1)
                .map(|tick| super::face_page_tick(tick, true) % pages)
                .collect();
            assert_eq!(
                shown,
                (0..pages * 2).map(|page| page % pages).collect::<Vec<_>>()
            );
        }
        for tick in 0..12 {
            assert_eq!(super::face_page_tick(tick, false), tick);
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Linux Face display stopped: {error}");
        std::process::exit(1);
    }
}
