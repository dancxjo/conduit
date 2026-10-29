//! Read-only Linux framebuffer realization of an exact caller-supplied Face.
mod framebuffer;
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
    let start = Instant::now();
    let mut previous = String::new();
    loop {
        let (face, status) = match input::read(&input_path, boot.trim()) {
            Ok(face) => {
                let status = format!("Face {} revision {}", face.identity.as_str(), face.revision);
                (face, status)
            }
            Err(reason) => (view::unavailable(&reason), reason),
        };
        let tick = (start.elapsed().as_secs() / 8) as usize;
        match view::render(face, width, height, tick, &mut pixels) {
            Ok(_) => fb.present(&pixels)?,
            Err(reason) => {
                view::render(view::unavailable(&reason), width, height, 0, &mut pixels)?;
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
