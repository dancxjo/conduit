//! Read-only Linux framebuffer realization of an exact caller-supplied Face.
mod columns;
mod display_clock;
mod framebuffer;
mod history_input;
mod input;
mod mask_controls;
mod presentation_state;
#[cfg(test)]
mod presentation_tests;
mod property_mask;
mod raster_target;
mod source_history;
#[cfg(test)]
mod todo_mask_tests;
mod touch_input;
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
    let interaction_height = display_clock::content_height(height)?;
    let content_height = interaction_height
        .checked_sub(mask_controls::HEIGHT)
        .ok_or("display too short for presentation controls")?;
    let mut ui = presentation_state::PresentationState::new(width, height, preferred);
    let content_len = usize::from(width) * usize::from(content_height);
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
    let mut columns = columns::Columns::new(width, content_height);
    let history_width = columns
        .as_ref()
        .map_or(width, |layout| layout.history_width);
    let mut history = history_input::HistoryInput::new_at_size(
        preferred,
        history_width,
        content_height.saturating_sub(view::STATUS_STRIP_HEIGHT as u16),
    );
    let mut history_status = String::new();
    let mut dirty = true;
    let mut last_render = Instant::now() - Duration::from_secs(1);
    loop {
        let graph_left = columns
            .as_ref()
            .map_or(0, |c| c.report_width + columns::GAP);
        dirty |= ui.poll(width, interaction_height, graph_left);
        if last_render.elapsed() < Duration::from_millis(100)
            || (!dirty && last_render.elapsed() < Duration::from_secs(1))
        {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        dirty = false;
        last_render = Instant::now();
        let preferred = Some(ui.preferences.temperature.unit());
        let look = ui
            .preferences
            .theme
            .look()
            .map_err(|e| format!("selected Look: {e:?}"))?;
        use conduit_face::presentation_preferences::SplitLayout;
        match ui.preferences.split {
            SplitLayout::Split { .. } if width >= 652 => {
                let requested = mask_controls::divider_position(ui.preferences, width);
                if columns.as_ref().is_none_or(|c| c.report_width != requested) {
                    columns = columns::Columns::at_divider(width, content_height, requested);
                }
            }
            _ => columns = None,
        }
        let history_width = columns.as_ref().map_or(width, |c| c.history_width);
        history.configure(
            ui.preferences.temperature.unit(),
            history_width,
            content_height.saturating_sub(view::STATUS_STRIP_HEIGHT as u16),
            ui.preferences.history_window.milliseconds(),
        )?;
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
        let shared_history_present = source_history::present(&face);
        ui.context = mask_controls::Context::for_face(
            &face,
            history_path.is_some() || shared_history_present,
        );
        ui.context.temperature |= shared_history_present;
        if !ui.context.history {
            columns = None;
        }
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
        if let Some(columns) = columns.as_mut() {
            let shared_history = if shared_history_present {
                match source_history::prepare(
                    &face,
                    preferred.unwrap_or(conduit_core::Unit::Celsius),
                    columns.history_width,
                    content_height.saturating_sub(view::STATUS_STRIP_HEIGHT as u16),
                    ui.preferences.history_window.milliseconds(),
                    Some(&look),
                ) {
                    Ok(prepared) => prepared,
                    Err(reason) => Some(source_history::Prepared {
                        scene: source_history::refused_scene(
                            &reason,
                            columns.history_width,
                            content_height.saturating_sub(view::STATUS_STRIP_HEIGHT as u16),
                        )?,
                        label: "Source history unavailable · Details".into(),
                    }),
                }
            } else {
                None
            };
            let history_notice = if let Some(prepared) = &shared_history {
                prepared.label.as_str()
            } else if !history_status.is_empty() {
                "HISTORY UPDATES DELAYED"
            } else {
                history.label().unwrap_or("RETAINED HISTORY")
            };
            let report_notice = historical_notice
                .as_ref()
                .map(|_| "LAST KNOWN · FEED DELAYED");
            columns.render_with_palette(
                face,
                shared_history
                    .as_ref()
                    .map(|prepared| &prepared.scene)
                    .or_else(|| history.scene()),
                tick,
                columns::Presentation {
                    unit: preferred,
                    details: ui.preferences.details,
                    retained_snapshot: historical_notice.is_some(),
                    history_in_primary: true,
                    look: Some(&look),
                },
                columns::Notices {
                    report: report_notice,
                    history: history_notice,
                },
                &mut pixels[..content_len],
            )?;
            mask_controls::chrome_in_context(
                width,
                height,
                &mut pixels,
                ui.preferences,
                &look,
                ui.context,
            )?;
            display_clock::render(width, height, start.elapsed().as_secs(), &mut pixels)?;
            fb.present(&pixels)?;
            if previous != status {
                eprintln!("{status}");
                previous = status;
            }
            continue;
        }
        let history_page = !shared_history_present
            && !ui.preferences.details
            && tick.is_multiple_of(2)
            && history.scene().is_some();
        let face_tick = face_page_tick(
            tick,
            !shared_history_present && !ui.preferences.details && history.scene().is_some(),
        );
        let rendered = if history_page {
            if let Some(history) = history.scene() {
                view::render_graphics_with_palette(
                    history,
                    width,
                    content_height,
                    &mut pixels[..content_len],
                    Some(&look),
                )
            } else {
                view::render_with_details(
                    face,
                    width,
                    content_height,
                    face_tick,
                    &mut pixels[..content_len],
                    view::Presentation {
                        unit: preferred,
                        look: Some(&look),
                        details: ui.preferences.details,
                        retained_snapshot: historical_notice.is_some(),
                        history_in_primary: true,
                    },
                )
                .map(|_| ())
            }
        } else {
            view::render_with_details(
                face,
                width,
                content_height,
                face_tick,
                &mut pixels[..content_len],
                view::Presentation {
                    unit: preferred,
                    look: Some(&look),
                    details: ui.preferences.details,
                    retained_snapshot: historical_notice.is_some(),
                    history_in_primary: true,
                },
            )
            .map(|_| ())
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
                    view::render_notice_with_palette(
                        &notice,
                        width,
                        content_height,
                        &mut pixels[..content_len],
                        Some(&look),
                    )?;
                } else if let Some(notice) = historical_notice.as_deref() {
                    view::render_notice_with_palette(
                        notice,
                        width,
                        content_height,
                        &mut pixels[..content_len],
                        Some(&look),
                    )?;
                }
                mask_controls::chrome_in_context(
                    width,
                    height,
                    &mut pixels,
                    ui.preferences,
                    &look,
                    ui.context,
                )?;
                display_clock::render(width, height, start.elapsed().as_secs(), &mut pixels)?;
                fb.present(&pixels)?;
            }
            Err(reason) => {
                view::render_with_details(
                    view::unavailable(&reason),
                    width,
                    content_height,
                    0,
                    &mut pixels[..content_len],
                    view::Presentation {
                        unit: preferred,
                        look: Some(&look),
                        details: ui.preferences.details,
                        retained_snapshot: historical_notice.is_some(),
                        history_in_primary: true,
                    },
                )?;
                mask_controls::chrome_in_context(
                    width,
                    height,
                    &mut pixels,
                    ui.preferences,
                    &look,
                    ui.context,
                )?;
                display_clock::render(width, height, start.elapsed().as_secs(), &mut pixels)?;
                fb.present(&pixels)?;
                eprintln!("{reason}");
            }
        }
        if previous != status {
            eprintln!("{status}");
            previous = status;
        }
    }
}
fn face_page_tick(tick: usize, history_present: bool) -> usize {
    if history_present { tick / 2 } else { tick }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Linux Face display stopped: {error}");
        std::process::exit(1);
    }
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
