//! Trusted public provisioning input; never read a controller private key.
use std::{io::Read, path::Path};
pub fn emit(out: &Path) {
    const VARIABLE: &str = "CONDUIT_ENVIRONMENT_OWNER_PUBLIC_KEY_FILE";
    println!("cargo:rerun-if-env-changed={VARIABLE}");
    let definition = match std::env::var_os(VARIABLE) {
        None => "pub const PUBLIC_KEY: Option<[u8; 32]> = None;\n".to_owned(),
        Some(path) => {
            let path = std::path::PathBuf::from(path);
            println!("cargo:rerun-if-changed={}", path.display());
            let mut bytes = Vec::new();
            std::fs::File::open(&path)
                .expect("owner public-key file")
                .take(33)
                .read_to_end(&mut bytes)
                .expect("read owner public key");
            let key: [u8; 32] = bytes
                .try_into()
                .expect("owner public key must be exactly 32 bytes");
            assert_ne!(key, [0; 32], "owner public key must not be zero");
            format!("pub const PUBLIC_KEY: Option<[u8; 32]> = Some({key:?});\n")
        }
    };
    std::fs::write(out.join("owner-control-key.rs"), definition)
        .expect("owner public-key material");
}
