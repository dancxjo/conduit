//! Make-time sRGB transfer table. Runtime contrast checks use only bounded integers.
use std::{env, fs, path::PathBuf};
pub fn generate() {
    let mut out = String::from("const SRGB_LINEAR_FLOOR: [u32; 256] = [\n");
    for value in 0..=255 {
        let v = f64::from(value) / 255.0;
        let linear = if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        };
        out.push_str(&format!("{},", (linear * 1_000_000_000.0).floor() as u32));
        if value % 8 == 7 {
            out.push('\n');
        }
    }
    out.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("local_look_srgb.rs"),
        out,
    )
    .expect("write finite transfer table");
}
