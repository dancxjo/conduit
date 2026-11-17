//! Host-only generation using the SAME emitter as firmware build.rs.
//! Required checked IMAGE/Source/workset variables retain their existing names.
fn main() {
    let mut args = std::env::args_os().skip(1);
    let output = args.next().expect("usage: emit-source OUTPUT_DIRECTORY");
    assert!(args.next().is_none(), "usage: emit-source OUTPUT_DIRECTORY");
    std::fs::create_dir_all(&output).expect("create explicit Source output directory");
    std::env::set_var("OUT_DIR", &output);
    conduit_host_esp32_make::image_emission::emit_from_environment();
}
