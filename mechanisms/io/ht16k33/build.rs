//! Optional checked frame material; only the fabrication host parses Source.
#[cfg(feature = "canonical-frame")]
#[path = "build/frame.rs"]
mod frame;
fn main() {
    #[cfg(feature = "canonical-frame")]
    frame::emit();
}
