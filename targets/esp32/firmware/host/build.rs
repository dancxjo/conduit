//! Checked physical Host construction uses the shared host-side Make emitter.
fn main() {
    println!("cargo:rerun-if-changed=heap_layout.rs");
    println!("cargo:rerun-if-changed=recording_build.rs");
    conduit_host_esp32_make::image_emission::emit_from_environment();
}
