//! Digest-bound native Home front evidence produced by the packaged application.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use conduit_home_native::NativeHomeJourneyReceipt;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::render::{Canvas, render};

const WIDTH: usize = 1120;
const HEIGHT: usize = 720;

#[derive(Serialize)]
struct HomeFaceReceipt<'a> {
    schema: &'static str,
    front_id: &'a str,
    proof_class: &'static str,
    step_ids: &'a [&'static str],
    host_id: &'a str,
    boot_id: &'a str,
    plan_id: &'a str,
    active_play_id: &'a str,
    renderer_id: String,
    manifestation_id: String,
    artifact_path: String,
    artifact_sha256: String,
}

pub fn retain(journey: &NativeHomeJourneyReceipt, root: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir(root)
        .map_err(|error| format!("create new native Home evidence directory: {error}"))?;
    let front = journey.host_front;
    if !matches!(front, "linux-native" | "windows-native") {
        return Err(format!(
            "{front} is not an admitted native Home evidence front"
        ));
    }

    let mut pixels = vec![0_u32; WIDTH * HEIGHT];
    let mut canvas = Canvas::new(&mut pixels, WIDTH, HEIGHT);
    render(
        &mut canvas,
        &journey.form_presentation,
        &journey.form_notice,
    );
    let artifact_name = format!("home-{front}.png");
    let artifact_path = root.join(&artifact_name);
    let artifact = png(&pixels)?;
    create_new(&artifact_path, &artifact)?;
    let digest = format!("sha256:{:x}", Sha256::digest(&artifact));
    let renderer_id = format!("presentation/renderer-native-software-{front}@1");
    let manifestation_id = format!("manifestation/home/{front}/{}", &digest[7..]);
    let receipt = HomeFaceReceipt {
        schema: "conduit.evidence/home-front@1",
        front_id: front,
        proof_class: "native-software-renderer",
        step_ids: &journey.step_ids,
        host_id: &journey.host_id,
        boot_id: &journey.boot_id,
        plan_id: &journey.form_plan_id,
        active_play_id: &journey.form_play_id,
        renderer_id,
        manifestation_id,
        artifact_path: artifact_name,
        artifact_sha256: digest,
    };
    let receipt_path = root.join(format!("home-front-{front}.json"));
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("encode native Home front receipt: {error}"))?;
    create_new(&receipt_path, &bytes)?;
    Ok(receipt_path)
}

fn png(pixels: &[u32]) -> Result<Vec<u8>, String> {
    let mut rgb = Vec::with_capacity(pixels.len() * 3);
    for pixel in pixels {
        rgb.extend_from_slice(&[
            ((pixel >> 16) & 0xff) as u8,
            ((pixel >> 8) & 0xff) as u8,
            (pixel & 0xff) as u8,
        ]);
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, WIDTH as u32, HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("start native Home PNG: {error}"))?;
    writer
        .write_image_data(&rgb)
        .map_err(|error| format!("encode native Home PNG: {error}"))?;
    writer
        .finish()
        .map_err(|error| format!("finish native Home PNG: {error}"))?;
    Ok(bytes)
}

fn create_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("create new {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_is_bounded_and_exactly_sized() {
        let bytes = png(&vec![0x12_34_56; WIDTH * HEIGHT]).unwrap();
        let decoder = png::Decoder::new(bytes.as_slice());
        let reader = decoder.read_info().unwrap();
        assert_eq!(reader.info().width, WIDTH as u32);
        assert_eq!(reader.info().height, HEIGHT as u32);
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);
    }
}
