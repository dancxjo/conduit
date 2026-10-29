//! Birth on the construction host; link ordinary biography, not future grants.
use std::{fs, io::Read, path::Path};

pub fn emit(out: &Path) -> conduit_environment_make::NativeSoftwareSourceRecipe {
    const VARIABLE: &str = "CONDUIT_ENVIRONMENT_BODY_BIRTH_SIGN";
    println!("cargo:rerun-if-env-changed={VARIABLE}");
    let path = std::path::PathBuf::from(
        std::env::var_os(VARIABLE)
            .expect("body image requires the construction host's retained birth Sign"),
    );
    println!("cargo:rerun-if-changed={}", path.display());
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .expect("birth Sign file")
        .take(8193)
        .read_to_end(&mut bytes)
        .expect("read birth Sign");
    assert!(bytes.len() <= 8192, "birth Sign exceeds image profile");
    let sign = serde_json::from_slice(&bytes).expect("typed construction birth Sign");
    #[cfg(not(feature = "target-mask-memory-check"))]
    let image = conduit_environment_make::EnvironmentBodyImage::prepare_from_sign(&sign)
        .expect("checked image Body birth");
    #[cfg(feature = "target-mask-memory-check")]
    let image = conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign)
        .expect("checked ordinary Mask Body birth");
    let biography = serde_json::to_vec(image.body.evidence()).expect("ordinary biography encoding");
    assert!(
        biography.len() <= 8192,
        "biography exceeds linked image profile"
    );
    let retained = path
        .parent()
        .expect("birth Sign directory")
        .join("body-biography.json");
    if retained.exists() {
        assert_eq!(
            fs::read(&retained).expect("retained biography"),
            biography,
            "construction changed: choose a new output directory for a new birth"
        );
    } else {
        use std::io::Write;
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&retained)
            .expect("new retained biography")
            .write_all(&biography)
            .expect("retain biography");
    }
    fs::write(out.join("body-biography.json"), biography).expect("linked biography");
    let mut bootstrap = String::new();
    image
        .body
        .write_bootstrap_rust(&mut bootstrap)
        .expect("exact pristine Body representation");
    fs::write(out.join("body-bootstrap.rs"), bootstrap).expect("compiled biography representation");
    image.sampling_software
}
