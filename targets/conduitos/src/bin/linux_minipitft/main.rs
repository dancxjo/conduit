extern crate alloc;
#[path = "../../protocol_boot/authority.rs"]
mod authority;
#[path = "../../protocol_boot/body.rs"]
mod body;
#[path = "../../protocol_boot/frame_chunks.rs"]
mod frame_chunks;
#[path = "../../protocol_boot/minipitft_face.rs"]
mod minipitft_face;
#[path = "../../protocol_boot/minipitft_source.rs"]
mod minipitft_source;
mod owners;
mod providers;
mod runtime;
use conduitos::{display, native_face_scene, protocol_source};
static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
unsafe extern "C" {
    fn signal(number: i32, handler: extern "C" fn(i32)) -> usize;
}
extern "C" fn stop(_: i32) {
    STOP.store(true, std::sync::atomic::Ordering::Relaxed);
}
fn main() {
    unsafe {
        signal(2, stop);
        signal(15, stop);
    }
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let entry = minipitft_source::entry().map_err(|e| format!("source: {e:?}"))?;
    let host = std::env::args().nth(1).ok_or("supply installed Host ID")?;
    let boot =
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(|e| e.to_string())?;
    let mut retained = unsafe {
        runtime::run(
            entry,
            host.into(),
            boot.trim().into(),
            "linux-minipitft-diagnostic@1",
            |n| eprintln!("stage {n}"),
        )
    }
    .map_err(str::to_string)?;
    println!("display-transfer-complete; diagnostic only; no Show acknowledgement");
    // Keep ownership and the backlight until the service is explicitly stopped.
    while !STOP.load(std::sync::atomic::Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    retained
        .play
        .cancel()
        .map_err(|e| format!("cancel: {e:?}"))?;
    Ok(())
}
